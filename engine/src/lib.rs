pub mod app;
pub mod camera;

use std::sync::Arc;
use tokio::runtime::Runtime;

use app::Application;
use render::{Renderer};
use asset::{AssetEvent, AssetServer};
use ecs::World;
use camera::{Camera, CameraController};
use glam::{Vec3};
use winit::{
    application::ApplicationHandler,
    event::WindowEvent,
    event_loop::{ActiveEventLoop, EventLoop},
    window::Window,
};

pub struct EngineConfig {
    pub window_title: String,
    pub window_width: u32,
    pub window_height: u32,
}

struct EngineRunner<A: Application> {
    app: A,
    config: EngineConfig,
    
    egui_state: Option<egui_winit::State>,
    egui_ctx: egui::Context,
    
    loaded_models: std::collections::HashSet<String>,
    asset_events: tokio::sync::broadcast::Receiver<AssetEvent>,
    asset_server: Arc<AssetServer>,

    world: World,
    camera: Camera,
    camera_controller: CameraController,
    
    metrics: EngineMetrics,
    last_frame_time: std::time::Instant,
    frames_this_second: u32,
    last_fps_update: std::time::Instant,

    rt: Arc<Runtime>,
    renderer: Option<Renderer>,
    window: Option<Arc<Window>>,
}

#[derive(Default, Clone, Copy)]
pub struct EngineMetrics {
    pub fps: f32,
    pub frame_time_ms: f32,
    pub draw_calls: u32,
    pub instance_count: u32,
}

impl<A: Application> EngineRunner<A> {
    fn load_model_async(&mut self, file_name: &str) {
        if self.loaded_models.contains(file_name) {
            return;
        }
        self.loaded_models.insert(file_name.to_string());
        
        let Some(renderer) = &self.renderer else { return };
        
        let device = renderer.device.clone();
        let queue = renderer.queue.clone();
        let layout = renderer.texture_bind_group_layout.clone();
        let server = self.asset_server.clone();
        let file_name_owned = file_name.to_string();

        self.rt.spawn(async move {
            let server_for_closure = server.clone();
            let file_name_clone = file_name_owned.clone();
            
            let res = server.load(asset::AssetId::from_path(&file_name_owned), move || async move {
                render::resources::load_binary_model(
                    &file_name_clone,
                    &device,
                    &queue,
                    &layout,
                    |tex_name| {
                        let tex_name = tex_name.to_string();
                        let dev = device.clone();
                        let q = queue.clone();
                        let srv = server_for_closure.clone(); 
                        
                        async move {
                            srv.load(asset::AssetId::from_path(&tex_name), move || async move {
                                let data = render::resources::load_binary(&tex_name).await?;
                                let tex = render::texture::Texture::from_bytes(&dev, &q, &data, &tex_name)?;
                                Ok(tex) 
                            }).await.map_err(|e| anyhow::anyhow!("Asset load error: {}", e)) 
                        }
                    }
                ).await
            }).await;

            if res.is_err() {
                log::error!("Failed to load model async: {}", file_name_owned);
            }
        });
    }
}


impl<A: Application> ApplicationHandler for EngineRunner<A> {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        event_loop.set_control_flow(winit::event_loop::ControlFlow::Poll);

        if self.window.is_none() {
            let mut window_attributes = Window::default_attributes();
            window_attributes.title = self.config.window_title.clone();

            let window = Arc::new(event_loop.create_window(window_attributes).unwrap());
            let renderer = pollster::block_on(Renderer::new(window.clone())).unwrap();

            let viewport_id = egui::ViewportId::ROOT;
            self.egui_state = Some(egui_winit::State::new(
                self.egui_ctx.clone(),
                viewport_id,
                window.as_ref(),
                Some(window.scale_factor() as f32),
                None,
                None,
            ));

            self.renderer = Some(renderer);
            self.window = Some(window.clone());

            self.app.on_init(&mut self.world, &mut self.camera, &self.asset_server);

            window.request_redraw();
        }
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        _window_id: winit::window::WindowId,
        event: WindowEvent,
    ) {
        if self.app.on_window_event(&mut self.world, &mut self.camera, &mut self.camera_controller, &event) {
            return;
        }

        if let Some(egui_state) = &mut self.egui_state {
            if let Some(window) = &self.window {
                let response = egui_state.on_window_event(window.as_ref(), &event);
                if response.consumed {
                    return;
                }
            }
        }

        match event {
            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::Resized(size) => {
                if size.width > 0 && size.height > 0 {
                    self.camera.aspect = size.width as f32 / size.height as f32;
                    if let Some(renderer) = &mut self.renderer {
                        renderer.resize(size.width, size.height);
                    }
                    self.app.on_resize(size.width, size.height);
                }
            }
            WindowEvent::RedrawRequested => {
                let now = std::time::Instant::now();
                self.metrics.frame_time_ms = now.duration_since(self.last_frame_time).as_secs_f32() * 1000.0;
                self.last_frame_time = now;
                self.frames_this_second += 1;

                if now.duration_since(self.last_fps_update).as_secs_f32() >= 1.0 {
                    self.metrics.fps = self.frames_this_second as f32;
                    self.frames_this_second = 0;
                    self.last_fps_update = now;
                }

                if let Some(window) = &self.window {
                    window.request_redraw();
                }

                let mut reloads = Vec::new();
                while let Ok(event) = self.asset_events.try_recv() {
                    if let AssetEvent::Unloaded { id, .. } = event {
                        for name in &self.loaded_models {
                            let glb_name = name.replace(".bin", ".glb");
                            if asset::AssetId::from_path(&glb_name) == id || asset::AssetId::from_path(name) == id {
                                reloads.push(name.clone());
                            }
                        }
                    }
                }

                for name in reloads {
                    log::warn!("Hot-reloading model: {}", name);
                    self.loaded_models.remove(&name);
                    self.asset_server.unload::<render::model::Model>(asset::AssetId::from_path(&name));
                    self.load_model_async(&name);
                }

                // 1. app updates logic
                self.app.on_update(&mut self.world, &mut self.camera, &mut self.camera_controller);

                // 2. engine: update camera matrices
                self.camera_controller.update_camera(&mut self.camera);
                let view_proj = self.camera.view_projection_matrix();
 
                if let Some(renderer) = &mut self.renderer {
                    renderer.upload_view_projection(view_proj);
                }

                let mut egui_primitives = Vec::new();
                let mut screen_desc = egui_wgpu::ScreenDescriptor {
                    size_in_pixels: [0, 0],
                    pixels_per_point: 1.0,
                };

                let mut load_requests = Vec::new();

                let ui_result = if let (Some(window), Some(egui_state)) = (&self.window, &mut self.egui_state) {
                    let raw_input = egui_state.take_egui_input(window.as_ref());
                    self.egui_ctx.begin_pass(raw_input);

                    self.app.on_ui(&self.egui_ctx, &mut self.world, &mut self.camera, &self.metrics, &mut load_requests);

                    let full_output = self.egui_ctx.end_pass();
                    
                    egui_state.handle_platform_output(window.as_ref(), full_output.platform_output);
                    
                    Some((full_output.textures_delta, full_output.shapes, full_output.pixels_per_point))
                } else {
                    None
                };

                for req in load_requests {
                    self.load_model_async(&req);
                }

                if let Some((mut textures_delta, shapes, pixels_per_point)) = ui_result {
                    
                    if let Some(renderer) = &mut self.renderer {
                        for (id, image_deltas) in &textures_delta.set {
                            for image_delta in image_deltas {
                                renderer.egui_renderer.update_texture(
                                    &renderer.device,
                                    &renderer.queue,
                                    *id,
                                    image_delta,
                                );
                            }
                        }

                        egui_primitives = self.egui_ctx.tessellate(shapes, pixels_per_point);

                        screen_desc = egui_wgpu::ScreenDescriptor {
                            size_in_pixels: [renderer.config.width, renderer.config.height],
                            pixels_per_point,
                        };

                        for id in &textures_delta.free {
                            renderer.egui_renderer.free_texture(id);
                        }
                    }

                    textures_delta.clear();
                }

                // 3. engine: collect data to ecs
                let mut batches: std::collections::HashMap<(asset::AssetId, usize), Vec<render::InstanceRaw>> = std::collections::HashMap::new();

                self.world.inner.each::<(&ecs::Position, &ecs::Rotation, &ecs::Scale, &ecs::ModelComponent)>(
                    |(pos, rot, scale, model_comp)| {
                        let model_mat = glam::Mat4::from_scale_rotation_translation(scale.0, rot.0, pos.0);
                        batches
                            .entry((model_comp.id, model_comp.mesh_index))
                            .or_default()
                            .push(render::InstanceRaw { model: model_mat });
                    }
                );
                
                let mut total_instances = 0;
                let mut draw_calls = 0;
                let mut render_data = Vec::new();
                
                for ((asset_id, mesh_idx), instances) in batches {
                    if let Some(model) = self.asset_server.get::<render::model::Model>(asset_id) {
                        total_instances += instances.len();
                        draw_calls += 1;
                        render_data.push((model, mesh_idx, instances));
                    }
                }
                
                self.metrics.instance_count = total_instances as u32;
                self.metrics.draw_calls = draw_calls;
                if let Some(renderer) = &mut self.renderer {
                    if let Err(e) = renderer.render_batches(&render_data, &egui_primitives, &screen_desc) {
                        log::error!("Render error: {e}");
                        event_loop.exit();
                    }
                }

                if let Some(window) = &self.window {
                    window.request_redraw();
                }
            }
            _ => {}
        }
    }
}

pub fn run<A: Application>(config: EngineConfig, app: A) -> anyhow::Result<()> {
    let rt = Arc::new(tokio::runtime::Runtime::new()?);

    let asset_server = Arc::new(AssetServer::new(std::time::Duration::from_secs(5)));
    let asset_events = asset_server.subscribe();

    let res_path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../res");
    if let Err(e) = asset_server.watch_directory(&res_path) {
        log::error!("failed to load asset server: {e}");
    } else {
        log::info!("asset server loaded in: {:?}", res_path);
    }

    let event_loop = EventLoop::new()?;
    
    // base engine
    let world = ecs::create_world();
    let camera = Camera {
        eye: Vec3::new(0.0, 5.0, -10.0),
        target: Vec3::ZERO,
        up: Vec3::Y,
        aspect: config.window_width as f32 / config.window_height as f32,
        fovy: 45.0,
        znear: 0.1,
        zfar: 100.0,
    };
    let camera_controller = CameraController::new(0.2);

    let egui_ctx = egui::Context::default();
    let egui_state = None;

    let loaded_models = std::collections::HashSet::new();

    let mut runner = EngineRunner {
        app,
        config,
        
        egui_state,
        egui_ctx,
        
        loaded_models,
        asset_events,
        asset_server,
        
        world,
        camera,
        camera_controller,
        
        metrics: EngineMetrics::default(),
        last_frame_time: std::time::Instant::now(),
        frames_this_second: 0,
        last_fps_update: std::time::Instant::now(),

        rt,
        renderer: None,
        window: None,
    };
    
    event_loop.run_app(&mut runner)?;
    Ok(())
}