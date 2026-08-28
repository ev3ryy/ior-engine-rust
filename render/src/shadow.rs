use glam::{Mat4, Vec3, Vec4};

pub const SHADOW_MAP_SIZE: u32 = 2048;
pub const CASCADE_COUNT: usize = 4;

const SHADOW_CASTER_MARGIN: f32 = 100.0;
const SHADOW_Z_MARGIN: f32 = 10.0;
const CASCADE_BLEND_FRACTION: f32 = 0.15;

pub const MAX_SHADOW_DISTANCE: f32 = 180.0;
pub const SPLIT_LAMBDA: f32 = 0.55;

#[repr(C)]
#[derive(Debug, Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
pub struct ShadowUniform {
    pub light_space_matrices: [Mat4; CASCADE_COUNT],  // 256 byte
    pub cascade_splits: [Vec4; 1],                    // 16 byte
    pub cascade_texel_sizes: [Vec4; 1],               // 16 byte
}

pub fn calculate_cascade_splits(
    z_near: f32,
    z_far: f32,
    max_distance: f32,
    lambda: f32,
) -> [f32; CASCADE_COUNT + 1] {
    let mut splits = [0.0; CASCADE_COUNT + 1];
    splits[0] = z_near;

    let shadow_far = z_far.min(max_distance).max(z_near + 1.0);
    let ratio = shadow_far / z_near;
    let range = shadow_far - z_near;

    for i in 1..=CASCADE_COUNT {
        let p = i as f32 / CASCADE_COUNT as f32;
        let log_split = z_near * ratio.powf(p);
        let uniform_split = z_near + range * p;
        splits[i] = lambda * log_split + (1.0 - lambda) * uniform_split;
    }

    splits
}

pub fn calculate_cascade_matrices(
    camera_view: Mat4,
    fov_y_radians: f32,
    aspect_ratio: f32,
    light_dir: Vec3,
    splits: [f32; 5],
) -> ShadowUniform {
    let mut matrices = [Mat4::IDENTITY; CASCADE_COUNT];
    let mut cascade_splits = Vec4::ZERO;
    let mut cascade_texel_sizes = Vec4::ZERO;

    let camera_inv = camera_view.inverse();
    let light_dir = light_dir.normalize();
    let tan_half_fov = (fov_y_radians * 0.5).tan();
    let up = if light_dir.y.abs() > 0.99 { Vec3::Z } else { Vec3::Y };

    for cascade in 0..CASCADE_COUNT {
        let cascade_near = splits[cascade];
        let cascade_far = splits[cascade + 1];
        let cascade_length = cascade_far - cascade_near;

        let fit_near = if cascade == 0 { cascade_near } else { (cascade_near - cascade_length * CASCADE_BLEND_FRACTION).max(splits[0]) };
        let fit_far = if cascade == CASCADE_COUNT - 1 { cascade_far } else { cascade_far + cascade_length * CASCADE_BLEND_FRACTION };

        let near_h = 2.0 * tan_half_fov * fit_near;
        let near_w = near_h * aspect_ratio;
        let far_h = 2.0 * tan_half_fov * fit_far;
        let far_w = far_h * aspect_ratio;

        let corners_view = [
            Vec3::new(-near_w * 0.5, -near_h * 0.5, -fit_near), Vec3::new( near_w * 0.5, -near_h * 0.5, -fit_near),
            Vec3::new(-near_w * 0.5,  near_h * 0.5, -fit_near), Vec3::new( near_w * 0.5,  near_h * 0.5, -fit_near),
            Vec3::new(-far_w * 0.5, -far_h * 0.5, -fit_far), Vec3::new( far_w * 0.5, -far_h * 0.5, -fit_far),
            Vec3::new(-far_w * 0.5,  far_h * 0.5, -fit_far), Vec3::new( far_w * 0.5,  far_h * 0.5, -fit_far),
        ];

        let mut center_vs = Vec3::ZERO;
        for corner in &corners_view {
            center_vs += *corner;
        }
        center_vs /= 8.0;

        let mut radius = 0.0f32;
        for corner in &corners_view {
            radius = radius.max(corner.distance(center_vs));
        }
        radius = (radius * 16.0).ceil() / 16.0;

        let center = camera_inv.transform_point3(center_vs);

        let mut corners_world = [Vec3::ZERO; 8];
        for (i, corner) in corners_view.iter().enumerate() {
            corners_world[i] = camera_inv.transform_point3(*corner);
        }

        let light_view_base = glam::camera::rh::view::look_at_mat4(Vec3::ZERO, light_dir, up);

        let world_units_per_texel = (radius * 2.0) / SHADOW_MAP_SIZE as f32;

        let center_ls = light_view_base.transform_point3(center);

        let snapped_x = (center_ls.x / world_units_per_texel).round() * world_units_per_texel;
        let snapped_y = (center_ls.y / world_units_per_texel).round() * world_units_per_texel;
        let snapped_center_ls = Vec3::new(snapped_x, snapped_y, center_ls.z);

        let snapped_center = light_view_base.inverse().transform_point3(snapped_center_ls);

        let light_distance = radius + SHADOW_CASTER_MARGIN;
        let light_pos = snapped_center - light_dir * light_distance;
        let light_view = glam::camera::rh::view::look_at_mat4(light_pos, snapped_center, up);

        let mut min_z = f32::INFINITY;
        let mut max_z = f32::NEG_INFINITY;
        for corner in corners_world {
            let p = light_view.transform_point3(corner);
            min_z = min_z.min(p.z);
            max_z = max_z.max(p.z);
        }

        let near_plane = 0.1;
        let far_plane = (-min_z + SHADOW_CASTER_MARGIN + SHADOW_Z_MARGIN).max(near_plane + 1.0);

        let light_proj = glam::camera::rh::proj::directx::orthographic(
            -radius, radius, -radius, radius, near_plane, far_plane,
        );

        matrices[cascade] = light_proj * light_view;
        match cascade {
            0 => {
                cascade_splits.x = cascade_far;
                cascade_texel_sizes.x = world_units_per_texel;
            }
            1 => {
                cascade_splits.y = cascade_far;
                cascade_texel_sizes.y = world_units_per_texel;
            }
            2 => {
                cascade_splits.z = cascade_far;
                cascade_texel_sizes.z = world_units_per_texel;
            }
            3 => {
                cascade_splits.w = cascade_far;
                cascade_texel_sizes.w = world_units_per_texel;
            }
            _ => unreachable!(),
        }
    }

    ShadowUniform {
        light_space_matrices: matrices,
        cascade_splits: [cascade_splits],
        cascade_texel_sizes: [cascade_texel_sizes],
    }
}