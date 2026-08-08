pub mod app;
pub mod camera;

use std::sync::Arc;
use tokio::runtime::Runtime;

use app::Application;
use render::{InstanceRaw, Renderer};
use asset::{AssetEvent, AssetId, AssetServer};
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
    renderer: Option<Renderer>,
    window: Option<Arc<Window>>,
    world: World,
    camera: Camera,
    camera_controller: CameraController,

    rt: Arc<Runtime>,

    test_model: Option<Arc<render::model::Model>>,
    asset_server: Arc<AssetServer>,
    asset_events: tokio::sync::broadcast::Receiver<AssetEvent>,
}

// !!!!!!!!! HARD CODE !!!!!!!!!!
impl<A: Application> EngineRunner<A> {
    fn load_test_model(&mut self) {
        let Some(renderer) = &self.renderer else { return };
        
        let device = renderer.device.clone();
        let queue = renderer.queue.clone();
        let layout = renderer.texture_bind_group_layout.clone();
        let server = self.asset_server.clone();
        
        log::info!("loading monkey.bin...");

        let model = self.rt.block_on(async move {
            let server_for_closure = server.clone();
            
            server.load(AssetId::from_path("monkey.bin"), move || async move {
                render::resources::load_binary_model(
                    "monkey.bin",
                    &device,
                    &queue,
                    &layout,
                    |tex_name| {
                        let tex_name = tex_name.to_string();
                        let dev = device.clone();
                        let q = queue.clone();
                        
                        let srv = server_for_closure.clone(); 
                        
                        async move {
                            srv.load(AssetId::from_path(&tex_name), move || async move {
                                let data = render::resources::load_binary(&tex_name).await?;
                                let tex = render::texture::Texture::from_bytes(&dev, &q, &data, &tex_name)?;
                                Ok(tex) 
                            })
                            .await
                            .map_err(|e| anyhow::anyhow!("Asset load error: {}", e)) 
                        }
                    }
                ).await
            }).await
        }).expect("Failed to load model through AssetServer");
        
        self.test_model = Some(model);
    }
}

impl<A: Application> ApplicationHandler for EngineRunner<A> {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_none() {
            let mut window_attributes = Window::default_attributes();
            window_attributes.title = self.config.window_title.clone();

            let window = Arc::new(event_loop.create_window(window_attributes).unwrap());
            let renderer = pollster::block_on(Renderer::new(window.clone())).unwrap();

            self.renderer = Some(renderer);
            self.window = Some(window);

            self.load_test_model();

            self.app.on_init(&mut self.world, &mut self.camera);
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
                if let Some(window) = &self.window {
                    window.request_redraw();
                }

                let mut files_changed = false;

                while let Ok(event) = self.asset_events.try_recv() {
                    if let AssetEvent::Unloaded { type_id, .. } = event {
                        if type_id == std::any::TypeId::of::<()>() {
                            files_changed = true;
                        }
                    }
                }

                if files_changed {
                    log::warn!("files changed");
                    
                    self.asset_server.unload::<render::model::Model>(AssetId::from_path("monkey.bin"));
                    
                    self.load_test_model(); 
                }

                // 1. app updates logic
                self.app.on_update(&mut self.world, &mut self.camera, &mut self.camera_controller);

                let Some(renderer) = &mut self.renderer else { return };

                // 2. engine: update camera matrices
                self.camera_controller.update_camera(&mut self.camera);
                let view_proj = self.camera.view_projection_matrix();
                renderer.upload_view_projection(view_proj);

                // 3. engine: collect data to ecs
                let positions = ecs::collect_instance_positions(&self.world);
                let mut instances: Vec<InstanceRaw> = Vec::with_capacity(positions.len());

                for &position in &positions {
                    let rotation = if position.length_squared() == 0.0 {
                        glam::Quat::from_axis_angle(glam::Vec3::Z, 0.0)
                    } else {
                        glam::Quat::from_axis_angle(position.normalize(), 45.0_f32.to_radians())
                    };

                    let model = glam::Mat4::from_rotation_translation(rotation, position);
                    instances.push(InstanceRaw { model });
                }

                renderer.upload_instances(&instances);

                // 4. engine: rendering
                if let Some(model) = &self.test_model {
                    if let Err(e) = renderer.render(model, positions.len() as u32) {
                        log::error!("Render error: {e}");
                        event_loop.exit();
                    }
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

    let res_path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../render/res");
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

    let mut runner = EngineRunner {
        app,
        config,
        renderer: None,
        window: None,
        world,
        camera,
        camera_controller,
        rt,
        asset_server,
        asset_events,
        test_model: None,
    };
    
    event_loop.run_app(&mut runner)?;
    Ok(())
}