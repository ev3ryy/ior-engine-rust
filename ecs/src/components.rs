use flecs_ecs::prelude::*;
use glam::{Quat, Vec3};
use asset::AssetId;
use rkyv::{Archive, Deserialize, Serialize};

// transform
#[derive(Component, Debug, Clone, Copy, PartialEq)]
pub struct Position(pub Vec3);

#[derive(Component, Debug, Clone, Copy, PartialEq)]
pub struct Rotation(pub Quat);

#[derive(Component, Debug, Clone, Copy, PartialEq)]
pub struct Scale(pub Vec3);

impl Default for Scale {
    fn default() -> Self {
        Self(Vec3::ONE)
    }
}

// render
#[derive(Component, Debug, Clone, PartialEq, Eq, Archive, Serialize, Deserialize)]
pub struct ModelComponent {
    pub id: AssetId,
    pub mesh_index: usize,
}

// tags
#[derive(Component, Debug, Clone, Copy)]
pub struct Static; // entity doesn`t move

#[derive(Component, Debug, Clone, Copy)]
pub struct Dynamic; // entity updates every frame