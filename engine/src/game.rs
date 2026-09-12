use crate::physics::{PhysicsCollisionEvent, PhysicsWorld};
use ecs::World;
use std::collections::HashSet;

pub use winit::event::MouseButton;
pub use winit::keyboard::KeyCode;

pub use crate::physics::{CollisionEventType, RayHit, RaycastFilter};

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
    #[inline] pub fn is_key_down(&self, code: KeyCode) -> bool { self.keys_pressed.contains(&code) }
    #[inline] pub fn is_key_just_pressed(&self, code: KeyCode) -> bool { self.keys_just_pressed.contains(&code) }
    #[inline] pub fn is_mouse_down(&self, button: MouseButton) -> bool { self.mouse_pressed.contains(&button) }
    #[inline] pub fn is_mouse_just_pressed(&self, button: MouseButton) -> bool { self.mouse_just_pressed.contains(&button) }
    #[inline] pub fn is_jump(&self) -> bool { self.is_key_just_pressed(KeyCode::Space) }
    #[inline] pub fn is_sprint(&self) -> bool { self.is_key_down(KeyCode::ShiftLeft) || self.is_key_down(KeyCode::ShiftRight) }
    #[inline] pub fn is_crouch(&self) -> bool { self.is_key_down(KeyCode::ControlLeft) || self.is_key_down(KeyCode::KeyC) }
    #[inline] pub fn is_shoot(&self) -> bool { self.is_mouse_down(MouseButton::Left) }
    #[inline] pub fn is_shoot_just_pressed(&self) -> bool { self.is_mouse_just_pressed(MouseButton::Left) }
    #[inline] pub fn is_aim(&self) -> bool { self.is_mouse_down(MouseButton::Right) }
    #[inline] pub fn is_reload(&self) -> bool { self.is_key_just_pressed(KeyCode::KeyR) }
    #[inline] pub fn is_interact(&self) -> bool { self.is_key_just_pressed(KeyCode::KeyE) }

    pub fn clear_frame_deltas(&mut self) {
        self.look_delta = glam::Vec2::ZERO;
        self.keys_just_pressed.clear();
        self.mouse_just_pressed.clear();
    }
}

pub trait GamePlugin: Send + Sync + 'static {
    fn init(&mut self, world: &World);

    #[allow(unused_variables)]
    fn on_start(&mut self, world: &World, physics: &mut PhysicsWorld) {}

    fn on_stop(&mut self, world: &World);

    #[allow(unused_variables)]
    fn on_collision(&mut self, world: &World, event: &PhysicsCollisionEvent) {}

    #[allow(unused_variables)]
    fn on_fixed_update(
        &mut self,
        world: &World,
        physics: &mut PhysicsWorld,
        input: &GameInput,
        fixed_dt: f32,
    ) {}

    #[allow(unused_variables)]
    fn on_update(
        &mut self,
        world: &World,
        physics: &mut PhysicsWorld,
        input: &GameInput,
        dt: f32,
    ) {}

    #[allow(unused_variables)]
    fn on_late_update(
        &mut self,
        world: &World,
        physics: &mut PhysicsWorld,
        input: &GameInput,
        dt: f32,
    ) {}
}

pub struct DefaultGamePlugin;

impl GamePlugin for DefaultGamePlugin {
    fn init(&mut self, _world: &World) {}
    fn on_start(&mut self, _world: &World, _physics: &mut PhysicsWorld) {}
    fn on_stop(&mut self, _world: &World) {}
    fn on_collision(&mut self, _world: &World, _event: &PhysicsCollisionEvent) {}
    fn on_fixed_update(&mut self, _world: &World, _physics: &mut PhysicsWorld, _input: &GameInput, _fixed_dt: f32) {}
    fn on_update(&mut self, _world: &World, _physics: &mut PhysicsWorld, _input: &GameInput, _dt: f32) {}
    fn on_late_update(&mut self, _world: &World, _physics: &mut PhysicsWorld, _input: &GameInput, _dt: f32) {}
}