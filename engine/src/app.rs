use ecs::World;
use crate::camera::{Camera, CameraController};
use crate::EngineMetrics;
use winit::{event::WindowEvent};

pub trait Application {
    #[allow(unused_variables)]
    fn on_init(&mut self, _world: &mut World, _camera: &mut Camera, asset_server: &std::sync::Arc<asset::AssetServer>) {}
    
    fn on_update(&mut self, _world: &mut World, _camera: &mut Camera, _camera_controller: &mut CameraController) {}
    
    fn on_resize(&mut self, _width: u32, _height: u32) {}
    
    fn on_window_event(
        &mut self, 
        _world: &mut World, 
        _camera: &mut Camera, 
        _camera_controller: &mut CameraController, 
        _event: &WindowEvent
    ) -> bool {
        false
    }

    #[allow(unused_variables)]
    fn on_ui(&mut self, ctx: &egui::Context, world: &mut World, camera: &mut Camera, postprocess: &mut render::postprocess::PostProcessSettings, metrics: &EngineMetrics, load_requests: &mut Vec<String>) {}

    fn is_in_play_mode(&self) -> bool { false }
}