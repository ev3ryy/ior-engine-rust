mod settings;
pub mod ui;

pub use engine::game::PlayControlAction;
pub use settings::EditorConfig;
pub use ui::asset_browser::AssetBrowserState;

use ecs::serialization::{load_world, save_world};
use ecs::World;
use engine::app::Application;
use engine::camera::{Camera, CameraController, CameraMovement};
use engine::game::PlayState;
use transform_gizmo_egui::prelude::*;
use ui::dock::TabKind;
use winit::event::{ElementState, MouseButton, MouseScrollDelta, WindowEvent};
use winit::keyboard::{KeyCode, PhysicalKey};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EditorGizmoMode {
    Translate,
    Rotate,
    Scale,
    All,
}

pub struct Editor {
    pub config: EditorConfig,
    pub dock_state: egui_dock::DockState<TabKind>,
    pub asset_browser_state: AssetBrowserState,
    pub models_to_load: Vec<String>,
    pub is_rmb_pressed: bool,
    pub is_lmb_pressed: bool,
    pub is_alt_pressed: bool,
    pub last_cursor_pos: Option<(f64, f64)>,
    pub selected_entity: Option<u64>,
    pub gizmo: Gizmo,
    pub gizmo_mode: EditorGizmoMode,
    pub gizmo_orientation: GizmoOrientation,
    pub play_state: PlayState,
}

impl Editor {
    pub fn new(config: EditorConfig) -> Self {
        Self {
            config,
            dock_state: TabKind::load_dock_state(),
            asset_browser_state: AssetBrowserState::default(),
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
        }
    }
}

impl Application for Editor {
    fn on_init(
        &mut self,
        _world: &mut World,
        camera: &mut Camera,
        _asset_server: &std::sync::Arc<asset::AssetServer>,
    ) {
        self.config = EditorConfig::load("editor.cfg");
        camera.eye = self.config.camera_eye;
        camera.target = self.config.camera_target;
        camera.fovy = self.config.camera_fovy;
        self.selected_entity = None;

        log::info!("Editor initialized.");
    }

    fn on_update(
        &mut self,
        _world: &mut World,
        _camera: &mut Camera,
        _camera_controller: &mut CameraController,
    ) {
    }

    fn on_window_event(
        &mut self,
        world: &mut World,
        _camera: &mut Camera,
        camera_controller: &mut CameraController,
        event: &WindowEvent,
    ) -> bool {
        if self.play_state == PlayState::Playing {
            return false;
        }

        match event {
            WindowEvent::KeyboardInput {
                event:
                    winit::event::KeyEvent {
                        physical_key,
                        state: key_state,
                        repeat,
                        ..
                    },
                ..
            } => {
                let pressed = key_state.is_pressed();

                if matches!(
                    physical_key,
                    PhysicalKey::Code(KeyCode::AltLeft | KeyCode::AltRight)
                ) {
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
                            TabKind::save_dock_state(&self.dock_state);
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
                let should_rotate =
                    self.is_rmb_pressed || (self.is_alt_pressed && self.is_lmb_pressed);

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
        play_state: engine::game::PlayState,
        asset_server: &std::sync::Arc<asset::AssetServer>,
        viewport_texture_id: Option<egui::TextureId>,
        desired_viewport_size: &mut Option<(u32, u32)>,
    ) -> engine::game::PlayControlAction {
        self.play_state = play_state;

        if !self.models_to_load.is_empty() {
            load_requests.append(&mut self.models_to_load);
        }

        let mut play_action = engine::game::PlayControlAction::None;

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
            play_state,
            &mut play_action,
            asset_server,
            &mut self.dock_state,
            &self.config.world_save_path,
            viewport_texture_id,
            desired_viewport_size,
            &mut self.asset_browser_state,
        );

        *postprocess = self.config.postprocess.clone();

        play_action
    }

    fn selected_entity(&self) -> Option<u64> {
        self.selected_entity
    }
}