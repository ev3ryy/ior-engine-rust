pub mod app;
pub mod camera;
pub mod game;
pub mod physics;

use std::sync::Arc;
use tokio::runtime::Runtime;

use app::Application;
use asset::{AssetEvent, AssetServer};
use camera::{Camera, CameraController};
use ecs::World;
use game::{GameInput, GamePlugin, PlayState};
use glam::Vec3;
use physics::PhysicsWorld;
use render::light::LightUniform;
use render::Renderer;
use winit::{
    application::ApplicationHandler,
    event::{ElementState, KeyEvent, WindowEvent},
    event_loop::{ActiveEventLoop, EventLoop},
    keyboard::{KeyCode, PhysicalKey},
    window::Window,
};

pub struct EngineConfig {
    pub window_title: String,
    pub window_width: u32,
    pub window_height: u32,
    pub msaa_samples: u32,
}

#[derive(Default, Clone, Copy)]
pub struct EngineMetrics {
    pub fps: f32,
    pub frame_time_ms: f32,
    pub draw_calls: u32,
    pub instance_count: u32,
}

pub struct EngineRunner<A: Application> {
    pub app: A,
    pub config: EngineConfig,

    pub egui_state: Option<egui_winit::State>,
    pub egui_ctx: egui::Context,

    pub tracked_models: Arc<std::sync::Mutex<std::collections::HashMap<asset::AssetId, String>>>,
    pub asset_events: tokio::sync::broadcast::Receiver<AssetEvent>,
    pub asset_server: Arc<AssetServer>,

    pub world: World,
    pub physics: PhysicsWorld,
    pub game_plugin: Box<dyn GamePlugin>,
    pub play_state: PlayState,
    pub game_input: GameInput,
    pub last_cursor_pos: Option<(f64, f64)>,
    pub world_snapshot: Option<Vec<u8>>,

    pub camera: Camera,
    pub camera_controller: CameraController,
    pub light_uniforms_cache: Vec<LightUniform>,

    pub metrics: EngineMetrics,
    pub last_frame_time: std::time::Instant,
    pub frames_this_second: u32,
    pub last_fps_update: std::time::Instant,

    pub rt: Arc<Runtime>,
    pub renderer: Option<Renderer>,
    pub default_textures: Option<Arc<render::resources::DefaultTextures>>,
    pub window: Option<Arc<Window>>,
}

impl<A: Application> EngineRunner<A> {
    pub fn start_game(&mut self) {
        if self.play_state == PlayState::Editing {
            if let Ok(snapshot) = self.world.create_snapshot() {
                self.world_snapshot = Some(snapshot);
            }
            self.play_state = PlayState::Playing;

            self.physics.clear();
            self.physics.sync_from_ecs(&self.world);
            self.game_plugin.on_start(&self.world);
            log::info!("[ENGINE] Game started (Play Mode). Snapshot saved.");
        }
    }

    pub fn start_simulate(&mut self) {
        if self.play_state == PlayState::Editing {
            if let Ok(snapshot) = self.world.create_snapshot() {
                self.world_snapshot = Some(snapshot);
            }
            self.play_state = PlayState::Simulating;

            self.physics.clear();
            self.physics.sync_from_ecs(&self.world);
            self.game_plugin.on_start(&self.world);
            log::info!("[ENGINE] Simulation started (Simulate Mode). Snapshot saved.");
        }
    }

    pub fn stop_game(&mut self) {
        if self.play_state != PlayState::Editing {
            self.game_plugin.on_stop(&self.world);
            self.physics.clear();

            if let Some(bytes) = &self.world_snapshot {
                if let Err(e) = self.world.restore_snapshot(bytes) {
                    log::error!("[ENGINE] Failed to restore world snapshot: {e}");
                }
            }
            self.play_state = PlayState::Editing;
            log::info!("[ENGINE] Game/Simulation stopped. Snapshot restored.");
        }
    }

    pub fn pause_game(&mut self) {
        match self.play_state {
            PlayState::Playing => {
                self.play_state = PlayState::Paused { step_frame: false, was_simulating: false };
                log::info!("[ENGINE] Game paused.");
            }
            PlayState::Simulating => {
                self.play_state = PlayState::Paused { step_frame: false, was_simulating: true };
                log::info!("[ENGINE] Simulation paused.");
            }
            PlayState::Paused { was_simulating, .. } => {
                self.play_state = if was_simulating {
                    PlayState::Simulating
                } else {
                    PlayState::Playing
                };
                log::info!("[ENGINE] Resumed.");
            }
            PlayState::Editing => {}
        }
    }

    pub fn step_frame(&mut self) {
        if let PlayState::Paused { ref mut step_frame, .. } = self.play_state {
            *step_frame = true;
        }
    }

    fn load_model_async(&mut self, file_name: &str) {
        let asset_id = asset::AssetId::from_path(file_name);

        {
            let mut tracked = self.tracked_models.lock().unwrap();
            if tracked.contains_key(&asset_id) {
                return;
            }
            tracked.insert(asset_id, file_name.to_string());
        }

        let (Some(renderer), Some(default_textures)) = (&self.renderer, &self.default_textures) else {
            return;
        };

        let device = renderer.device.clone();
        let queue = renderer.queue.clone();
        let layout = renderer.material_bind_group_layout.clone();
        let default_textures = default_textures.clone();
        let server = self.asset_server.clone();
        let file_name_owned = file_name.to_string();
        let tracked_models = self.tracked_models.clone();

        self.rt.spawn(async move {
            let server_for_closure = server.clone();
            let file_name_clone = file_name_owned.clone();

            let res = server
                .load(asset::AssetId::from_path(&file_name_owned), move || async move {
                    render::resources::load_binary_model(
                        &file_name_clone,
                        &device,
                        &queue,
                        &layout,
                        &default_textures,
                        |tex_name, is_srgb| {
                            let tex_name = tex_name.to_string();
                            let dev = device.clone();
                            let q = queue.clone();
                            let srv = server_for_closure.clone();

                            async move {
                                srv.load(asset::AssetId::from_path(&tex_name), move || async move {
                                    let data = render::resources::load_binary(&tex_name).await?;
                                    let tex = render::texture::Texture::from_bytes(
                                        &dev, &q, &data, &tex_name, is_srgb,
                                    )?;
                                    Ok(tex)
                                })
                                .await
                                .map_err(|e| anyhow::anyhow!("Asset load error: {}", e))
                            }
                        },
                    )
                    .await
                })
                .await;

            match res {
                Ok(_) => {
                    log::info!("Model {} loaded successfully!", file_name_owned);
                }
                Err(e) => {
                    log::error!("Failed to load model '{}': {:#?}", file_name_owned, e);
                    let mut tracked = tracked_models.lock().unwrap();
                    tracked.remove(&asset_id);
                }
            }
        });
    }
}

impl<A: Application> ApplicationHandler for EngineRunner<A> {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        event_loop.set_control_flow(winit::event_loop::ControlFlow::Poll);

        if self.window.is_none() {
            let icon_bytes = include_bytes!("../res/icon.png");
            let image_decoded = image::load_from_memory(icon_bytes)
                .expect("Failed to decode icon")
                .into_rgba8();

            let (icon_width, icon_height) = image_decoded.dimensions();
            let icon_rgba = image_decoded.into_raw();

            let window_icon = winit::window::Icon::from_rgba(icon_rgba, icon_width, icon_height)
                .expect("Failed to create window icon");

            let window_attributes = Window::default_attributes()
                .with_title(self.config.window_title.clone())
                .with_window_icon(Some(window_icon))
                .with_inner_size(winit::dpi::LogicalSize::new(
                    self.config.window_width,
                    self.config.window_height,
                ));

            let window = Arc::new(event_loop.create_window(window_attributes).unwrap());
            let size = window.inner_size();

            let renderer = pollster::block_on(Renderer::new(
                window.clone(),
                size.width,
                size.height,
                self.config.msaa_samples,
            ))
            .unwrap();

            let default_textures = Arc::new(render::resources::DefaultTextures::new(
                &renderer.device,
                &renderer.queue,
            ));

            if size.width > 0 && size.height > 0 {
                self.camera.aspect = size.width as f32 / size.height as f32;
            }

            let viewport_id = egui::ViewportId::ROOT;
            self.egui_state = Some(egui_winit::State::new(
                self.egui_ctx.clone(),
                viewport_id,
                window.as_ref(),
                Some(window.scale_factor() as f32),
                None,
                None,
            ));

            self.default_textures = Some(default_textures);
            self.renderer = Some(renderer);
            self.window = Some(window.clone());

            self.game_plugin.init(&self.world);

            self.app
                .on_init(&mut self.world, &mut self.camera, &self.asset_server);

            window.request_redraw();
        }
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        _window_id: winit::window::WindowId,
        event: WindowEvent,
    ) {
        match &event {
            WindowEvent::CloseRequested => {
                event_loop.exit();
                return;
            }
            WindowEvent::Resized(size) => {
                if size.width > 0 && size.height > 0 {
                    self.camera.aspect = size.width as f32 / size.height as f32;
                    if let Some(renderer) = &mut self.renderer {
                        renderer.resize(size.width, size.height);
                    }
                    self.app.on_resize(size.width, size.height);
                }
            }
            _ => {}
        }

        match &event {
            WindowEvent::KeyboardInput {
                event: KeyEvent { physical_key, state, repeat, .. },
                ..
            } => {
                if let PhysicalKey::Code(code) = physical_key {
                    if *state == ElementState::Pressed {
                        self.game_input.keys_pressed.insert(*code);
                        if !repeat {
                            self.game_input.keys_just_pressed.insert(*code);
                        }
                    } else {
                        self.game_input.keys_pressed.remove(code);
                    }
                }
            }
            WindowEvent::MouseInput { state, button, .. } => {
                if *state == ElementState::Pressed {
                    self.game_input.mouse_pressed.insert(*button);
                    self.game_input.mouse_just_pressed.insert(*button);
                } else {
                    self.game_input.mouse_pressed.remove(button);
                }
            }
            WindowEvent::CursorMoved { position, .. } => {
                if let Some((last_x, last_y)) = self.last_cursor_pos {
                    let dx = (position.x - last_x) as f32;
                    let dy = (position.y - last_y) as f32;
                    self.game_input.look_delta += glam::Vec2::new(dx, dy);
                }
                self.last_cursor_pos = Some((position.x, position.y));
            }
            WindowEvent::CursorLeft { .. } => {
                self.last_cursor_pos = None;
            }
            _ => {}
        }

        let mut move_axis = glam::Vec2::ZERO;
        if self.game_input.is_key_down(KeyCode::KeyW) || self.game_input.is_key_down(KeyCode::ArrowUp) { move_axis.y += 1.0; }
        if self.game_input.is_key_down(KeyCode::KeyS) || self.game_input.is_key_down(KeyCode::ArrowDown) { move_axis.y -= 1.0; }
        if self.game_input.is_key_down(KeyCode::KeyD) || self.game_input.is_key_down(KeyCode::ArrowRight) { move_axis.x += 1.0; }
        if self.game_input.is_key_down(KeyCode::KeyA) || self.game_input.is_key_down(KeyCode::ArrowLeft) { move_axis.x -= 1.0; }
        self.game_input.move_axis = move_axis;

        if let Some(egui_state) = &mut self.egui_state {
            if let Some(window) = &self.window {
                let _ = egui_state.on_window_event(window.as_ref(), &event);
            }
        }

        let is_typing_in_ui = matches!(&event, WindowEvent::KeyboardInput { .. })
            && self.egui_ctx.egui_wants_keyboard_input();

        if !is_typing_in_ui {
            if self.app.on_window_event(
                &mut self.world,
                &mut self.camera,
                &mut self.camera_controller,
                &event,
            ) {
                return;
            }
        }

        match event {
            WindowEvent::RedrawRequested => {
                let now = std::time::Instant::now();
                self.metrics.frame_time_ms =
                    now.duration_since(self.last_frame_time).as_secs_f32() * 1000.0;
                self.last_frame_time = now;
                self.frames_this_second += 1;

                let dt = self.metrics.frame_time_ms / 1000.0;

                if now.duration_since(self.last_fps_update).as_secs_f32() >= 1.0 {
                    self.metrics.fps = self.frames_this_second as f32;
                    self.frames_this_second = 0;
                    self.last_fps_update = now;
                }

                if let Some(window) = &self.window {
                    window.request_redraw();
                }

                let mut reloads: Vec<String> = Vec::new();
                while let Ok(event) = self.asset_events.try_recv() {
                    match event {
                        AssetEvent::Reloaded { id, .. } | AssetEvent::Unloaded { id, .. } => {
                            let mut tracked = self.tracked_models.lock().unwrap();
                            if let Some(name) = tracked.remove(&id) {
                                reloads.push(name);
                            } else if id.has_known_path() {
                                reloads.push(id.path());
                            }
                        }
                        AssetEvent::Failed { id, error, .. } => {
                            let mut tracked = self.tracked_models.lock().unwrap();
                            if let Some(name) = tracked.remove(&id) {
                                log::warn!("Removed failed model '{}' from cache: {}", name, error);
                            }
                        }
                        _ => {}
                    }
                }

                for name in reloads {
                    log::warn!("Hot-reloading model: {}", name);
                    self.asset_server
                        .unload::<render::model::Model>(asset::AssetId::from_path(&name));
                    self.load_model_async(&name);
                }

                match self.play_state {
                    PlayState::Playing => {
                        self.physics.sync_from_ecs(&self.world);
                        self.physics.step(dt);
                        self.physics.sync_to_ecs(&self.world);

                        self.game_plugin.on_update(&self.world, &self.game_input, dt);
                        self.world.progress();
                        self.game_input.clear_frame_deltas();
                    }
                    PlayState::Simulating => {
                        self.physics.sync_from_ecs(&self.world);
                        self.physics.step(dt);
                        self.physics.sync_to_ecs(&self.world);

                        let dummy_input = GameInput::default();
                        self.game_plugin.on_update(&self.world, &dummy_input, dt);
                        self.world.progress();
                        self.game_input.clear_frame_deltas();
                    }
                    PlayState::Paused { ref mut step_frame, was_simulating } => {
                        if *step_frame {
                            self.physics.sync_from_ecs(&self.world);
                            self.physics.step(dt);
                            self.physics.sync_to_ecs(&self.world);

                            let input = if was_simulating {
                                GameInput::default()
                            } else {
                                self.game_input.clone()
                            };
                            self.game_plugin.on_update(&self.world, &input, dt);
                            self.world.progress();
                            self.game_input.clear_frame_deltas();
                            *step_frame = false;
                        }
                    }
                    PlayState::Editing => {}
                }

                self.app
                    .on_update(&mut self.world, &mut self.camera, &mut self.camera_controller);

                self.world.update_transforms();

                self.camera_controller.update_camera(&mut self.camera, dt);

                let is_play_mode = self.play_state == PlayState::Playing;

                let camera_uniform = if is_play_mode {
                    if let Some((cam_pos, cam_rot, cam_comp)) = self.world.get_active_game_camera() {
                        let forward = cam_rot * -glam::Vec3::Z;
                        let up = cam_rot * glam::Vec3::Y;
                        let target = cam_pos + forward;
                        let game_cam = camera::Camera {
                            eye: cam_pos,
                            target,
                            up,
                            aspect: self.camera.aspect,
                            fovy: cam_comp.fovy,
                            znear: cam_comp.znear,
                            zfar: cam_comp.zfar,
                        };
                        game_cam.build_uniform()
                    } else {
                        self.camera.build_uniform()
                    }
                } else {
                    self.camera.build_uniform()
                };

                if let Some(renderer) = &mut self.renderer {
                    renderer.upload_camera(camera_uniform);
                }

                let mut egui_primitives = Vec::new();
                let mut screen_desc = egui_wgpu::ScreenDescriptor {
                    size_in_pixels: [0, 0],
                    pixels_per_point: 1.0,
                };
                let mut load_requests = Vec::new();

                let mut desired_viewport_size = None;
                let viewport_texture_id = self.renderer.as_ref().and_then(|r| r.viewport_texture_id);

                let (ui_result, play_action) = if let (Some(window), Some(egui_state), Some(renderer)) =
                    (&self.window, &mut self.egui_state, &mut self.renderer)
                {
                    let raw_input = egui_state.take_egui_input(window.as_ref());
                    self.egui_ctx.begin_pass(raw_input);

                    let action = self.app.on_ui(
                        &self.egui_ctx,
                        &mut self.world,
                        &mut self.camera,
                        &mut renderer.postprocess_settings,
                        &self.metrics,
                        &mut load_requests,
                        self.play_state,
                        &self.asset_server,
                        viewport_texture_id,
                        &mut desired_viewport_size,
                    );

                    let full_output = self.egui_ctx.end_pass();
                    egui_state.handle_platform_output(window.as_ref(), full_output.platform_output);

                    (
                        Some((
                            full_output.textures_delta,
                            full_output.shapes,
                            full_output.pixels_per_point,
                        )),
                        action,
                    )
                } else {
                    (None, game::PlayControlAction::None)
                };

                // apply dynamic viewport resize if tab size changed
                if let (Some((vp_w, vp_h)), Some(renderer)) = (desired_viewport_size, &mut self.renderer) {
                    renderer.resize_viewport(vp_w, vp_h);
                }

                match play_action {
                    game::PlayControlAction::Start => self.start_game(),
                    game::PlayControlAction::Simulate => self.start_simulate(),
                    game::PlayControlAction::Stop => self.stop_game(),
                    game::PlayControlAction::Pause => self.pause_game(),
                    game::PlayControlAction::Resume => self.pause_game(),
                    game::PlayControlAction::Step => self.step_frame(),
                    game::PlayControlAction::None => {}
                }

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

                self.light_uniforms_cache.clear();

                self.world.each_light(|pos, rot, light| {
                    let forward = rot * -glam::Vec3::Z;
                    let direction = if forward.length_squared() < 1e-6 {
                        glam::Vec3::NEG_Y
                    } else {
                        forward.normalize()
                    };

                    let light_type = match light.light_type {
                        ecs::LightType::Directional => 0,
                        ecs::LightType::Point => 1,
                        ecs::LightType::Spot => 2,
                    };

                    let (inner_cos, outer_cos) = if light_type == 2 {
                        let inner = light.inner_angle.min(light.outer_angle);
                        let outer = light.outer_angle.max(light.inner_angle);
                        (inner.cos(), outer.cos())
                    } else {
                        (0.0, 0.0)
                    };

                    self.light_uniforms_cache.push(LightUniform {
                        position: pos.into(),
                        light_type,
                        color: light.color,
                        intensity: light.intensity,
                        direction: direction.into(),
                        range: light.range.max(0.1),
                        inner_cone_cos: inner_cos,
                        outer_cone_cos: outer_cos,
                        shadow_index: -1,
                        shadow_bias: 1.0,
                    });
                });

                if let Some(renderer) = &mut self.renderer {
                    renderer.upload_lights(&self.light_uniforms_cache, &camera_uniform);
                }

                let mut batches: std::collections::BTreeMap<
                    (asset::AssetId, usize),
                    Vec<render::InstanceRaw>,
                > = std::collections::BTreeMap::new();

                self.world.each_model(|global_transform, model| {
                    batches
                        .entry((model.id, model.mesh_index))
                        .or_default()
                        .push(render::InstanceRaw::new(global_transform.0));
                });

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
                    if let Err(e) = renderer.render_batches(
                        &camera_uniform,
                        &render_data,
                        &egui_primitives,
                        &screen_desc,
                    ) {
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

pub fn run<A: Application>(
    config: EngineConfig,
    app: A,
    game_plugin: Box<dyn GamePlugin>,
) -> anyhow::Result<()> {
    let rt = Arc::new(tokio::runtime::Runtime::new()?);
    let _rt_guard = rt.enter();

    let asset_root = asset::resolve_asset_root();
    log::info!("Assets root resolved to: {:?}", asset_root);

    let asset_server = Arc::new(AssetServer::new(asset_root, std::time::Duration::from_secs(5)));
    let asset_events = asset_server.subscribe();

    if let Err(e) = asset_server.watch_directory() {
        log::error!("Failed to start asset watcher: {e}");
    }

    let event_loop = EventLoop::new()?;

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
    let camera_controller = CameraController::new(5.0);

    let egui_ctx = egui::Context::default();
    let egui_state = None;

    let tracked_models = Arc::new(std::sync::Mutex::new(std::collections::HashMap::new()));

    let mut runner = EngineRunner {
        app,
        config,

        egui_state,
        egui_ctx,

        tracked_models,
        asset_events,
        asset_server,

        world,
        physics: PhysicsWorld::new(),
        game_plugin,
        play_state: PlayState::Editing,
        game_input: GameInput::default(),
        last_cursor_pos: None,
        world_snapshot: None,

        camera,
        camera_controller,

        light_uniforms_cache: Vec::with_capacity(100),

        metrics: EngineMetrics::default(),
        last_frame_time: std::time::Instant::now(),
        frames_this_second: 0,
        last_fps_update: std::time::Instant::now(),

        rt,
        renderer: None,
        default_textures: None,
        window: None,
    };

    event_loop.run_app(&mut runner)?;
    Ok(())
}