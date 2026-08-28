mod settings;
pub mod ui;
pub use settings::EditorConfig;

use engine::app::Application;
use engine::camera::{Camera, CameraController, CameraMovement};
use ecs::World;
use ecs::serialization::{save_world, load_world};
use winit::event::{WindowEvent, KeyEvent, ElementState, MouseButton, MouseScrollDelta};
use winit::keyboard::{KeyCode, PhysicalKey, Key, NamedKey};
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
    last_cursor_pos: Option<(f64, f64)>,
    pub selected_entity: Option<u64>,
    pub gizmo: Gizmo,
    pub gizmo_mode: EditorGizmoMode,
    pub gizmo_orientation: GizmoOrientation,
}

impl Editor {
    pub fn new(config: EditorConfig) -> Self {
        Self {
            config,
            models_to_load: Vec::new(),
            is_rmb_pressed: false,
            last_cursor_pos: None,
            selected_entity: None,
            gizmo: Gizmo::default(),
            gizmo_mode: EditorGizmoMode::Translate,
            gizmo_orientation: GizmoOrientation::Global,
        }
    }
}

impl Application for Editor {
    fn on_init(
        &mut self,
        _world: &mut World,
        camera: &mut Camera,
        _asset_server: &std::sync::Arc<asset::AssetServer>
    ) {
        self.config = EditorConfig::load("editor.cfg");
        camera.eye = self.config.camera_eye;
        camera.target = self.config.camera_target;
        camera.fovy = self.config.camera_fovy;
        self.selected_entity = None;
        log::info!("Editor initialized with empty world.");
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
                event: KeyEvent { physical_key, logical_key, state: key_state, .. },
                ..
            } => {
                let pressed = key_state.is_pressed();

                if matches!(physical_key, PhysicalKey::Code(KeyCode::Escape)) || matches!(logical_key, Key::Named(NamedKey::Escape)) {
                    if pressed {
                        if self.selected_entity.is_some() {
                            self.selected_entity = None;
                            return true;
                        }
                        //std::process::exit(0);
                    }
                }

                if pressed && !self.is_rmb_pressed {
                    let is_key_e = matches!(physical_key, PhysicalKey::Code(KeyCode::KeyE))
                        || matches!(logical_key, Key::Character(c) if c.eq_ignore_ascii_case("e") || c == "у" || c == "У");
                    let is_key_r = matches!(physical_key, PhysicalKey::Code(KeyCode::KeyR))
                        || matches!(logical_key, Key::Character(c) if c.eq_ignore_ascii_case("r") || c == "к" || c == "К");
                    let is_key_t = matches!(physical_key, PhysicalKey::Code(KeyCode::KeyT))
                        || matches!(logical_key, Key::Character(c) if c.eq_ignore_ascii_case("t") || c == "е" || c == "Е");
                    let is_key_q = matches!(physical_key, PhysicalKey::Code(KeyCode::KeyQ))
                        || matches!(logical_key, Key::Character(c) if c.eq_ignore_ascii_case("q") || c == "й" || c == "Й");

                    if is_key_e { self.gizmo_mode = EditorGizmoMode::Rotate; }
                    if is_key_r { self.gizmo_mode = EditorGizmoMode::Scale; }
                    if is_key_t { self.gizmo_mode = EditorGizmoMode::All; }
                    if is_key_q {
                        self.gizmo_orientation = match self.gizmo_orientation {
                            GizmoOrientation::Global => GizmoOrientation::Local,
                            GizmoOrientation::Local => GizmoOrientation::Global,
                        };
                    }
                }

                let is_forward = matches!(physical_key, PhysicalKey::Code(KeyCode::KeyW | KeyCode::ArrowUp))
                    || matches!(logical_key, Key::Character(c) if c.eq_ignore_ascii_case("w") || c == "ц" || c == "Ц")
                    || matches!(logical_key, Key::Named(NamedKey::ArrowUp));

                let is_backward = matches!(physical_key, PhysicalKey::Code(KeyCode::KeyS | KeyCode::ArrowDown))
                    || matches!(logical_key, Key::Character(c) if c.eq_ignore_ascii_case("s") || c == "ы" || c == "Ы")
                    || matches!(logical_key, Key::Named(NamedKey::ArrowDown));

                let is_left = matches!(physical_key, PhysicalKey::Code(KeyCode::KeyA | KeyCode::ArrowLeft))
                    || matches!(logical_key, Key::Character(c) if c.eq_ignore_ascii_case("a") || c == "ф" || c == "Ф")
                    || matches!(logical_key, Key::Named(NamedKey::ArrowLeft));

                let is_right = matches!(physical_key, PhysicalKey::Code(KeyCode::KeyD | KeyCode::ArrowRight))
                    || matches!(logical_key, Key::Character(c) if c.eq_ignore_ascii_case("d") || c == "в" || c == "В")
                    || matches!(logical_key, Key::Named(NamedKey::ArrowRight));

                let is_up = matches!(physical_key, PhysicalKey::Code(KeyCode::Space))
                    || matches!(logical_key, Key::Named(NamedKey::Space));

                let is_down = matches!(physical_key, PhysicalKey::Code(KeyCode::ShiftLeft | KeyCode::ShiftRight))
                    || matches!(logical_key, Key::Named(NamedKey::Shift));

                if is_forward { camera_controller.apply_movement(CameraMovement::Forward(pressed)); }
                if is_backward { camera_controller.apply_movement(CameraMovement::Backward(pressed)); }
                if is_left { camera_controller.apply_movement(CameraMovement::Left(pressed)); }
                if is_right { camera_controller.apply_movement(CameraMovement::Right(pressed)); }
                if is_up { camera_controller.apply_movement(CameraMovement::Up(pressed)); }
                if is_down { camera_controller.apply_movement(CameraMovement::Down(pressed)); }

                if pressed && matches!(physical_key, PhysicalKey::Code(KeyCode::F5)) {
                    if let Err(e) = save_world(world, &self.config.world_save_path) {
                        log::error!("Failed to save world: {}", e);
                    } else {
                        log::info!("World saved to {}", self.config.world_save_path);
                    }
                    self.config.save("editor.cfg");
                }

                if pressed && matches!(physical_key, PhysicalKey::Code(KeyCode::F9)) {
                    *world = ecs::create_world();
                    self.selected_entity = None;

                    if let Err(e) = load_world(world, &self.config.world_save_path) {
                        log::error!("Failed to load world: {}", e);
                    } else {
                        log::info!("World loaded from {}", self.config.world_save_path);
                        world.inner.each::<&ecs::components::ModelComponent>(|model_comp| {
                            self.models_to_load.push(model_comp.id.path());
                        });
                    }
                    self.config = EditorConfig::load("editor.cfg");
                }

                true
            }

            WindowEvent::MouseInput { state, button: MouseButton::Right, .. } => {
                self.is_rmb_pressed = *state == ElementState::Pressed;
                false
            }

            WindowEvent::CursorMoved { position, .. } => {
                let mut consumed = false;
                if self.is_rmb_pressed {
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
                    MouseScrollDelta::LineDelta(_x, y) => {
                        camera_controller.apply_movement(CameraMovement::Zoom(*y * 20.0));
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

    fn on_ui(&mut self, ctx: &egui::Context, world: &mut World, camera: &mut Camera, postprocess: &mut render::postprocess::PostProcessSettings, metrics: &engine::EngineMetrics, load_requests: &mut Vec<String>) {
        if !self.models_to_load.is_empty() {
            load_requests.append(&mut self.models_to_load);
        }

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
        );

        *postprocess = self.config.postprocess.clone();
    }
}