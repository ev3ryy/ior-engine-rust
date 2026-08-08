use glam::Vec3;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;

#[derive(Debug, Serialize, Deserialize)]
pub struct EditorConfig {
    pub window_title: String,
    pub window_width: u32,
    pub window_height: u32,

    // start camera settings
    pub camera_eye: Vec3,
    pub camera_target: Vec3,
    pub camera_fovy: f32,
    pub camera_znear: f32,
    pub camera_zfar: f32,
    pub camera_speed: f32,

    // render
    pub enable_vsync: bool,
    pub msaa_samples: u32,      // 1, 2, 4, 8

    // ECS / scene
    pub initial_grid_rows: u32,
    pub initial_grid_spacing: f32,

    // Save / load
    pub world_save_path: String,
}

impl Default for EditorConfig {
    fn default() -> Self {
        Self {
            window_title: "IOR Engine".to_string(),
            window_width: 800,
            window_height: 600,
            camera_eye: Vec3::new(0.0, 5.0, -10.0),
            camera_target: Vec3::ZERO,
            camera_fovy: 45.0,
            camera_znear: 0.1,
            camera_zfar: 100.0,
            camera_speed: 0.2,
            enable_vsync: false,
            msaa_samples: 1,
            initial_grid_rows: 10,
            initial_grid_spacing: 3.0,
            world_save_path: "world.bin".to_string(),
        }
    }
}

impl EditorConfig {
    pub fn load(path: &str) -> Self {
        if Path::new(path).exists() {
            if let Ok(content) = fs::read_to_string(path) {
                if let Ok(config) = toml::from_str(&content) {
                    return config;
                }
            }
            log::warn!("Failed to parse config at {}, falling back to defaults", path);
        }

        let default_config = Self::default();
        default_config.save(path);
        default_config
    }

    pub fn save(&self, path: &str) {
        if let Ok(content) = toml::to_string_pretty(self) {
            if let Err(e) = fs::write(path, content) {
                log::error!("Failed to save config to {}: {}", path, e);
            }
        }
    }
}