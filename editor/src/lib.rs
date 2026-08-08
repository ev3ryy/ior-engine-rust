mod settings;
pub use settings::EditorConfig;


use engine::app::Application;
use engine::camera::{Camera, CameraController, CameraMovement};
use ecs::{spawn_instance_grid, World};
use ecs::serialization::{save_world, load_world};
use winit::event::{WindowEvent, KeyEvent};
use winit::keyboard::{KeyCode, PhysicalKey};

pub struct Editor {
    config: EditorConfig,
}

impl Editor {
    pub fn new(config: EditorConfig) -> Self {
        Self { config }
    }
}

impl Application for Editor {
    fn on_init(&mut self, world: &mut World, camera: &mut Camera) {
        self.config = EditorConfig::load("editor.cfg");
        spawn_instance_grid(world, self.config.initial_grid_rows, self.config.initial_grid_spacing);
        
        camera.eye = self.config.camera_eye;
        camera.target = self.config.camera_target;
        camera.fovy = self.config.camera_fovy;
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
                    }
                }
                
                true
            }
            _ => false,
        }
    }
}