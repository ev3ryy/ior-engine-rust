pub mod serialization;

use flecs_ecs::prelude::*;
use glam::{Quat, Vec3};

pub struct World {
    pub inner: flecs_ecs::core::World,
}

#[derive(Component, Debug, Clone, Copy)]
pub struct Position(pub Vec3);

#[derive(Component, Debug, Clone, Copy)]
pub struct Rotation(pub Quat);

pub fn create_world() -> World {
    World {
        inner: flecs_ecs::core::World::new(),
    }
}

pub fn spawn_instance_grid(world: &World, rows: u32, spacing: f32) {
    let half = (rows as f32 - 1.0) * 0.5;
    for z in 0..rows {
        for x in 0..rows {
            let px = spacing * (x as f32 - half);
            let pz = spacing * (z as f32 - half);
            world.inner
                .entity()
                .set(Position(Vec3::new(px, 0.0, pz)))
                .set(Rotation(Quat::IDENTITY));
        }
    }
}

pub struct InstanceData {
    pub position: Vec3,
    pub rotation: Quat,
}

pub fn collect_instance_data(world: &World) -> Vec<InstanceData> {
    let mut result = Vec::new();
    world.inner.each::<(&Position, &Rotation)>(|(pos, rot)| {
        result.push(InstanceData {
            position: pos.0,
            rotation: rot.0,
        });
    });
    result
}

pub fn collect_instance_positions(world: &World) -> Vec<Vec3> {
    let mut result = Vec::new();
    world.inner.each::<(&Position,)>(|(pos,)| {
        result.push(pos.0);
    });
    result
}