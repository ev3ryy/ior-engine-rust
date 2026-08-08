use glam::{Mat4, Vec3, camera};

#[derive(Debug)]
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
    pub fn view_projection_matrix(&self) -> Mat4 {
        let view = camera::rh::view::look_at_mat4(self.eye, self.target, self.up);
        let proj = camera::rh::proj::directx::perspective(self.fovy.to_radians(), self.aspect, self.znear, self.zfar);
        proj * view
    }
}