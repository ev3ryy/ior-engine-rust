use ecs::World;
use crate::camera::{Camera, CameraController};
use winit::{event::WindowEvent};

pub trait Application {
    fn on_init(&mut self, _world: &mut World, _camera: &mut Camera) {}
    
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
}