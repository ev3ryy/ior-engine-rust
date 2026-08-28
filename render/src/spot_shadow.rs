// render/src/spot_shadow.rs

use glam::{Mat4, Vec3};

pub const MAX_SPOT_SHADOWS: usize = 8;
pub const SPOT_SHADOW_MAP_SIZE: u32 = 1024;
pub const SPOT_NEAR_PLANE: f32 = 0.05;

#[repr(C)]
#[derive(Debug, Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
pub struct SpotShadowUniform {
    pub shadow_matrices: [Mat4; MAX_SPOT_SHADOWS], // 512 bytes (64 * 8)
}

pub fn calculate_spot_matrix(
    position: Vec3,
    direction: Vec3,
    outer_cone_cos: f32,
    range: f32,
) -> Mat4 {
    let dir = if direction.length_squared() < 1e-6 {
        Vec3::NEG_Y
    } else {
        direction.normalize()
    };
    
    let up = if dir.y.abs() > 0.99 {
        Vec3::Z
    } else {
        Vec3::Y
    };

    let view = glam::camera::rh::view::look_at_mat4(position, position + dir, up);
    let half_angle = outer_cone_cos.clamp(-1.0, 0.9999).acos();
    let fov_y = (half_angle * 2.0).clamp(0.01, std::f32::consts::PI - 0.01);
    let proj = glam::camera::rh::proj::directx::perspective(fov_y, 1.0, SPOT_NEAR_PLANE, range.max(0.1));

    proj * view
}