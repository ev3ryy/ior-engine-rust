use ecs::World;
use std::collections::HashSet;
use winit::event::MouseButton;
use winit::keyboard::KeyCode;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlayState {
    Editing,
    Playing,
    Simulating,
    Paused { step_frame: bool, was_simulating: bool },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlayControlAction {
    None,
    Start,
    Simulate,
    Stop,
    Pause,
    Resume,
    Step,
}

#[derive(Default, Debug, Clone)]
pub struct GameInput {
    pub move_axis: glam::Vec2,
    pub look_delta: glam::Vec2,
    pub keys_pressed: HashSet<KeyCode>,
    pub keys_just_pressed: HashSet<KeyCode>,
    pub mouse_pressed: HashSet<MouseButton>,
    pub mouse_just_pressed: HashSet<MouseButton>,
}

impl GameInput {
    pub fn is_key_down(&self, code: KeyCode) -> bool {
        self.keys_pressed.contains(&code)
    }

    pub fn is_key_just_pressed(&self, code: KeyCode) -> bool {
        self.keys_just_pressed.contains(&code)
    }

    pub fn clear_frame_deltas(&mut self) {
        self.look_delta = glam::Vec2::ZERO;
        self.keys_just_pressed.clear();
        self.mouse_just_pressed.clear();
    }
}

pub trait GamePlugin: Send + Sync + 'static {
    fn init(&mut self, world: &World);
    fn on_start(&mut self, world: &World);
    fn on_stop(&mut self, world: &World);
    fn on_update(&mut self, world: &World, input: &GameInput, dt: f32);
}

pub struct DefaultGamePlugin;

impl GamePlugin for DefaultGamePlugin {
    fn init(&mut self, _world: &World) {}
    fn on_start(&mut self, _world: &World) {}
    fn on_stop(&mut self, _world: &World) {}
    fn on_update(&mut self, _world: &World, _input: &GameInput, _dt: f32) {}
}