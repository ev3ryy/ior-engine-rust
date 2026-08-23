use flecs_ecs::prelude::*;
use glam::{Quat, Vec3, Mat4};
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

#[derive(Component, Debug, Clone, Copy, PartialEq)]
pub struct GlobalTransform(pub Mat4);

impl Default for GlobalTransform {
    fn default() -> Self {
        Self(Mat4::IDENTITY)
    }
}

// render
#[derive(Component, Debug, Clone, PartialEq, Eq, Archive, Serialize, Deserialize)]
pub struct ModelComponent {
    pub id: AssetId,
    pub mesh_index: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Archive, Serialize, Deserialize)]
#[repr(u32)]
pub enum LightType {
    Directional = 0,
    Point = 1,
    Spot = 2,
}

#[derive(Component, Debug, Clone, Archive, Serialize, Deserialize)]
pub struct LightComponent {
    pub light_type: LightType,
    pub color: [f32; 3],
    pub intensity: f32,
    pub range: f32,
    pub inner_angle: f32, // radians
    pub outer_angle: f32, // radians
}
// tags
#[derive(Component, Debug, Clone, Copy)]
pub struct Static; // entity doesn`t move

#[derive(Component, Debug, Clone, Copy)]
pub struct Dynamic; // entity updates every frame