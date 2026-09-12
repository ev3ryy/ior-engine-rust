use asset::AssetId;
use flecs_ecs::prelude::*;
use glam::{Mat4, Quat, Vec3};
use serde::{Deserialize, Serialize};

pub struct RenderLayers;

impl RenderLayers {
    pub const DEFAULT: u32   = 1 << 0;
    pub const VIEWMODEL: u32 = 1 << 1;
    pub const UI_3D: u32     = 1 << 2;
    pub const ALL: u32       = u32::MAX;
}


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

    #[serde(default = "default_u32_max")]
    pub culling_mask: u32,
    #[serde(default)]
    pub clear_depth: bool, 
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
            culling_mask: u32::MAX,
            clear_depth: false,
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
    TriMesh,
    ConvexHull,
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
    pub size: [f32; 3],
    #[serde(default = "default_friction")]
    pub friction: f32,
    #[serde(default = "default_restitution")]
    pub restitution: f32,
    #[serde(default)]
    pub is_sensor: bool,

    #[serde(default = "default_vec3_zero")]
    pub offset_position: [f32; 3],
    #[serde(default = "default_quat_identity")]
    pub offset_rotation: [f32; 4], // [x, y, z, w]

    #[serde(default = "default_u32_max")]
    pub collision_memberships: u32,
    #[serde(default = "default_u32_max")]
    pub collision_filters: u32,

    #[serde(default = "default_true")]
    pub generate_collision_events: bool,
}

fn default_collider_size() -> [f32; 3] { [0.5, 0.5, 0.5] }
fn default_friction() -> f32 { 0.5 }
fn default_restitution() -> f32 { 0.0 }
fn default_vec3_zero() -> [f32; 3] { [0.0, 0.0, 0.0] }
fn default_quat_identity() -> [f32; 4] { [0.0, 0.0, 0.0, 1.0] }
fn default_u32_max() -> u32 { u32::MAX }

impl Default for ColliderComponent {
    fn default() -> Self {
        Self {
            shape_type: ColliderShapeType::Cuboid,
            size: default_collider_size(),
            friction: default_friction(),
            restitution: default_restitution(),
            is_sensor: false,
            offset_position: default_vec3_zero(),
            offset_rotation: default_quat_identity(),
            collision_memberships: default_u32_max(),
            collision_filters: default_u32_max(),
            generate_collision_events: true,
        }
    }
}

#[derive(Component, Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct CharacterControllerComponent {
    #[serde(default = "default_autostep_height")]
    pub autostep_max_height: f32,
    #[serde(default = "default_autostep_width")]
    pub autostep_min_width: f32,
    #[serde(default = "default_max_slope")]
    pub max_slope_climb_angle: f32,
    #[serde(default = "default_min_slide")]
    pub min_slope_slide_angle: f32,
    #[serde(default = "default_snap_distance")]
    pub snap_to_ground_distance: f32,
    #[serde(default = "default_true")]
    pub enable_autostep: bool,
    #[serde(default = "default_true")]
    pub enable_snap_to_ground: bool,
    #[serde(default = "default_true")]
    pub slide: bool,
}

fn default_autostep_height() -> f32 { 0.35 }
fn default_autostep_width() -> f32 { 0.2 }
fn default_max_slope() -> f32 { 45.0 }
fn default_min_slide() -> f32 { 45.0 }
fn default_snap_distance() -> f32 { 0.25 }

impl Default for CharacterControllerComponent {
    fn default() -> Self {
        Self {
            autostep_max_height: default_autostep_height(),
            autostep_min_width: default_autostep_width(),
            max_slope_climb_angle: default_max_slope(),
            min_slope_slide_angle: default_min_slide(),
            snap_to_ground_distance: default_snap_distance(),
            enable_autostep: true,
            enable_snap_to_ground: true,
            slide: true,
        }
    }
}

// --- Render Components ---

#[derive(Component, Debug, Clone, PartialEq, Eq)]
pub struct ModelComponent {
    pub id: AssetId,
    pub mesh_index: usize,
    pub render_layer: u32,
}

impl ModelComponent {
    pub fn new(id: AssetId, mesh_index: usize) -> Self {
        Self {
            id,
            mesh_index,
            render_layer: RenderLayers::DEFAULT,
        }
    }

    pub fn with_layer(id: AssetId, mesh_index: usize, render_layer: u32) -> Self {
        Self {
            id,
            mesh_index,
            render_layer,
        }
    }
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

#[derive(Component, Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PrefabRef {
    pub path: String,
}

impl Default for PrefabRef {
    fn default() -> Self {
        Self {
            path: String::new(),
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