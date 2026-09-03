use glam::{camera, Mat4, Vec2, Vec3, Vec4};
use render::CameraUniform;

#[derive(Debug, Clone, Copy)]
pub struct Ray {
    pub origin: Vec3,
    pub direction: Vec3,
}

impl Ray {
    pub fn new(origin: Vec3, direction: Vec3) -> Self {
        Self {
            origin,
            direction: direction.normalize(),
        }
    }

    pub fn intersect_aabb(&self, aabb_min: Vec3, aabb_max: Vec3) -> Option<f32> {
        let inv_dir = Vec3::new(
            if self.direction.x.abs() > 1e-6 { 1.0 / self.direction.x } else { f32::INFINITY },
            if self.direction.y.abs() > 1e-6 { 1.0 / self.direction.y } else { f32::INFINITY },
            if self.direction.z.abs() > 1e-6 { 1.0 / self.direction.z } else { f32::INFINITY },
        );

        let t0 = (aabb_min - self.origin) * inv_dir;
        let t1 = (aabb_max - self.origin) * inv_dir;

        let tmin = t0.min(t1);
        let tmax = t0.max(t1);

        let tenter = tmin.max_element();
        let texit = tmax.min_element();

        if tenter <= texit && texit >= 0.0 {
            if tenter >= 0.0 {
                Some(tenter)
            } else {
                Some(texit)
            }
        } else {
            None
        }
    }

    pub fn intersect_obb(&self, aabb_min: Vec3, aabb_max: Vec3, model_matrix: Mat4) -> Option<f32> {
        let inv_mat = model_matrix.inverse();
        let local_origin = inv_mat.transform_point3(self.origin);
        let local_dir = inv_mat.transform_vector3(self.direction);

        let length = local_dir.length();
        if length < 1e-6 {
            return None;
        }

        let local_ray = Ray {
            origin: local_origin,
            direction: local_dir / length,
        };

        let padding = Vec3::splat(0.05);
        let min_padded = aabb_min.min(aabb_max) - padding;
        let max_padded = aabb_min.max(aabb_max) + padding;

        local_ray.intersect_aabb(min_padded, max_padded).map(|local_t| local_t / length)
    }

    pub fn intersect_sphere(&self, center: Vec3, radius: f32) -> Option<f32> {
        let oc = self.origin - center;
        let b = 2.0 * oc.dot(self.direction);
        let c = oc.dot(oc) - radius * radius;
        let discriminant = b * b - 4.0 * c;

        if discriminant < 0.0 {
            None
        } else {
            let sqrt_disc = discriminant.sqrt();
            let t1 = (-b - sqrt_disc) * 0.5;
            let t2 = (-b + sqrt_disc) * 0.5;

            if t1 >= 0.0 {
                Some(t1)
            } else if t2 >= 0.0 {
                Some(t2)
            } else {
                None
            }
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct Camera {
    pub eye: Vec3,
    pub target: Vec3,
    pub up: Vec3,
    pub aspect: f32,
    pub fovy: f32,
    pub znear: f32,
    pub zfar: f32,
}

impl Camera {
    pub fn build_uniform(&self) -> CameraUniform {
        let view = camera::rh::view::look_at_mat4(self.eye, self.target, self.up);
        let proj = camera::rh::proj::directx::perspective(
            self.fovy.to_radians(),
            self.aspect,
            self.znear,
            self.zfar,
        );

        CameraUniform {
            view_proj: proj * view,
            view_matrix: view,
            inv_proj_matrix: proj.inverse(),
            view_position: Vec4::new(self.eye.x, self.eye.y, self.eye.z, 1.0),
        }
    }

    pub fn screen_to_ray(&self, screen_pos: Vec2, viewport_size: Vec2) -> Ray {
        let x_ndc = (2.0 * screen_pos.x / viewport_size.x) - 1.0;
        let y_ndc = 1.0 - (2.0 * screen_pos.y / viewport_size.y);

        let view = camera::rh::view::look_at_mat4(self.eye, self.target, self.up);
        let proj = camera::rh::proj::directx::perspective(
            self.fovy.to_radians(),
            self.aspect,
            self.znear,
            self.zfar,
        );

        let inv_view_proj = (proj * view).inverse();
        let far_pt = inv_view_proj.project_point3(Vec3::new(x_ndc, y_ndc, 1.0));

        Ray::new(self.eye, far_pt - self.eye)
    }
}