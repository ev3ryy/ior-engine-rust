use glam::{Vec3, Vec4, camera};
use render::CameraUniform;

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
            self.zfar
        );
        
        CameraUniform {
            view_proj: proj * view,
            view_matrix: view,
            inv_proj_matrix: proj.inverse(),
            view_position: Vec4::new(self.eye.x, self.eye.y, self.eye.z, 1.0),
        }
    }
}