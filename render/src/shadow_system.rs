use std::sync::Arc;
use glam::{Mat4, Vec3};

use crate::InstanceRaw;
use crate::frustum::Frustum;
use crate::light::LightUniform;
use crate::model::{self};
use crate::point_shadow::{MAX_POINT_SHADOWS, POINT_NEAR_PLANE, POINT_SHADOW_FACES, POINT_SHADOW_MAP_SIZE, PointShadowUniform, TOTAL_POINT_FACES, self};
use crate::shadow::{CASCADE_COUNT, SHADOW_MAP_SIZE, ShadowUniform, self};
use crate::spot_shadow::{MAX_SPOT_SHADOWS, SPOT_SHADOW_MAP_SIZE, SpotShadowUniform, self};

pub struct ShadowSystem {
    // Cascades (CSM)
    pub shadow_texture: wgpu::Texture,
    pub shadow_view: wgpu::TextureView,
    pub cascade_views: [wgpu::TextureView; CASCADE_COUNT],
    pub shadow_uniform_buffer: wgpu::Buffer,
    pub cascade_matrix_buffers: [wgpu::Buffer; CASCADE_COUNT],
    pub cascade_bind_groups: [wgpu::BindGroup; CASCADE_COUNT],
    pub cascade_bind_group_layout: Arc<wgpu::BindGroupLayout>,

    // Spot Shadows
    pub spot_shadow_texture: wgpu::Texture,
    pub spot_shadow_views: [wgpu::TextureView; MAX_SPOT_SHADOWS],
    pub spot_shadow_uniform_buffer: wgpu::Buffer,
    pub spot_matrix_buffers: [wgpu::Buffer; MAX_SPOT_SHADOWS],
    pub spot_bind_groups: [wgpu::BindGroup; MAX_SPOT_SHADOWS],

    // Point Shadows
    pub point_shadow_texture: wgpu::Texture,
    pub point_shadow_views: [wgpu::TextureView; TOTAL_POINT_FACES],
    pub point_shadow_uniform_buffer: wgpu::Buffer,
    pub point_matrix_buffers: [wgpu::Buffer; TOTAL_POINT_FACES],
    pub point_bind_groups: [wgpu::BindGroup; TOTAL_POINT_FACES],

    // Shared forward binding
    pub shadow_sampler: wgpu::Sampler,
    pub shadow_map_bind_group_layout: Arc<wgpu::BindGroupLayout>,
    pub shadow_map_bind_group: wgpu::BindGroup,

    // Pipelines
    pub shadow_opaque_pipeline: wgpu::RenderPipeline,
    pub shadow_alpha_pipeline: wgpu::RenderPipeline,

    pub cascade_frustums: [Frustum; CASCADE_COUNT],
    pub active_spot_frustums: Vec<(usize, Frustum)>,
    pub active_point_lights: Vec<(usize, Vec3, f32, [Frustum; 6])>,

    // Draw call caches and scratch memory
    pub shadow_draw_calls_cache: [Vec<(Arc<model::Model>, usize, std::ops::Range<u32>)>; CASCADE_COUNT],
    pub spot_shadow_draw_calls_cache: [Vec<(Arc<model::Model>, usize, std::ops::Range<u32>)>; MAX_SPOT_SHADOWS],
    pub point_shadow_draw_calls_cache: [Vec<(Arc<model::Model>, usize, std::ops::Range<u32>)>; TOTAL_POINT_FACES],

    pub casc_scratch: [Vec<InstanceRaw>; CASCADE_COUNT],
    pub spot_scratch: [Vec<InstanceRaw>; MAX_SPOT_SHADOWS],
    pub point_scratch: [Vec<InstanceRaw>; TOTAL_POINT_FACES],
}

impl ShadowSystem {
    pub fn new(
        device: &wgpu::Device,
        material_bind_group_layout: &wgpu::BindGroupLayout,
        depth_fs_module: &wgpu::ShaderModule,
    ) -> Self {
        // 1. CSM Textures & Buffers
        let shadow_texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("CSM Texture Array"),
            size: wgpu::Extent3d {
                width: SHADOW_MAP_SIZE,
                height: SHADOW_MAP_SIZE,
                depth_or_array_layers: CASCADE_COUNT as u32,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Depth32Float,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });

        let shadow_view = shadow_texture.create_view(&wgpu::TextureViewDescriptor {
            label: Some("CSM Array View"),
            dimension: Some(wgpu::TextureViewDimension::D2Array),
            ..Default::default()
        });

        let cascade_views: [wgpu::TextureView; CASCADE_COUNT] = std::array::from_fn(|i| {
            shadow_texture.create_view(&wgpu::TextureViewDescriptor {
                label: Some(&format!("CSM Layer View {}", i)),
                dimension: Some(wgpu::TextureViewDimension::D2),
                base_array_layer: i as u32,
                array_layer_count: Some(1),
                ..Default::default()
            })
        });

        let shadow_uniform_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Shadow Uniform Buffer"),
            size: std::mem::size_of::<ShadowUniform>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let cascade_bind_group_layout = Arc::new(device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("cascade_bind_group_layout"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: Some(std::num::NonZeroU64::new(64).unwrap()),
                },
                count: None,
            }],
        }));

        let cascade_matrix_buffers: [wgpu::Buffer; CASCADE_COUNT] = std::array::from_fn(|i| {
            device.create_buffer(&wgpu::BufferDescriptor {
                label: Some(&format!("Cascade Matrix Buffer {}", i)),
                size: 64,
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            })
        });

        let cascade_bind_groups: [wgpu::BindGroup; CASCADE_COUNT] = std::array::from_fn(|i| {
            device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some(&format!("Cascade Matrix BG {}", i)),
                layout: &cascade_bind_group_layout,
                entries: &[wgpu::BindGroupEntry {
                    binding: 0,
                    resource: cascade_matrix_buffers[i].as_entire_binding(),
                }],
            })
        });

        // 2. Spot Shadows
        let spot_shadow_texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Spot Shadow Texture Array"),
            size: wgpu::Extent3d {
                width: SPOT_SHADOW_MAP_SIZE,
                height: SPOT_SHADOW_MAP_SIZE,
                depth_or_array_layers: MAX_SPOT_SHADOWS as u32,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Depth32Float,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });

        let spot_shadow_array_view = spot_shadow_texture.create_view(&wgpu::TextureViewDescriptor {
            label: Some("Spot Shadow Array View"),
            dimension: Some(wgpu::TextureViewDimension::D2Array),
            ..Default::default()
        });

        let spot_shadow_views: [wgpu::TextureView; MAX_SPOT_SHADOWS] = std::array::from_fn(|i| {
            spot_shadow_texture.create_view(&wgpu::TextureViewDescriptor {
                label: Some(&format!("Spot Shadow Layer View {}", i)),
                dimension: Some(wgpu::TextureViewDimension::D2),
                base_array_layer: i as u32,
                array_layer_count: Some(1),
                ..Default::default()
            })
        });

        let spot_shadow_uniform_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Spot Shadow Uniform Buffer"),
            size: std::mem::size_of::<SpotShadowUniform>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let spot_matrix_buffers: [wgpu::Buffer; MAX_SPOT_SHADOWS] = std::array::from_fn(|i| {
            device.create_buffer(&wgpu::BufferDescriptor {
                label: Some(&format!("Spot Matrix Buffer {}", i)),
                size: 64,
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            })
        });

        let spot_bind_groups: [wgpu::BindGroup; MAX_SPOT_SHADOWS] = std::array::from_fn(|i| {
            device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some(&format!("Spot Matrix BG {}", i)),
                layout: &cascade_bind_group_layout,
                entries: &[wgpu::BindGroupEntry {
                    binding: 0,
                    resource: spot_matrix_buffers[i].as_entire_binding(),
                }],
            })
        });

        // 3. Point Shadows
        let point_shadow_texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Point Shadow Cube Array Texture"),
            size: wgpu::Extent3d {
                width: POINT_SHADOW_MAP_SIZE,
                height: POINT_SHADOW_MAP_SIZE,
                depth_or_array_layers: TOTAL_POINT_FACES as u32,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Depth32Float,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });

        let point_shadow_cube_view = point_shadow_texture.create_view(&wgpu::TextureViewDescriptor {
            label: Some("Point Shadow Cube Array View"),
            dimension: Some(wgpu::TextureViewDimension::CubeArray),
            ..Default::default()
        });

        let point_shadow_views: [wgpu::TextureView; TOTAL_POINT_FACES] = std::array::from_fn(|i| {
            point_shadow_texture.create_view(&wgpu::TextureViewDescriptor {
                label: Some(&format!("Point Shadow Layer View {}", i)),
                dimension: Some(wgpu::TextureViewDimension::D2),
                base_array_layer: i as u32,
                array_layer_count: Some(1),
                ..Default::default()
            })
        });

        let point_matrix_buffers: [wgpu::Buffer; TOTAL_POINT_FACES] = std::array::from_fn(|i| {
            device.create_buffer(&wgpu::BufferDescriptor {
                label: Some(&format!("Point Face Matrix Buffer {}", i)),
                size: 64,
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            })
        });

        let point_bind_groups: [wgpu::BindGroup; TOTAL_POINT_FACES] = std::array::from_fn(|i| {
            device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some(&format!("Point Face BG {}", i)),
                layout: &cascade_bind_group_layout,
                entries: &[wgpu::BindGroupEntry {
                    binding: 0,
                    resource: point_matrix_buffers[i].as_entire_binding(),
                }],
            })
        });

        let point_shadow_uniform_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Point Shadow Uniform Buffer"),
            size: std::mem::size_of::<PointShadowUniform>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        // 4. Shadow Sampler & Forward Bind Group
        let shadow_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("Shadow Comparison Sampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::MipmapFilterMode::Nearest,
            compare: Some(wgpu::CompareFunction::LessEqual),
            ..Default::default()
        });

        let shadow_map_bind_group_layout = Arc::new(device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("shadow_map_bind_group_layout"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture { sample_type: wgpu::TextureSampleType::Depth, view_dimension: wgpu::TextureViewDimension::D2Array, multisampled: false },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Comparison),
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture { sample_type: wgpu::TextureSampleType::Depth, view_dimension: wgpu::TextureViewDimension::D2Array, multisampled: false },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 3,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer { ty: wgpu::BufferBindingType::Uniform, has_dynamic_offset: false, min_binding_size: None },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 4,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture { sample_type: wgpu::TextureSampleType::Depth, view_dimension: wgpu::TextureViewDimension::CubeArray, multisampled: false },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 5,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer { ty: wgpu::BufferBindingType::Uniform, has_dynamic_offset: false, min_binding_size: None },
                    count: None,
                },
            ],
        }));

        let shadow_map_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("shadow_map_bind_group"),
            layout: &shadow_map_bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry { binding: 0, resource: wgpu::BindingResource::TextureView(&shadow_view) },
                wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::Sampler(&shadow_sampler) },
                wgpu::BindGroupEntry { binding: 2, resource: wgpu::BindingResource::TextureView(&spot_shadow_array_view) },
                wgpu::BindGroupEntry { binding: 3, resource: spot_shadow_uniform_buffer.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 4, resource: wgpu::BindingResource::TextureView(&point_shadow_cube_view) },
                wgpu::BindGroupEntry { binding: 5, resource: point_shadow_uniform_buffer.as_entire_binding() },
            ],
        });

        // 5. Pipelines
        let shadow_vs_module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("shadow_vs.wgsl"),
            source: wgpu::ShaderSource::Wgsl(include_str!(concat!(env!("OUT_DIR"), "/shadow_vs.wgsl")).into()),
        });

        let shadow_pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Shadow Pipeline Layout"),
            bind_group_layouts: &[
                Some(material_bind_group_layout),
                Some(&cascade_bind_group_layout),
            ],
            immediate_size: 0,
        });

        let shadow_opaque_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Shadow Opaque Pipeline (Fast Depth-Only)"),
            layout: Some(&shadow_pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shadow_vs_module,
                entry_point: Some("vs_main"),
                buffers: &[Some(model::ModelVertex::desc()), Some(InstanceRaw::desc())],
                compilation_options: Default::default(),
            },
            fragment: None,
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: None,
                polygon_mode: wgpu::PolygonMode::Fill,
                ..Default::default()
            },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: wgpu::TextureFormat::Depth32Float,
                depth_write_enabled: Some(true),
                depth_compare: Some(wgpu::CompareFunction::LessEqual),
                stencil: wgpu::StencilState::default(),
                bias: wgpu::DepthBiasState {
                    constant: 0,
                    slope_scale: 1.0,
                    clamp: 0.0,
                },
            }),
            multisample: wgpu::MultisampleState::default(),
            multiview_mask: None,
            cache: None,
        });

        let shadow_alpha_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Shadow Alpha-Test Pipeline"),
            layout: Some(&shadow_pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shadow_vs_module,
                entry_point: Some("vs_main"),
                buffers: &[Some(model::ModelVertex::desc()), Some(InstanceRaw::desc())],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: depth_fs_module,
                entry_point: Some("depth_fs_main"),
                targets: &[],
                compilation_options: Default::default(),
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: None,
                polygon_mode: wgpu::PolygonMode::Fill,
                ..Default::default()
            },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: wgpu::TextureFormat::Depth32Float,
                depth_write_enabled: Some(true),
                depth_compare: Some(wgpu::CompareFunction::LessEqual),
                stencil: wgpu::StencilState::default(),
                bias: wgpu::DepthBiasState {
                    constant: 0,
                    slope_scale: 1.0,
                    clamp: 0.0,
                },
            }),
            multisample: wgpu::MultisampleState::default(),
            multiview_mask: None,
            cache: None,
        });

        Self {
            shadow_texture,
            shadow_view,
            cascade_views,
            shadow_uniform_buffer,
            cascade_matrix_buffers,
            cascade_bind_groups,
            cascade_bind_group_layout,

            spot_shadow_texture,
            spot_shadow_views,
            spot_shadow_uniform_buffer,
            spot_matrix_buffers,
            spot_bind_groups,

            point_shadow_texture,
            point_shadow_views,
            point_shadow_uniform_buffer,
            point_matrix_buffers,
            point_bind_groups,

            shadow_sampler,
            shadow_map_bind_group_layout,
            shadow_map_bind_group,

            shadow_opaque_pipeline,
            shadow_alpha_pipeline,

            cascade_frustums: std::array::from_fn(|_| Frustum::from_view_proj(Mat4::IDENTITY)),
            active_spot_frustums: Vec::with_capacity(MAX_SPOT_SHADOWS),
            active_point_lights: Vec::with_capacity(MAX_POINT_SHADOWS),

            shadow_draw_calls_cache: std::array::from_fn(|_| Vec::with_capacity(250)),
            spot_shadow_draw_calls_cache: std::array::from_fn(|_| Vec::with_capacity(250)),
            point_shadow_draw_calls_cache: std::array::from_fn(|_| Vec::with_capacity(250)),

            casc_scratch: std::array::from_fn(|_| Vec::with_capacity(1000)),
            spot_scratch: std::array::from_fn(|_| Vec::with_capacity(1000)),
            point_scratch: std::array::from_fn(|_| Vec::with_capacity(1000)),
        }
    }

    pub fn prepare_shadow_uniforms(
        &mut self,
        queue: &wgpu::Queue,
        sorted_lights: &[LightUniform],
        camera: &crate::CameraUniform,
        width: u32,
        height: u32,
    ) {
        // 1. Spot Shadows
        let mut spot_matrices = [glam::Mat4::IDENTITY; MAX_SPOT_SHADOWS];
        self.active_spot_frustums.clear();

        for light in sorted_lights {
            if light.light_type == 2 && light.shadow_index >= 0 {
                let idx = light.shadow_index as usize;
                let matrix = spot_shadow::calculate_spot_matrix(
                    glam::Vec3::from(light.position),
                    glam::Vec3::from(light.direction),
                    light.outer_cone_cos,
                    light.range,
                );
                spot_matrices[idx] = matrix;
                self.active_spot_frustums.push((idx, Frustum::from_view_proj(matrix)));
            }
        }

        queue.write_buffer(
            &self.spot_shadow_uniform_buffer,
            0,
            bytemuck::cast_slice(&[SpotShadowUniform { shadow_matrices: spot_matrices }]),
        );

        for &(idx, _) in &self.active_spot_frustums {
            queue.write_buffer(
                &self.spot_matrix_buffers[idx],
                0,
                bytemuck::cast_slice(&[spot_matrices[idx]]),
            );
        }

        // 2. Point Shadows
        let mut point_uniform = PointShadowUniform {
            light_params: [[POINT_NEAR_PLANE, 10.0, 0.0, 0.0]; MAX_POINT_SHADOWS],
        };
        self.active_point_lights.clear();

        for light in sorted_lights {
            if light.light_type == 1 && light.shadow_index >= 0 {
                let p_idx = light.shadow_index as usize;
                let l_pos = Vec3::from(light.position);
                let l_range = light.range.max(0.1);
                point_uniform.light_params[p_idx] = [POINT_NEAR_PLANE, l_range, light.shadow_bias, 0.0];

                let face_frustums: [Frustum; 6] = std::array::from_fn(|f| {
                    let matrix = point_shadow::calculate_cube_face_matrix(l_pos, f, POINT_NEAR_PLANE, l_range);
                    let global_face_idx = p_idx * POINT_SHADOW_FACES + f;
                    queue.write_buffer(
                        &self.point_matrix_buffers[global_face_idx],
                        0,
                        bytemuck::cast_slice(&[matrix]),
                    );
                    Frustum::from_view_proj(matrix)
                });

                self.active_point_lights.push((p_idx, l_pos, l_range, face_frustums));
            }
        }

        queue.write_buffer(
            &self.point_shadow_uniform_buffer,
            0,
            bytemuck::cast_slice(&[point_uniform]),
        );

        // 3. CSM Shadows
        let p_near = camera.inv_proj_matrix.project_point3(glam::Vec3::new(0.0, 0.0, 0.0));
        let p_far = camera.inv_proj_matrix.project_point3(glam::Vec3::new(0.0, 0.0, 1.0));
        let z_near = (-p_near.z).max(0.01);
        let z_far = (-p_far.z).max(z_near + 1.0);

        let shadow_splits = shadow::calculate_cascade_splits(
            z_near,
            z_far,
            shadow::MAX_SHADOW_DISTANCE,
            shadow::SPLIT_LAMBDA,
        );

        let light_dir = sorted_lights.iter().find(|l| l.light_type == 0).map_or(
            glam::Vec3::new(0.0, -1.0, 0.3),
            |l| glam::Vec3::from(l.direction),
        );
        let fov_y = (1.0 / camera.inv_proj_matrix.y_axis.y).atan() * 2.0;

        let shadow_uniform = shadow::calculate_cascade_matrices(
            camera.view_matrix,
            fov_y,
            width as f32 / height as f32,
            light_dir,
            shadow_splits,
        );

        queue.write_buffer(&self.shadow_uniform_buffer, 0, bytemuck::cast_slice(&[shadow_uniform]));

        for i in 0..CASCADE_COUNT {
            queue.write_buffer(
                &self.cascade_matrix_buffers[i],
                0,
                bytemuck::cast_slice(&[shadow_uniform.light_space_matrices[i]]),
            );
            self.cascade_frustums[i] = Frustum::from_view_proj(shadow_uniform.light_space_matrices[i]);
        }
    }

    pub fn clear_caches(&mut self) {
        for calls in &mut self.shadow_draw_calls_cache { calls.clear(); }
        for calls in &mut self.spot_shadow_draw_calls_cache { calls.clear(); }
        for calls in &mut self.point_shadow_draw_calls_cache { calls.clear(); }
    }

    pub fn record_render_passes(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        instance_buffer: &wgpu::Buffer,
    ) {
        // 1. Cascades
        for i in 0..CASCADE_COUNT {
            let mut shadow_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some(&format!("Shadow Pass Cascade {}", i)),
                color_attachments: &[],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &self.cascade_views[i],
                    depth_ops: Some(wgpu::Operations { load: wgpu::LoadOp::Clear(1.0), store: wgpu::StoreOp::Store }),
                    stencil_ops: None,
                }),
                ..Default::default()
            });

            shadow_pass.set_bind_group(1, &self.cascade_bind_groups[i], &[]);
            shadow_pass.set_vertex_buffer(1, instance_buffer.slice(..));

            let mut current_is_alpha = false;
            shadow_pass.set_pipeline(&self.shadow_opaque_pipeline);

            for (model, mesh_idx, range) in &self.shadow_draw_calls_cache[i] {
                let mesh = &model.meshes[*mesh_idx];
                let material = &model.materials[mesh.material];
                let is_alpha_tested = material.has_alpha_test;

                if is_alpha_tested != current_is_alpha {
                    if is_alpha_tested {
                        shadow_pass.set_pipeline(&self.shadow_alpha_pipeline);
                    } else {
                        shadow_pass.set_pipeline(&self.shadow_opaque_pipeline);
                    }
                    current_is_alpha = is_alpha_tested;
                }

                shadow_pass.set_vertex_buffer(0, mesh.vertex_buffer.slice(..));
                shadow_pass.set_index_buffer(mesh.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
                shadow_pass.set_bind_group(0, &material.bind_group, &[]);
                shadow_pass.draw_indexed(0..mesh.num_elements, 0, range.clone());
            }
        }

        // 2. Spot Shadows
        for &(spot_idx, _) in &self.active_spot_frustums {
            let mut spot_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some(&format!("Spot Shadow Pass {}", spot_idx)),
                color_attachments: &[],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &self.spot_shadow_views[spot_idx],
                    depth_ops: Some(wgpu::Operations { load: wgpu::LoadOp::Clear(1.0), store: wgpu::StoreOp::Store }),
                    stencil_ops: None,
                }),
                ..Default::default()
            });

            spot_pass.set_bind_group(1, &self.spot_bind_groups[spot_idx], &[]);
            spot_pass.set_vertex_buffer(1, instance_buffer.slice(..));

            let mut current_is_alpha = false;
            spot_pass.set_pipeline(&self.shadow_opaque_pipeline);

            for (model, mesh_idx, range) in &self.spot_shadow_draw_calls_cache[spot_idx] {
                let mesh = &model.meshes[*mesh_idx];
                let material = &model.materials[mesh.material];
                let is_alpha_tested = material.has_alpha_test;

                if is_alpha_tested != current_is_alpha {
                    if is_alpha_tested {
                        spot_pass.set_pipeline(&self.shadow_alpha_pipeline);
                    } else {
                        spot_pass.set_pipeline(&self.shadow_opaque_pipeline);
                    }
                    current_is_alpha = is_alpha_tested;
                }

                shadow_pass_draw(&mut spot_pass, mesh, material, range);
            }
        }

        // 3. Point Shadows
        for (p_idx, _, _, _) in &self.active_point_lights {
            for f in 0..6 {
                let global_idx = p_idx * POINT_SHADOW_FACES + f;
                let calls = &self.point_shadow_draw_calls_cache[global_idx];

                let mut point_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: Some(&format!("Point Shadow Pass L{} F{}", p_idx, f)),
                    color_attachments: &[],
                    depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                        view: &self.point_shadow_views[global_idx],
                        depth_ops: Some(wgpu::Operations { load: wgpu::LoadOp::Clear(1.0), store: wgpu::StoreOp::Store }),
                        stencil_ops: None,
                    }),
                    ..Default::default()
                });

                if !calls.is_empty() {
                    point_pass.set_bind_group(1, &self.point_bind_groups[global_idx], &[]);
                    point_pass.set_vertex_buffer(1, instance_buffer.slice(..));

                    let mut current_is_alpha = false;
                    point_pass.set_pipeline(&self.shadow_opaque_pipeline);

                    for (model, mesh_idx, range) in calls {
                        let mesh = &model.meshes[*mesh_idx];
                        let material = &model.materials[mesh.material];
                        let is_alpha_tested = material.has_alpha_test;

                        if is_alpha_tested != current_is_alpha {
                            if is_alpha_tested {
                                point_pass.set_pipeline(&self.shadow_alpha_pipeline);
                            } else {
                                point_pass.set_pipeline(&self.shadow_opaque_pipeline);
                            }
                            current_is_alpha = is_alpha_tested;
                        }

                        shadow_pass_draw(&mut point_pass, mesh, material, range);
                    }
                }
            }
        }
    }
}

#[inline(always)]
fn shadow_pass_draw<'a>(
    pass: &mut wgpu::RenderPass<'a>,
    mesh: &'a model::Mesh,
    material: &'a model::Material,
    range: &std::ops::Range<u32>,
) {
    pass.set_vertex_buffer(0, mesh.vertex_buffer.slice(..));
    pass.set_index_buffer(mesh.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
    pass.set_bind_group(0, &material.bind_group, &[]);
    pass.draw_indexed(0..mesh.num_elements, 0, range.clone());
}