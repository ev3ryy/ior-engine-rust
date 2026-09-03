use std::collections::HashSet;
use std::sync::Arc;
use glam::Vec3;

use crate::frustum::Frustum;
use crate::light::{LightUniform, MAX_LIGHTS};
use crate::point_shadow::{self, MAX_POINT_SHADOWS};
use crate::spot_shadow::{MAX_SPOT_SHADOWS};
use crate::CameraUniform;

pub const GRID_X: u32 = 16;
pub const GRID_Y: u32 = 9;
pub const GRID_Z: u32 = 24;
pub const TOTAL_CLUSTERS: u32 = GRID_X * GRID_Y * GRID_Z;
pub const MAX_LIGHTS_PER_CLUSTER: u32 = 256;

#[repr(C)]
#[derive(Debug, Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
pub struct ClusterData {
    pub grid_dim: [u32; 3],
    pub z_near: f32,
    pub tile_size: [f32; 2],
    pub screen_size: [f32; 2],
    pub z_far: f32,
    pub total_lights: u32,
    pub dir_light_count: u32,
    pub padding: u32,
}

pub struct LightClusterSystem {
    pub camera_buffer: wgpu::Buffer,
    pub lights_buffer: wgpu::Buffer,
    pub cluster_data_buffer: wgpu::Buffer,
    pub cluster_grid_buffer: wgpu::Buffer,
    pub light_indices_buffer: wgpu::Buffer,

    pub compute_pipeline: wgpu::ComputePipeline,
    pub compute_bind_group: wgpu::BindGroup,
    pub camera_bind_group: wgpu::BindGroup,
    pub camera_bind_group_layout: Arc<wgpu::BindGroupLayout>,

    pub sorted_lights: Vec<LightUniform>,
    pub previous_shadow_points: HashSet<u32>,
    pub previous_shadow_spots: HashSet<u32>,

    scored_points_scratch: Vec<(usize, f32)>,
    scored_spots_scratch: Vec<(usize, f32)>,
}

impl LightClusterSystem {
    pub fn new(device: &wgpu::Device, shadow_uniform_buffer: &wgpu::Buffer) -> Self {
        let camera_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Camera Buffer"),
            size: std::mem::size_of::<CameraUniform>() as wgpu::BufferAddress,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let lights_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Lights Buffer"),
            size: (std::mem::size_of::<LightUniform>() * MAX_LIGHTS) as u64,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let cluster_data_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Cluster Data Buffer"),
            size: std::mem::size_of::<ClusterData>() as wgpu::BufferAddress,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let cluster_grid_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Cluster Grid Buffer"),
            size: (TOTAL_CLUSTERS * 8) as u64,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let light_indices_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Light Indices Buffer"),
            size: (TOTAL_CLUSTERS * MAX_LIGHTS_PER_CLUSTER * 4) as u64,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let compute_bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Compute Bind Group Layout"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer { ty: wgpu::BufferBindingType::Uniform, has_dynamic_offset: false, min_binding_size: None },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer { ty: wgpu::BufferBindingType::Uniform, has_dynamic_offset: false, min_binding_size: None },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 3,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer { ty: wgpu::BufferBindingType::Storage { read_only: false }, has_dynamic_offset: false, min_binding_size: None },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 4,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer { ty: wgpu::BufferBindingType::Storage { read_only: false }, has_dynamic_offset: false, min_binding_size: None },
                    count: None,
                },
            ],
        });

        let compute_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Compute Bind Group"),
            layout: &compute_bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry { binding: 0, resource: camera_buffer.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 1, resource: cluster_data_buffer.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 2, resource: lights_buffer.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 3, resource: cluster_grid_buffer.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 4, resource: light_indices_buffer.as_entire_binding() },
            ],
        });

        let cull_module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("cull_cs.wgsl"),
            source: wgpu::ShaderSource::Wgsl(include_str!(concat!(env!("OUT_DIR"), "/cull_cs.wgsl")).into()),
        });

        let compute_pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Compute Pipeline Layout"),
            bind_group_layouts: &[Some(&compute_bind_group_layout)],
            immediate_size: 0,
        });

        let compute_pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("Compute Culling Pipeline"),
            layout: Some(&compute_pipeline_layout),
            module: &cull_module,
            entry_point: Some("cull_lights"),
            compilation_options: Default::default(),
            cache: None,
        });

        let camera_bind_group_layout = Arc::new(device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("camera_bind_group_layout"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer { ty: wgpu::BufferBindingType::Uniform, has_dynamic_offset: false, min_binding_size: None },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer { ty: wgpu::BufferBindingType::Uniform, has_dynamic_offset: false, min_binding_size: None },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 3,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer { ty: wgpu::BufferBindingType::Storage { read_only: true }, has_dynamic_offset: false, min_binding_size: None },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 4,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer { ty: wgpu::BufferBindingType::Storage { read_only: true }, has_dynamic_offset: false, min_binding_size: None },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 5,
                    visibility: wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer { ty: wgpu::BufferBindingType::Uniform, has_dynamic_offset: false, min_binding_size: None },
                    count: None,
                },
            ],
        }));

        let camera_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("camera_bind_group"),
            layout: &camera_bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry { binding: 0, resource: camera_buffer.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 1, resource: lights_buffer.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 2, resource: cluster_data_buffer.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 3, resource: cluster_grid_buffer.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 4, resource: light_indices_buffer.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 5, resource: shadow_uniform_buffer.as_entire_binding() },
            ],
        });

        Self {
            camera_buffer,
            lights_buffer,
            cluster_data_buffer,
            cluster_grid_buffer,
            light_indices_buffer,
            compute_pipeline,
            compute_bind_group,
            camera_bind_group,
            camera_bind_group_layout,
            sorted_lights: Vec::with_capacity(MAX_LIGHTS),
            previous_shadow_points: HashSet::with_capacity(MAX_POINT_SHADOWS * 2),
            previous_shadow_spots: HashSet::with_capacity(MAX_SPOT_SHADOWS * 2),
            scored_points_scratch: Vec::with_capacity(256),
            scored_spots_scratch: Vec::with_capacity(256),
        }
    }

    pub fn upload_camera(&self, queue: &wgpu::Queue, camera: &CameraUniform) {
        queue.write_buffer(&self.camera_buffer, 0, bytemuck::cast_slice(&[*camera]));
    }

    pub fn upload_lights(&mut self, queue: &wgpu::Queue, lights: &[LightUniform], camera: &CameraUniform) {
        self.sorted_lights.clear();
        self.sorted_lights.extend_from_slice(lights);

        let cam_pos = camera.view_position.truncate();
        let cam_frustum = Frustum::from_view_proj(camera.view_proj);

        self.scored_points_scratch.clear();
        self.scored_spots_scratch.clear();

        for (i, l) in self.sorted_lights.iter().enumerate() {
            if l.light_type == 1 {
                let previously_active = self.previous_shadow_points.contains(&(i as u32));
                let score = point_shadow::calculate_shadow_score(
                    cam_pos,
                    &cam_frustum,
                    Vec3::from(l.position),
                    l.range,
                    l.intensity,
                    l.color,
                    previously_active,
                );
                self.scored_points_scratch.push((i, score));
            } else if l.light_type == 2 {
                let previously_active = self.previous_shadow_spots.contains(&(i as u32));
                let score = point_shadow::calculate_shadow_score(
                    cam_pos,
                    &cam_frustum,
                    Vec3::from(l.position),
                    l.range,
                    l.intensity,
                    l.color,
                    previously_active,
                );
                self.scored_spots_scratch.push((i, score));
            }
        }

        // 1. Point Shadows
        self.scored_points_scratch.sort_unstable_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        self.previous_shadow_points.clear();
        for (slot, &(light_idx, score)) in self.scored_points_scratch.iter().take(MAX_POINT_SHADOWS).enumerate() {
            if score > 0.001 {
                self.sorted_lights[light_idx].shadow_index = slot as i32;
                self.previous_shadow_points.insert(light_idx as u32);
            } else {
                self.sorted_lights[light_idx].shadow_index = -1;
            }
        }

        // 2. Spot Shadows
        self.scored_spots_scratch.sort_unstable_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        self.previous_shadow_spots.clear();
        for (slot, &(light_idx, score)) in self.scored_spots_scratch.iter().take(MAX_SPOT_SHADOWS).enumerate() {
            if score > 0.001 {
                self.sorted_lights[light_idx].shadow_index = slot as i32;
                self.previous_shadow_spots.insert(light_idx as u32);
            } else {
                self.sorted_lights[light_idx].shadow_index = -1;
            }
        }

        self.sorted_lights.sort_unstable_by_key(|l| l.light_type);

        let count = self.sorted_lights.len().min(MAX_LIGHTS);
        queue.write_buffer(
            &self.lights_buffer,
            0,
            bytemuck::cast_slice(&self.sorted_lights[..count]),
        );
    }

    pub fn update_cluster_data(&self, queue: &wgpu::Queue, width: u32, height: u32) {
        let total_lights = self.sorted_lights.len().min(MAX_LIGHTS) as u32;
        let dir_light_count = self.sorted_lights.iter().take(MAX_LIGHTS).filter(|l| l.light_type == 0).count() as u32;

        let cluster_data = ClusterData {
            grid_dim: [GRID_X, GRID_Y, GRID_Z],
            z_near: 0.1,
            z_far: 1000.0,
            tile_size: [width as f32 / GRID_X as f32, height as f32 / GRID_Y as f32],
            screen_size: [width as f32, height as f32],
            total_lights,
            dir_light_count,
            padding: 0,
        };
        queue.write_buffer(&self.cluster_data_buffer, 0, bytemuck::cast_slice(&[cluster_data]));
    }

    pub fn dispatch_culling(&self, encoder: &mut wgpu::CommandEncoder) {
        let mut compute_pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor::default());
        compute_pass.set_pipeline(&self.compute_pipeline);
        compute_pass.set_bind_group(0, &self.compute_bind_group, &[]);
        compute_pass.dispatch_workgroups((GRID_X + 7) / 8, (GRID_Y + 7) / 8, GRID_Z);
    }
}