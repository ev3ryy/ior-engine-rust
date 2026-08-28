use glam::{Mat4, Vec3};

pub const MAX_POINT_SHADOWS: usize = 4;
pub const POINT_SHADOW_FACES: usize = 6;
pub const TOTAL_POINT_FACES: usize = MAX_POINT_SHADOWS * POINT_SHADOW_FACES;
pub const POINT_SHADOW_MAP_SIZE: u32 = 512;
pub const POINT_NEAR_PLANE: f32 = 0.05;

#[repr(C)]
#[derive(Debug, Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
pub struct PointShadowUniform {
    // x: near_plane, y: far_plane, z: bias, w: padding
    pub light_params: [[f32; 4]; MAX_POINT_SHADOWS],
}

/// 0: +X, 1: -X, 2: +Y, 3: -Y, 4: +Z, 5: -Z
pub fn calculate_cube_face_matrix(
    light_pos: Vec3,
    face_idx: usize,
    near: f32,
    far: f32,
) -> Mat4 {
    let (dir, up) = match face_idx {
        0 => (Vec3::X, Vec3::NEG_Y),    // +X (Right)
        1 => (Vec3::NEG_X, Vec3::NEG_Y),// -X (Left)
        2 => (Vec3::Y, Vec3::Z),        // +Y (Top)
        3 => (Vec3::NEG_Y, Vec3::NEG_Z),// -Y (Bottom)
        4 => (Vec3::Z, Vec3::NEG_Y),    // +Z (Front)
        5 => (Vec3::NEG_Z, Vec3::NEG_Y),// -Z (Back)
        _ => unreachable!("Invalid cube face index"),
    };

    let view = glam::camera::rh::view::look_at_mat4(light_pos, light_pos + dir, up);
    let mut proj = glam::camera::rh::proj::directx::perspective(std::f32::consts::FRAC_PI_2, 1.0, near, far);
    
    proj.y_axis.y *= -1.0;
    
    proj * view
}

pub fn calculate_shadow_score(
    cam_pos: Vec3,
    cam_frustum: &crate::Frustum,
    light_pos: Vec3,
    light_range: f32,
    light_intensity: f32,
    light_color: [f32; 3],
    previously_had_shadow: bool,
) -> f32 {
    if !cam_frustum.intersects_sphere(light_pos, light_range) {
        return 0.0;
    }

    let dist_sq = cam_pos.distance_squared(light_pos).max(0.01);
    let dist = dist_sq.sqrt();

    let color_max = light_color[0].max(light_color[1]).max(light_color[2]);
    let luminance = light_intensity * color_max;

    let projected_area = (light_range / dist.max(0.5)).powi(2);

    let center_inside = cam_frustum.intersects_sphere(light_pos, 0.0);
    let vis_factor = if center_inside { 1.0 } else { 0.3 };

    let hyst_factor = if previously_had_shadow { 1.3 } else { 1.0 };

    (luminance / dist_sq) * projected_area * vis_factor * hyst_factor
}