use crate::camera::Camera;

#[derive(Debug, Clone, Copy)]
pub enum CameraMovement {
    Forward(bool),
    Backward(bool),
    Left(bool),
    Right(bool),
    Up(bool),
    Down(bool),
}

pub struct CameraController {
    speed: f32,
    is_up_pressed: bool,
    is_down_pressed: bool,
    is_forward_pressed: bool,
    is_backward_pressed: bool,
    is_left_pressed: bool,
    is_right_pressed: bool,
}

impl CameraController {
    pub fn new(speed: f32) -> Self {
        Self {
            speed,
            is_up_pressed: false,
            is_down_pressed: false,
            is_forward_pressed: false,
            is_backward_pressed: false,
            is_left_pressed: false,
            is_right_pressed: false,
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
        }
    }

    pub fn update_camera(&self, camera: &mut Camera) {
        let forward = camera.target - camera.eye;
        let forward_norm = forward.normalize();
        let forward_mag = forward.length();

        if self.is_forward_pressed && forward_mag > self.speed {
            camera.eye += forward_norm * self.speed;
        }
        if self.is_backward_pressed {
            camera.eye -= forward_norm * self.speed;
        }

        let right = forward_norm.cross(camera.up);
        let forward = camera.target - camera.eye;
        let forward_mag = forward.length();

        if self.is_right_pressed {
            camera.eye = camera.target - (forward + right * self.speed).normalize() * forward_mag;
        }
        if self.is_left_pressed {
            camera.eye = camera.target - (forward - right * self.speed).normalize() * forward_mag;
        }
    }
}