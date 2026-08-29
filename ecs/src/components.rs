use asset::AssetId;
use flecs_ecs::prelude::*;
use glam::{Mat4, Quat, Vec3};
use rkyv::{Archive, Deserialize, Serialize};

// --- Transform Components ---

#[derive(Component, Debug, Clone, Copy, PartialEq)]
pub struct Transform {
    pub position: Vec3,
    pub rotation: Quat,
    pub scale: Vec3,
}

impl Default for Transform {
    fn default() -> Self {
        Self {
            position: Vec3::ZERO,
            rotation: Quat::IDENTITY,
            scale: Vec3::ONE,
        }
    }
}

impl Transform {
    pub fn from_translation(position: Vec3) -> Self {
        Self {
            position,
            rotation: Quat::IDENTITY,
            scale: Vec3::ONE,
        }
    }

    pub fn from_xyz(x: f32, y: f32, z: f32) -> Self {
        Self::from_translation(Vec3::new(x, y, z))
    }

    #[inline]
    pub fn compute_matrix(&self) -> Mat4 {
        Mat4::from_scale_rotation_translation(self.scale, self.rotation, self.position)
    }
}

#[derive(Component, Debug, Clone, Copy, PartialEq, Default)]
pub struct GlobalTransform(pub Mat4);

impl GlobalTransform {
    #[inline]
    pub fn matrix(&self) -> Mat4 {
        self.0
    }

    #[inline]
    pub fn translation(&self) -> Vec3 {
        self.0.w_axis.truncate()
    }
}

// --- Render Components ---

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
    pub inner_angle: f32,
    pub outer_angle: f32,
}

// --- Tags & Gameplay Markers ---

#[derive(Component, Debug, Clone, Copy, Default)]
pub struct Static;

#[derive(Component, Debug, Clone, Copy, Default)]
pub struct Dynamic;

#[derive(Component, Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct SceneEntity;