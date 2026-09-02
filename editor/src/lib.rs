mod settings;
pub mod ui;
pub use settings::EditorConfig;
pub use ui::PlayControlAction;

use engine::app::Application;
use engine::camera::{Camera, CameraController, CameraMovement};
use engine::game::{GameInput, GamePlugin, PlayState};
use ecs::World;
use ecs::serialization::{save_world, load_world};
use winit::event::{WindowEvent, KeyEvent, ElementState, MouseButton, MouseScrollDelta};
use winit::keyboard::{KeyCode, PhysicalKey};
use transform_gizmo_egui::prelude::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EditorGizmoMode {
    Translate,
    Rotate,
    Scale,
    All,
}

pub struct Editor {
    config: EditorConfig,
    models_to_load: Vec<String>,
    is_rmb_pressed: bool,
    is_lmb_pressed: bool,
    is_alt_pressed: bool,
    last_cursor_pos: Option<(f64, f64)>,
    pub selected_entity: Option<u64>,
    pub gizmo: Gizmo,
    pub gizmo_mode: EditorGizmoMode,
    pub gizmo_orientation: GizmoOrientation,

    pub play_state: PlayState,
    pub game_plugin: Box<dyn GamePlugin>,
    pub game_input: GameInput,
    pub world_snapshot: Option<Vec<u8>>,
}

impl Editor {
    pub fn new(config: EditorConfig, game_plugin: Box<dyn GamePlugin>) -> Self {
        Self {
            config,
            models_to_load: Vec::new(),
            is_rmb_pressed: false,
            is_lmb_pressed: false,
            is_alt_pressed: false,
            last_cursor_pos: None,
            selected_entity: None,
            gizmo: Gizmo::default(),
            gizmo_mode: EditorGizmoMode::Translate,
            gizmo_orientation: GizmoOrientation::Global,

            play_state: PlayState::Editing,
            game_plugin,
            game_input: GameInput::default(),
            world_snapshot: None,
        }
    }

    pub fn start_game(&mut self, world: &mut World) {
        if self.play_state == PlayState::Editing {
            if let Ok(snapshot) = world.create_snapshot() {
                self.world_snapshot = Some(snapshot);
            }
            self.play_state = PlayState::Playing;
            self.selected_entity = None;
            self.game_plugin.on_start(world);
            log::info!("Game started (Play Mode). World snapshot created.");
        }
    }

    pub fn start_simulate(&mut self, world: &mut World) {
        if self.play_state == PlayState::Editing {
            if let Ok(snapshot) = world.create_snapshot() {
                self.world_snapshot = Some(snapshot);
            }
            self.play_state = PlayState::Simulating;
            self.game_plugin.on_start(world);
            log::info!("Simulation started (Simulate Mode). World snapshot created.");
        }
    }

    pub fn stop_game(&mut self, world: &mut World) {
        if self.play_state != PlayState::Editing {
            self.game_plugin.on_stop(world);
            if let Some(bytes) = &self.world_snapshot {
                if let Err(e) = world.restore_snapshot(bytes) {
                    log::error!("Failed to restore snapshot: {e}");
                }
            }
            self.play_state = PlayState::Editing;
            self.selected_entity = None;
            log::info!("Game/Simulation stopped. World restored to pre-run snapshot.");
        }
    }

    pub fn pause_game(&mut self) {
        match self.play_state {
            PlayState::Playing => {
                self.play_state = PlayState::Paused { step_frame: false, was_simulating: false };
                log::info!("Game paused.");
            }
            PlayState::Simulating => {
                self.play_state = PlayState::Paused { step_frame: false, was_simulating: true };
                log::info!("Simulation paused.");
            }
            PlayState::Paused { was_simulating, .. } => {
                self.play_state = if was_simulating {
                    PlayState::Simulating
                } else {
                    PlayState::Playing
                };
                log::info!("Resumed.");
            }
            PlayState::Editing => {}
        }
    }

    pub fn step_frame(&mut self) {
        if let PlayState::Paused { ref mut step_frame, .. } = self.play_state {
            *step_frame = true;
        }
    }
}

impl Application for Editor {
    fn on_init(
        &mut self,
        world: &mut World,
        camera: &mut Camera,
        _asset_server: &std::sync::Arc<asset::AssetServer>
    ) {
        self.config = EditorConfig::load("editor.cfg");
        camera.eye = self.config.camera_eye;
        camera.target = self.config.camera_target;
        camera.fovy = self.config.camera_fovy;
        self.selected_entity = None;

        eprintln!("[EDITOR] Initializing Game Plugin from DLL...");
        self.game_plugin.init(world);
        eprintln!("[EDITOR] Total registered components in world: {}", world.get_component_entries().len());

        log::info!("Editor and Game systems initialized.");
    }

    fn on_update(
        &mut self,
        world: &mut World,
        _camera: &mut Camera,
        _camera_controller: &mut CameraController,
    ) {
        let dt = 1.0 / 60.0;

        match self.play_state {
            PlayState::Playing => {
                self.game_plugin.on_update(world, &self.game_input, dt);
                world.progress();
                self.game_input.clear_frame_deltas();
            }
            PlayState::Simulating => {
                let dummy_input = GameInput::default();
                self.game_plugin.on_update(world, &dummy_input, dt);
                world.progress();
                self.game_input.clear_frame_deltas();
            }
            PlayState::Paused { ref mut step_frame, was_simulating } => {
                if *step_frame {
                    let input = if was_simulating {
                        GameInput::default()
                    } else {
                        self.game_input.clone()
                    };
                    self.game_plugin.on_update(world, &input, dt);
                    world.progress();
                    self.game_input.clear_frame_deltas();
                    *step_frame = false;
                }
            }
            PlayState::Editing => {}
        }
    }

    fn on_window_event(
        &mut self,
        world: &mut World,
        _camera: &mut Camera,
        camera_controller: &mut CameraController,
        event: &WindowEvent,
    ) -> bool {
        match event {
            WindowEvent::KeyboardInput {
                event: KeyEvent { physical_key, state: key_state, repeat, .. },
                ..
            } => {
                if let PhysicalKey::Code(code) = physical_key {
                    if *key_state == ElementState::Pressed {
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
            }
            _ => {}
        }

        let mut move_axis = glam::Vec2::ZERO;
        if self.game_input.is_key_down(KeyCode::KeyW) || self.game_input.is_key_down(KeyCode::ArrowUp) { move_axis.y += 1.0; }
        if self.game_input.is_key_down(KeyCode::KeyS) || self.game_input.is_key_down(KeyCode::ArrowDown) { move_axis.y -= 1.0; }
        if self.game_input.is_key_down(KeyCode::KeyD) || self.game_input.is_key_down(KeyCode::ArrowRight) { move_axis.x += 1.0; }
        if self.game_input.is_key_down(KeyCode::KeyA) || self.game_input.is_key_down(KeyCode::ArrowLeft) { move_axis.x -= 1.0; }
        self.game_input.move_axis = move_axis;

        if self.play_state == PlayState::Playing {
            return false;
        }

        match event {
            WindowEvent::KeyboardInput {
                event: KeyEvent { physical_key, state: key_state, repeat, .. },
                ..
            } => {
                let pressed = key_state.is_pressed();

                if matches!(physical_key, PhysicalKey::Code(KeyCode::AltLeft | KeyCode::AltRight)) {
                    self.is_alt_pressed = pressed;
                }

                if pressed && !repeat {
                    match physical_key {
                        PhysicalKey::Code(KeyCode::Escape) => {
                            if self.selected_entity.is_some() {
                                self.selected_entity = None;
                                return true;
                            }
                        }
                        PhysicalKey::Code(KeyCode::Digit1 | KeyCode::KeyG) => {
                            self.gizmo_mode = EditorGizmoMode::Translate;
                            return true;
                        }
                        PhysicalKey::Code(KeyCode::Digit2 | KeyCode::KeyE) => {
                            self.gizmo_mode = EditorGizmoMode::Rotate;
                            return true;
                        }
                        PhysicalKey::Code(KeyCode::Digit3 | KeyCode::KeyR) => {
                            self.gizmo_mode = EditorGizmoMode::Scale;
                            return true;
                        }
                        PhysicalKey::Code(KeyCode::Digit4 | KeyCode::KeyT) => {
                            self.gizmo_mode = EditorGizmoMode::All;
                            return true;
                        }
                        PhysicalKey::Code(KeyCode::KeyQ) => {
                            self.gizmo_orientation = match self.gizmo_orientation {
                                GizmoOrientation::Global => GizmoOrientation::Local,
                                GizmoOrientation::Local => GizmoOrientation::Global,
                            };
                            return true;
                        }
                        PhysicalKey::Code(KeyCode::F5) => {
                            if let Err(e) = save_world(world, &self.config.world_save_path) {
                                log::error!("Failed to save world: {}", e);
                            } else {
                                log::info!("World saved to {}", self.config.world_save_path);
                            }
                            self.config.save("editor.cfg");
                            return true;
                        }
                        PhysicalKey::Code(KeyCode::F9) => {
                            self.selected_entity = None;

                            if let Err(e) = load_world(world, &self.config.world_save_path) {
                                log::error!("Failed to load world: {}", e);
                            } else {
                                log::info!("World loaded from {}", self.config.world_save_path);
                                world.each_model(|_, model_comp| {
                                    self.models_to_load.push(model_comp.id.path());
                                });
                            }
                            self.config = EditorConfig::load("editor.cfg");
                            return true;
                        }
                        _ => {}
                    }
                }

                let mut is_movement = true;
                match physical_key {
                    PhysicalKey::Code(KeyCode::KeyW | KeyCode::ArrowUp) => {
                        camera_controller.apply_movement(CameraMovement::Forward(pressed));
                    }
                    PhysicalKey::Code(KeyCode::KeyS | KeyCode::ArrowDown) => {
                        camera_controller.apply_movement(CameraMovement::Backward(pressed));
                    }
                    PhysicalKey::Code(KeyCode::KeyA | KeyCode::ArrowLeft) => {
                        camera_controller.apply_movement(CameraMovement::Left(pressed));
                    }
                    PhysicalKey::Code(KeyCode::KeyD | KeyCode::ArrowRight) => {
                        camera_controller.apply_movement(CameraMovement::Right(pressed));
                    }
                    PhysicalKey::Code(KeyCode::Space) => {
                        camera_controller.apply_movement(CameraMovement::Up(pressed));
                    }
                    PhysicalKey::Code(KeyCode::ShiftLeft | KeyCode::ShiftRight) => {
                        camera_controller.apply_movement(CameraMovement::Down(pressed));
                    }
                    _ => {
                        is_movement = false;
                    }
                }

                is_movement
            }

            WindowEvent::MouseInput { state, button, .. } => {
                let pressed = *state == ElementState::Pressed;
                match button {
                    MouseButton::Right => self.is_rmb_pressed = pressed,
                    MouseButton::Left => self.is_lmb_pressed = pressed,
                    _ => {}
                }
                false
            }

            WindowEvent::CursorMoved { position, .. } => {
                let mut consumed = false;
                let should_rotate = self.is_rmb_pressed || (self.is_alt_pressed && self.is_lmb_pressed);

                if should_rotate {
                    if let Some((last_x, last_y)) = self.last_cursor_pos {
                        let dx = (position.x - last_x) as f32;
                        let dy = (position.y - last_y) as f32;
                        camera_controller.apply_movement(CameraMovement::Rotate { dx, dy });
                    }
                    consumed = true;
                }
                self.last_cursor_pos = Some((position.x, position.y));
                consumed
            }

            WindowEvent::MouseWheel { delta, .. } => {
                match delta {
                    MouseScrollDelta::PixelDelta(pos) => {
                        let dx = pos.x as f32 * 0.5;
                        let dy = -pos.y as f32 * 0.5;
                        camera_controller.apply_movement(CameraMovement::Rotate { dx, dy });
                    }
                    MouseScrollDelta::LineDelta(x, y) => {
                        if x.abs() > 0.001 || (self.is_alt_pressed && y.abs() > 0.001) {
                            let dx = *x * 15.0;
                            let dy = -*y * 15.0;
                            camera_controller.apply_movement(CameraMovement::Rotate { dx, dy });
                        } else {
                            camera_controller.apply_movement(CameraMovement::Zoom(*y * 20.0));
                        }
                    }
                }
                false
            }

            WindowEvent::PinchGesture { delta, .. } => {
                camera_controller.apply_movement(CameraMovement::Zoom(*delta as f32 * 3000.0));
                false
            }

            _ => false,
        }
    }

    fn on_ui(
        &mut self,
        ctx: &egui::Context,
        world: &mut World,
        camera: &mut Camera,
        postprocess: &mut render::postprocess::PostProcessSettings,
        metrics: &engine::EngineMetrics,
        load_requests: &mut Vec<String>,
    ) {
        if !self.models_to_load.is_empty() {
            load_requests.append(&mut self.models_to_load);
        }

        let mut play_action = PlayControlAction::None;

        ui::draw_editor_ui(
            ctx,
            world,
            camera,
            &mut self.config.postprocess,
            metrics,
            load_requests,
            &mut self.selected_entity,
            &mut self.gizmo,
            &mut self.gizmo_mode,
            &mut self.gizmo_orientation,
            self.play_state,
            &mut play_action,
        );

        match play_action {
            PlayControlAction::Start => self.start_game(world),
            PlayControlAction::Simulate => self.start_simulate(world),
            PlayControlAction::Stop => self.stop_game(world),
            PlayControlAction::Pause => self.pause_game(),
            PlayControlAction::Resume => self.pause_game(),
            PlayControlAction::Step => self.step_frame(),
            PlayControlAction::None => {}
        }

        *postprocess = self.config.postprocess.clone();
    }

    fn is_in_play_mode(&self) -> bool {
        self.play_state == PlayState::Playing
    }
}