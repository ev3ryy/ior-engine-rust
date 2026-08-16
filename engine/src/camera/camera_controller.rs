use crate::camera::Camera;
use glam::Vec3;

#[derive(Debug, Clone, Copy)]
pub enum CameraMovement {
    Forward(bool),
    Backward(bool),
    Left(bool),
    Right(bool),
    Up(bool),
    Down(bool), 
    Rotate { dx: f32, dy: f32 }, 
    Zoom(f32),
}

pub struct CameraController {
    pub speed: f32,
    pub sensitivity: f32,
    yaw: f32,
    pitch: f32,
    is_up_pressed: bool,
    is_down_pressed: bool,
    is_forward_pressed: bool,
    is_backward_pressed: bool,
    is_left_pressed: bool,
    is_right_pressed: bool,
    first_update: bool,
    zoom_delta: f32,
}

impl CameraController {
    pub fn new(speed: f32) -> Self {
        Self {
            speed,
            sensitivity: 0.15,
            yaw: -90.0,
            pitch: 0.0,
            is_up_pressed: false,
            is_down_pressed: false,
            is_forward_pressed: false,
            is_backward_pressed: false,
            is_left_pressed: false,
            is_right_pressed: false,
            first_update: true,
            zoom_delta: 0.0,
        }
    }

    pub fn apply_movement(&mut self, movement: CameraMovement) {
        match movement {
            CameraMovement::Forward(pressed) => self.is_forward_pressed = pressed,
            CameraMovement::Backward(pressed) => self.is_backward_pressed = pressed,
            CameraMovement::Left(pressed) => self.is_left_pressed = pressed,
            CameraMovement::Right(pressed) => self.is_right_pressed = pressed,
            CameraMovement::Up(pressed) => self.is_up_pressed = pressed,
            CameraMovement::Down(pressed) => self.is_down_pressed = pressed,
            CameraMovement::Rotate { dx, dy } => {
                self.yaw += dx * self.sensitivity;
                self.pitch -= dy * self.sensitivity;

                if self.pitch > 89.0 { self.pitch = 89.0; }
                if self.pitch < -89.0 { self.pitch = -89.0; }
            }

            CameraMovement::Zoom(delta) => {
                self.zoom_delta += delta;
            }
        }
    }

    pub fn update_camera(&mut self, camera: &mut Camera, dt: f32) {
        if self.first_update {
            let forward = (camera.target - camera.eye).normalize();
            self.pitch = forward.y.asin().to_degrees();
            self.yaw = forward.z.atan2(forward.x).to_degrees();
            self.first_update = false;
        }

        let yaw_r = self.yaw.to_radians();
        let pitch_r = self.pitch.to_radians();

        let forward = Vec3::new(
            yaw_r.cos() * pitch_r.cos(),
            pitch_r.sin(),
            yaw_r.sin() * pitch_r.cos(),
        ).normalize();

        let right = forward.cross(Vec3::Y).normalize();
        let up = right.cross(forward).normalize();

        let velocity = self.speed * dt;
        let mut movement = Vec3::ZERO;

        if self.is_forward_pressed { movement += forward; }
        if self.is_backward_pressed { movement -= forward; }
        if self.is_right_pressed { movement += right; }
        if self.is_left_pressed { movement -= right; }
        if self.is_up_pressed { movement += Vec3::Y; }
        if self.is_down_pressed { movement -= Vec3::Y; }

        if self.zoom_delta != 0.0 {
            camera.eye += forward * self.zoom_delta * self.speed * 5.0 * dt;
            self.zoom_delta = 0.0;
        }

        if movement.length_squared() > 0.0 {
            camera.eye += movement.normalize() * velocity;
        }

        camera.target = camera.eye + forward;
        camera.up = up;
    }
}