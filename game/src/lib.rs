pub mod components;

use components::{Health, Player};
use ecs::{Transform, World};
use engine::game::{GameInput, GamePlugin, KeyCode};
use engine::physics::PhysicsWorld;

pub struct MyGamePlugin;

impl GamePlugin for MyGamePlugin {
    fn init(&mut self, world: &World) {
        world.register_serializable::<Player>("Player");
        world.register_serializable::<Health>("Health");

        log::info!("[GAME] RpgGamePlugin initialized, components registered.");
    }

    fn on_start(&mut self, _world: &World, _physics: &mut PhysicsWorld) {
        log::info!("[GAME] Game Play Mode started!");
    }

    fn on_stop(&mut self, _world: &World) {
        log::info!("[GAME] Game Play Mode stopped.");
    }

    fn on_fixed_update(
        &mut self,
        world: &World,
        _physics: &mut PhysicsWorld,
        input: &GameInput,
        fixed_dt: f32,
    ) {
        world.query2_mut1::<Transform, Player, _>(|_entity, transform, player| {
            if input.move_axis.length_squared() > 0.001 {
                let forward = transform.rotation * -glam::Vec3::Z;
                let right = transform.rotation * glam::Vec3::X;

                let move_dir = (right * input.move_axis.x + forward * input.move_axis.y).normalize();
                transform.position += move_dir * player.move_speed * fixed_dt;
            }
        });
    }

    fn on_update(
        &mut self,
        world: &World,
        _physics: &mut PhysicsWorld,
        input: &GameInput,
        _dt: f32,
    ) {
        if input.is_key_just_pressed(KeyCode::KeyH) {
            world.query1_mut::<Health, _>(|_entity, health| {
                health.current = (health.current - 10.0).max(0.0);
                log::info!("[GAME] Player took damage! Current HP: {}", health.current);
            });
        }
    }
}