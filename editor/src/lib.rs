mod settings;
pub mod ui;
pub use settings::EditorConfig;


use engine::app::Application;
use engine::camera::{Camera, CameraController, CameraMovement};
use ecs::World;
use ecs::serialization::{save_world, load_world};
use winit::event::{WindowEvent, KeyEvent, ElementState, MouseButton, MouseScrollDelta};
use winit::keyboard::{KeyCode, PhysicalKey};

pub struct Editor {
    config: EditorConfig,
    models_to_load: Vec<String>,
    is_rmb_pressed: bool,
    last_cursor_pos: Option<(f64, f64)>,
}

impl Editor {
    pub fn new(config: EditorConfig) -> Self {
        Self { 
            config, 
            models_to_load: Vec::new(),
            is_rmb_pressed: false,
            last_cursor_pos: None,
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
                event: KeyEvent { physical_key: PhysicalKey::Code(code), state: key_state, .. },
                ..
            } => {
                let pressed = key_state.is_pressed();
                
                if *code == KeyCode::Escape && pressed {
                    std::process::exit(0);
                }
                
                let movement = match code {
                    KeyCode::Space => Some(CameraMovement::Up(pressed)),
                    KeyCode::ShiftLeft => Some(CameraMovement::Down(pressed)),
                    KeyCode::KeyW | KeyCode::ArrowUp => Some(CameraMovement::Forward(pressed)),
                    KeyCode::KeyS | KeyCode::ArrowDown => Some(CameraMovement::Backward(pressed)),
                    KeyCode::KeyA | KeyCode::ArrowLeft => Some(CameraMovement::Left(pressed)),
                    KeyCode::KeyD | KeyCode::ArrowRight => Some(CameraMovement::Right(pressed)),
                    _ => None,
                };
                
                if let Some(mov) = movement {
                    camera_controller.apply_movement(mov);
                }
                
                if pressed && *code == KeyCode::F5 {
                    if let Err(e) = save_world(world, &self.config.world_save_path) {
                        log::error!("Failed to save world: {}", e);
                    } else {
                        log::info!("World saved to {}", self.config.world_save_path);
                    }

                    self.config.save("editor.cfg");
                }

                if pressed && *code == KeyCode::F9 {
                    *world = ecs::create_world();
                    
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

    fn on_ui(&mut self, ctx: &egui::Context, world: &mut World, camera: &mut Camera, metrics: &engine::EngineMetrics, load_requests: &mut Vec<String>) {
        if !self.models_to_load.is_empty() {
            load_requests.append(&mut self.models_to_load);
        }

        ui::draw_editor_ui(ctx, world, camera, metrics, load_requests);
    }
}