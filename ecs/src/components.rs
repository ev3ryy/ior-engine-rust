use asset::AssetId;
use flecs_ecs::prelude::*;
use glam::{Mat4, Quat, Vec3};
use serde::{Deserialize, Serialize};

// --- Camera Component ---

#[derive(Component, Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct CameraComponent {
    #[serde(default = "default_fovy")]
    pub fovy: f32,
    #[serde(default = "default_znear")]
    pub znear: f32,
    #[serde(default = "default_zfar")]
    pub zfar: f32,
    #[serde(default = "default_true")]
    pub is_active: bool,
    #[serde(default)]
    pub priority: i32,
}

fn default_fovy() -> f32 { 60.0 }
fn default_znear() -> f32 { 0.1 }
fn default_zfar() -> f32 { 1000.0 }
fn default_true() -> bool { true }

impl Default for CameraComponent {
    fn default() -> Self {
        Self {
            fovy: default_fovy(),
            znear: default_znear(),
            zfar: default_zfar(),
            is_active: default_true(),
            priority: 0,
        }
    }
}

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

// --- Physics Components (Rapier 3D) ---

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum BodyType {
    Dynamic,
    Fixed,
    KinematicPositionBased,
    KinematicVelocityBased,
}

impl Default for BodyType {
    fn default() -> Self {
        Self::Dynamic
    }
}

#[derive(Component, Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct RigidBodyComponent {
    #[serde(default)]
    pub body_type: BodyType,
    #[serde(default = "default_mass")]
    pub mass: f32,
    #[serde(default = "default_damping")]
    pub linear_damping: f32,
    #[serde(default = "default_damping")]
    pub angular_damping: f32,
    #[serde(default = "default_gravity_scale")]
    pub gravity_scale: f32,
    #[serde(default)]
    pub ccd_enabled: bool,
    #[serde(default)]
    pub lock_rotations: bool,
}

fn default_mass() -> f32 { 1.0 }
fn default_damping() -> f32 { 0.05 }
fn default_gravity_scale() -> f32 { 1.0 }

impl Default for RigidBodyComponent {
    fn default() -> Self {
        Self {
            body_type: BodyType::Dynamic,
            mass: default_mass(),
            linear_damping: default_damping(),
            angular_damping: default_damping(),
            gravity_scale: default_gravity_scale(),
            ccd_enabled: false,
            lock_rotations: false,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ColliderShapeType {
    Cuboid,
    Sphere,
    Capsule,
    Cylinder,
}

impl Default for ColliderShapeType {
    fn default() -> Self {
        Self::Cuboid
    }
}

#[derive(Component, Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct ColliderComponent {
    #[serde(default)]
    pub shape_type: ColliderShapeType,
    #[serde(default = "default_collider_size")]
    pub size: [f32; 3], // half-extents для Cuboid, x = radius для Sphere, [x=radius, y=half_height] для Capsule/Cylinder
    #[serde(default = "default_friction")]
    pub friction: f32,
    #[serde(default = "default_restitution")]
    pub restitution: f32,
    #[serde(default)]
    pub is_sensor: bool,
}

fn default_collider_size() -> [f32; 3] { [0.5, 0.5, 0.5] }
fn default_friction() -> f32 { 0.5 }
fn default_restitution() -> f32 { 0.0 }

impl Default for ColliderComponent {
    fn default() -> Self {
        Self {
            shape_type: ColliderShapeType::Cuboid,
            size: default_collider_size(),
            friction: default_friction(),
            restitution: default_restitution(),
            is_sensor: false,
        }
    }
}

// --- Render Components ---

#[derive(Component, Debug, Clone, PartialEq, Eq)]
pub struct ModelComponent {
    pub id: AssetId,
    pub mesh_index: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[repr(u32)]
pub enum LightType {
    Directional = 0,
    Point = 1,
    Spot = 2,
}

impl Default for LightType {
    fn default() -> Self {
        Self::Point
    }
}

#[derive(Component, Debug, Clone, Serialize, Deserialize)]
pub struct LightComponent {
    #[serde(default)]
    pub light_type: LightType,
    #[serde(default = "default_light_color")]
    pub color: [f32; 3],
    #[serde(default = "default_intensity")]
    pub intensity: f32,
    #[serde(default = "default_range")]
    pub range: f32,
    #[serde(default)]
    pub inner_angle: f32,
    #[serde(default)]
    pub outer_angle: f32,
}

fn default_light_color() -> [f32; 3] { [1.0, 1.0, 1.0] }
fn default_intensity() -> f32 { 10.0 }
fn default_range() -> f32 { 15.0 }

impl Default for LightComponent {
    fn default() -> Self {
        Self {
            light_type: LightType::Point,
            color: default_light_color(),
            intensity: default_intensity(),
            range: default_range(),
            inner_angle: 0.0,
            outer_angle: 0.0,
        }
    }
}

// --- Tags & Gameplay Markers ---

#[derive(Component, Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EntityName(pub String);

#[derive(Component, Debug, Clone, Copy, Default)]
pub struct Static;

#[derive(Component, Debug, Clone, Copy, Default)]
pub struct Dynamic;

#[derive(Component, Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct SceneEntity;