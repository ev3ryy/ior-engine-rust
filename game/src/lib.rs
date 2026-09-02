use ecs::{flecs_ecs, Component, Inspectable, Transform, World};
use engine::game::{GameInput, GamePlugin};
use glam::Vec3;

#[derive(Component, Inspectable, Default, Clone)]
#[component(category = "Combat")]
pub struct Health {
    #[inspect(range = 0.0..=1000.0, speed = 1.0)]
    pub current: f32,

    #[inspect(range = 1.0..=1000.0)]
    pub max: f32,

    pub is_invincible: bool,
}

#[derive(Component, Inspectable, Default, Clone)]
#[component(category = "Movement")]
pub struct Rotator {
    #[inspect(range = -10.0..=10.0, speed = 0.1)]
    pub speed: f32,
}

#[derive(Component, Inspectable, Default, Clone)]
#[component(category = "Gameplay")]
pub struct PlayerController {
    #[inspect(range = 0.1..=50.0)]
    pub move_speed: f32,

    #[inspect(color)]
    pub marker_color: [f32; 3],

    pub spawn_point: Vec3,
    pub tag: String,
}

#[derive(Default)]
pub struct MyGamePlugin;

impl GamePlugin for MyGamePlugin {
    fn init(&mut self, world: &World) {
        log::info!("[GAME] init() started!");

        world.register_component::<Health>();
        world.register_component::<Rotator>();
        world.register_component::<PlayerController>();

        log::info!(
            "[GAME] Registered components! Total in registry: {}",
            world.get_component_entries().len()
        );
    }

    fn on_start(&mut self, _world: &World) {
        log::info!("[GAME] Game Play mode started!");
    }

    fn on_stop(&mut self, _world: &World) {
        log::info!("[GAME] Game Play mode stopped!");
    }

    fn on_update(&mut self, world: &World, _input: &GameInput, dt: f32) {
        world.each::<(&mut Transform, &Rotator)>(|(transform, rotator)| {
            transform.rotation *= glam::Quat::from_rotation_y(rotator.speed * dt);
        });
    }
}