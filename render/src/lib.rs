pub mod model;
pub mod resources;
pub mod texture;
pub mod mipmap;
pub mod light;
pub mod shadow;

use light::{LightUniform};
use shadow::{CASCADE_COUNT, SHADOW_MAP_SIZE, ShadowUniform};

use glam::{Mat4, Vec4};
use std::sync::Arc;

#[repr(C)]
#[derive(Debug, Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
pub struct CameraUniform {
    pub view_proj: Mat4,
    pub view_matrix: Mat4,
    pub inv_proj_matrix: Mat4,
    pub view_position: Vec4,
}

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

#[repr(C)]
#[derive(Debug, Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
pub struct InstanceRaw {
    #[allow(dead_code)]
    pub model: Mat4,
    pub normal_matrix: Mat4,
}

impl InstanceRaw {
    pub fn new(model_matrix: Mat4) -> Self {
        let normal_matrix = model_matrix.inverse().transpose();
        Self {
            model: model_matrix,
            normal_matrix,
        }
    }

    fn desc() -> wgpu::VertexBufferLayout<'static> {
        use std::mem;
        wgpu::VertexBufferLayout {
            array_stride: mem::size_of::<InstanceRaw>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Instance,
            attributes: &[
                // --- Model Matrix (locations 5-8) ---
                wgpu::VertexAttribute { offset: 0, shader_location: 5, format: wgpu::VertexFormat::Float32x4 },
                wgpu::VertexAttribute { offset: mem::size_of::<Vec4>() as wgpu::BufferAddress, shader_location: 6, format: wgpu::VertexFormat::Float32x4 },
                wgpu::VertexAttribute { offset: (mem::size_of::<Vec4>() * 2) as wgpu::BufferAddress, shader_location: 7, format: wgpu::VertexFormat::Float32x4 },
                wgpu::VertexAttribute { offset: (mem::size_of::<Vec4>() * 3) as wgpu::BufferAddress, shader_location: 8, format: wgpu::VertexFormat::Float32x4 },
                
                // --- Normal Matrix (locations 9-12) ---
                wgpu::VertexAttribute { offset: (mem::size_of::<Vec4>() * 4) as wgpu::BufferAddress, shader_location: 9, format: wgpu::VertexFormat::Float32x4 },
                wgpu::VertexAttribute { offset: (mem::size_of::<Vec4>() * 5) as wgpu::BufferAddress, shader_location: 10, format: wgpu::VertexFormat::Float32x4 },
                wgpu::VertexAttribute { offset: (mem::size_of::<Vec4>() * 6) as wgpu::BufferAddress, shader_location: 11, format: wgpu::VertexFormat::Float32x4 },
                wgpu::VertexAttribute { offset: (mem::size_of::<Vec4>() * 7) as wgpu::BufferAddress, shader_location: 12, format: wgpu::VertexFormat::Float32x4 },
            ],
        }
    }
}

const GRID_X: u32 = 16;
const GRID_Y: u32 = 9;
const GRID_Z: u32 = 24;
const TOTAL_CLUSTERS: u32 = GRID_X * GRID_Y * GRID_Z;
const MAX_LIGHTS_PER_CLUSTER: u32 = 256;

pub struct Frustum {
    planes: [glam::Vec4; 6],
}

impl Frustum {
    pub fn from_view_proj(vp: glam::Mat4) -> Self {
        let r1 = vp.row(0);
        let r2 = vp.row(1);
        let r3 = vp.row(2);
        let r4 = vp.row(3);

        let mut planes = [
            r4 + r1, // Left
            r4 - r1, // Right
            r4 + r2, // Bottom
            r4 - r2, // Top
            r3,      // Near
            r4 - r3, // Far
        ];

        for p in &mut planes {
            let len = p.truncate().length();
            *p /= len;
        }

        Self { planes }
    }

    #[inline]
    pub fn intersects_sphere(&self, center: glam::Vec3, radius: f32) -> bool {
        for p in &self.planes {
            if p.truncate().dot(center) + p.w < -radius {
                return false;
            }
        }
        true
    }
}

pub struct Renderer {
    pub egui_renderer: egui_wgpu::Renderer,
    pub depth_texture: texture::Texture,
    
    #[allow(dead_code)]
    instance_buffer: wgpu::Buffer,
    instance_buffer_capacity: usize,
    camera_bind_group: wgpu::BindGroup,
    compute_bind_group: wgpu::BindGroup,
    compute_pipeline: wgpu::ComputePipeline,
    camera_buffer: wgpu::Buffer,
    sorted_lights: Vec<LightUniform>,
    lights_buffer: wgpu::Buffer,
    cluster_data_buffer: wgpu::Buffer,
    #[allow(dead_code)]
    cluster_grid_buffer: wgpu::Buffer,
    #[allow(dead_code)]
    light_indices_buffer: wgpu::Buffer,
    #[allow(dead_code)]
    cascade_bind_group_layout: Arc<wgpu::BindGroupLayout>,
    cascade_bind_groups: Vec<wgpu::BindGroup>,
    shadow_map_bind_group: wgpu::BindGroup,
    cascade_views: Vec<wgpu::TextureView>,
    shadow_uniform_buffer: wgpu::Buffer,
    depth_pipeline: wgpu::RenderPipeline,
    shadow_pipeline: wgpu::RenderPipeline,
    render_pipeline: wgpu::RenderPipeline,
    
    is_surface_configured: bool,
    pub config: wgpu::SurfaceConfiguration,
    instances_cache: Vec<InstanceRaw>,
    draw_calls_cache: Vec<(Arc<model::Model>, usize, std::ops::Range<u32>)>,
    pub material_bind_group_layout: Arc<wgpu::BindGroupLayout>,
    
    pub queue: Arc<wgpu::Queue>,
    pub device: Arc<wgpu::Device>,

    surface: wgpu::Surface<'static>,
    
    #[allow(dead_code)]
    instance: wgpu::Instance,
}

impl Renderer {
    pub async fn new(
        window: impl Into<wgpu::SurfaceTarget<'static>>,
        width: u32,
        height: u32,
    ) -> anyhow::Result<Self> {
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
            backends: wgpu::Backends::PRIMARY,
            flags: Default::default(),
            memory_budget_thresholds: Default::default(),
            backend_options: Default::default(),
            display: None,
        });

        let surface = instance.create_surface(window).unwrap();

        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::default(),
                compatible_surface: Some(&surface),
                force_fallback_adapter: false,
                apply_limit_buckets: true,
            })
            .await
            .unwrap();

        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor {
                label: None,
                required_features: wgpu::Features::INDIRECT_FIRST_INSTANCE | wgpu::Features::IMMEDIATES,
                experimental_features: wgpu::ExperimentalFeatures::disabled(),
                required_limits: wgpu::Limits::default(),
                memory_hints: Default::default(),
                trace: wgpu::Trace::Off,
            })
            .await
            .unwrap();

        let device = Arc::new(device);
        let queue = Arc::new(queue);

        let surface_caps = surface.get_capabilities(&adapter);
        let surface_format = surface_caps
            .formats
            .iter()
            .copied()
            .find(|f| f.is_srgb())
            .unwrap_or(surface_caps.formats[0]);

        let width = width.max(1);
        let height = height.max(1);

        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format: surface_format,
            width,
            height,
            present_mode: wgpu::PresentMode::Immediate,
            alpha_mode: surface_caps.alpha_modes[0],
            view_formats: vec![],
            desired_maximum_frame_latency: 2,
            color_space: wgpu::SurfaceColorSpace::Auto,
        };

        surface.configure(&device, &config);

        let egui_renderer = egui_wgpu::Renderer::new(&device, config.format, egui_wgpu::RendererOptions::default());

        let material_bind_group_layout =
            Arc::new(device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("material_bind_group_layout"),
                entries: &[
                    // 0: Material Uniforms (const)
                    wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Uniform,
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    },
                    // 1: Albedo Texture (sRGB)
                    wgpu::BindGroupLayoutEntry {
                        binding: 1,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Texture { multisampled: false, view_dimension: wgpu::TextureViewDimension::D2, sample_type: wgpu::TextureSampleType::Float { filterable: true } },
                        count: None,
                    },
                    // 2: Normal Texture (Linear)
                    wgpu::BindGroupLayoutEntry {
                        binding: 2,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Texture { multisampled: false, view_dimension: wgpu::TextureViewDimension::D2, sample_type: wgpu::TextureSampleType::Float { filterable: true } },
                        count: None,
                    },
                    // 3: Metallic Roughness Texture (Linear)
                    wgpu::BindGroupLayoutEntry {
                        binding: 3,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Texture { multisampled: false, view_dimension: wgpu::TextureViewDimension::D2, sample_type: wgpu::TextureSampleType::Float { filterable: true } },
                        count: None,
                    },
                    // 4: Sampler (единый для всех, если не нужны разные параметры)
                    wgpu::BindGroupLayoutEntry {
                        binding: 4,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                        count: None,
                    },
                ],
            }));

        let camera_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Camera Buffer"),
            size: std::mem::size_of::<CameraUniform>() as wgpu::BufferAddress, 
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let initial_capacity = 100;
        let instance_buffer = Self::create_instance_buffer(&device, initial_capacity);
        
        let lights_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Lights Buffer"),
            size: (std::mem::size_of::<light::LightUniform>() * light::MAX_LIGHTS) as u64,
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
                // 0: Camera Uniform
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer { ty: wgpu::BufferBindingType::Uniform, has_dynamic_offset: false, min_binding_size: None },
                    count: None,
                },
                // 1: Cluster Data Uniform
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer { ty: wgpu::BufferBindingType::Uniform, has_dynamic_offset: false, min_binding_size: None },
                    count: None,
                },
                // 2: Lights Buffer Uniform
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer { 
                        ty: wgpu::BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: false, 
                        min_binding_size: None 
                    },
                    count: None,
                },
                // 3: Cluster Grid (Storage, Read-Write)
                wgpu::BindGroupLayoutEntry {
                    binding: 3,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer { ty: wgpu::BufferBindingType::Storage { read_only: false }, has_dynamic_offset: false, min_binding_size: None },
                    count: None,
                },
                // 4: Light Indices (Storage, Read-Write)
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

        let shadow_texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("CSM Texture Array"),
            size: wgpu::Extent3d {
                width: SHADOW_MAP_SIZE,
                height: SHADOW_MAP_SIZE,
                depth_or_array_layers: CASCADE_COUNT as u32, // 4 layers
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            // Depth32Float or Depth16Float...
            format: wgpu::TextureFormat::Depth32Float, 
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });

        let shadow_view = shadow_texture.create_view(&wgpu::TextureViewDescriptor {
            label: Some("CSM Array View"),
            dimension: Some(wgpu::TextureViewDimension::D2Array),
            ..Default::default()
        });

        let mut cascade_views = Vec::new();
        for i in 0..CASCADE_COUNT as u32 {
            let view = shadow_texture.create_view(&wgpu::TextureViewDescriptor {
                label: Some(&format!("CSM Layer View {}", i)),
                dimension: Some(wgpu::TextureViewDimension::D2),
                base_array_layer: i,
                array_layer_count: Some(1),
                ..Default::default()
            });
            cascade_views.push(view);
        }

        let shadow_sampler = device.create_sampler(
            &wgpu::SamplerDescriptor {
                label: Some("Shadow Comparison Sampler"),

                address_mode_u: wgpu::AddressMode::ClampToEdge,
                address_mode_v: wgpu::AddressMode::ClampToEdge,
                address_mode_w: wgpu::AddressMode::ClampToEdge,

                mag_filter: wgpu::FilterMode::Linear,
                min_filter: wgpu::FilterMode::Linear,
                mipmap_filter: wgpu::MipmapFilterMode::Nearest,

                compare: Some(wgpu::CompareFunction::LessEqual),

                ..Default::default()
            }
        );

        let shadow_uniform_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Shadow Uniform Buffer"),
            size: std::mem::size_of::<ShadowUniform>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let camera_bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("camera_bind_group_layout"),
            entries: &[
                // 0: Camera Uniform (Vertex + Fragment)
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer { ty: wgpu::BufferBindingType::Uniform, has_dynamic_offset: false, min_binding_size: None },
                    count: None,
                },
                // 1: Lights Buffer Uniform (Fragment)
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer { 
                        ty: wgpu::BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: false, 
                        min_binding_size: None 
                    },
                    count: None,
                },
                // 2: Cluster Data Uniform (Fragment)
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer { ty: wgpu::BufferBindingType::Uniform, has_dynamic_offset: false, min_binding_size: None },
                    count: None,
                },
                // 3: Cluster Grid (Storage, Read-Only!) (Fragment)
                wgpu::BindGroupLayoutEntry {
                    binding: 3,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer { ty: wgpu::BufferBindingType::Storage { read_only: true }, has_dynamic_offset: false, min_binding_size: None },
                    count: None,
                },
                // 4: Light Indices (Storage, Read-Only!) (Fragment)
                wgpu::BindGroupLayoutEntry {
                    binding: 4,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer { ty: wgpu::BufferBindingType::Storage { read_only: true }, has_dynamic_offset: false, min_binding_size: None },
                    count: None,
                },
                // 5: Shadow Uniform
                wgpu::BindGroupLayoutEntry {
                    binding: 5,
                    visibility: wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer { ty: wgpu::BufferBindingType::Uniform, has_dynamic_offset: false, min_binding_size: None },
                    count: None,
                },
            ],
        });

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

        let shadow_map_bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("shadow_map_bind_group_layout"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture { 
                        sample_type: wgpu::TextureSampleType::Depth, 
                        view_dimension: wgpu::TextureViewDimension::D2Array, 
                        multisampled: false 
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Comparison),
                    count: None,
                },
            ],
        });

        let shadow_map_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("shadow_map_bind_group"),
            layout: &shadow_map_bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry { binding: 0, resource: wgpu::BindingResource::TextureView(&shadow_view) },
                wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::Sampler(&shadow_sampler) },
            ],
        });
        
        let vs_module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("main_vs.wgsl"),
            source: wgpu::ShaderSource::Wgsl(include_str!(concat!(env!("OUT_DIR"), "/main_vs.wgsl")).into()),
        });

        let fs_module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("main_fs.wgsl"),
            source: wgpu::ShaderSource::Wgsl(include_str!(concat!(env!("OUT_DIR"), "/main_fs.wgsl")).into()),
        });


        let depth_texture =
            texture::Texture::create_depth_texture(&device, &config, "depth_texture");

        let render_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("Render Pipeline Layout"),
                bind_group_layouts: &[
                    Some(&material_bind_group_layout),
                    Some(&camera_bind_group_layout),
                    Some(&shadow_map_bind_group_layout),
                ],
                immediate_size: 0,
            });

        let depth_fs_module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("depth_fs.wgsl"),
            source: wgpu::ShaderSource::Wgsl(include_str!(concat!(env!("OUT_DIR"), "/depth_fs.wgsl")).into()),
        });

        let depth_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Depth Prepass Pipeline"),
            layout: Some(&render_pipeline_layout),
            vertex: wgpu::VertexState {
                module: &vs_module,
                entry_point: Some("vs_main"),
                buffers: &[Some(model::ModelVertex::desc()), Some(InstanceRaw::desc())],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &depth_fs_module,
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
                format: texture::Texture::DEPTH_FORMAT,
                depth_write_enabled: Some(true), 
                depth_compare: Some(wgpu::CompareFunction::Less),
                stencil: wgpu::StencilState::default(),
                bias: wgpu::DepthBiasState::default(),
            }),
            multisample: wgpu::MultisampleState::default(),
            multiview_mask: None,
            cache: None,
        });

        let shadow_vs_module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("depth_fs.wgsl"),
            source: wgpu::ShaderSource::Wgsl(include_str!(concat!(env!("OUT_DIR"), "/shadow_vs.wgsl")).into()),
        });

        let cascade_bind_group_layout = Arc::new(device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("cascade_bind_group_layout"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX,
                ty: wgpu::BindingType::Buffer { 
                    ty: wgpu::BufferBindingType::Uniform, 
                    has_dynamic_offset: false, 
                    min_binding_size: Some(std::num::NonZeroU64::new(16).unwrap()), 
                },
                count: None,
            }],
        }));

        let mut cascade_bind_groups = Vec::new();
        for i in 0..4u32 {
            let buffer = device.create_buffer(&wgpu::BufferDescriptor {
                label: Some(&format!("Cascade Index Buffer {}", i)),
                size: 16,
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
            
            queue.write_buffer(&buffer, 0, bytemuck::cast_slice(&[i, 0, 0, 0]));

            let bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some(&format!("Cascade Index BG {}", i)),
                layout: &cascade_bind_group_layout,
                entries: &[wgpu::BindGroupEntry { 
                    binding: 0, 
                    resource: buffer.as_entire_binding() 
                }],
            });
            cascade_bind_groups.push(bg);
        }

        let shadow_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("Shadow Pipeline Layout"),
                bind_group_layouts: &[
                    Some(&material_bind_group_layout),
                    Some(&camera_bind_group_layout),
                    Some(&cascade_bind_group_layout),
                ],
                immediate_size: 0,
            });

        let shadow_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Shadow Pipeline"),
            layout: Some(&shadow_pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shadow_vs_module,
                entry_point: Some("vs_main"),
                buffers: &[Some(model::ModelVertex::desc()), Some(InstanceRaw::desc())],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &depth_fs_module, 
                entry_point: Some("depth_fs_main"),
                targets: &[],
                compilation_options: Default::default(),
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: Some(wgpu::Face::Front),
                polygon_mode: wgpu::PolygonMode::Fill,
                ..Default::default()
            },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: wgpu::TextureFormat::Depth32Float,
                depth_write_enabled: Some(true),
                depth_compare: Some(
                    wgpu::CompareFunction::LessEqual
                ),
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

        let render_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Render Pipeline"),
            layout: Some(&render_pipeline_layout),
            vertex: wgpu::VertexState {
                module: &vs_module,
                entry_point: Some("vs_main"),
                buffers: &[Some(model::ModelVertex::desc()), Some(InstanceRaw::desc())],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &fs_module,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format: config.format,
                    blend: Some(wgpu::BlendState {
                        color: wgpu::BlendComponent::REPLACE,
                        alpha: wgpu::BlendComponent::REPLACE,
                    }),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                strip_index_format: None,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: Some(wgpu::Face::Back),
                polygon_mode: wgpu::PolygonMode::Fill,
                unclipped_depth: false,
                conservative: false,
            },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: texture::Texture::DEPTH_FORMAT,
                depth_write_enabled: Some(false),
                depth_compare: Some(wgpu::CompareFunction::LessEqual),
                stencil: wgpu::StencilState::default(),
                bias: wgpu::DepthBiasState::default(),
            }),
            multisample: wgpu::MultisampleState {
                count: 1,
                mask: !0,
                alpha_to_coverage_enabled: false,
            },
            multiview_mask: None,
            cache: None,
        });

        Ok(Self {
            egui_renderer,
            depth_texture,
            instance_buffer,
            instance_buffer_capacity: initial_capacity,
            camera_bind_group,
            compute_bind_group,
            compute_pipeline,
            sorted_lights: Vec::with_capacity(crate::light::MAX_LIGHTS),
            lights_buffer,
            cluster_data_buffer,
            cluster_grid_buffer,
            light_indices_buffer,
            cascade_views,
            camera_buffer,
            shadow_uniform_buffer,
            cascade_bind_group_layout,
            cascade_bind_groups,
            shadow_map_bind_group,
            depth_pipeline,
            shadow_pipeline,
            render_pipeline,
            is_surface_configured: true,
            config,
            instances_cache: Vec::with_capacity(10000),
            draw_calls_cache: Vec::with_capacity(1000),
            material_bind_group_layout,
            queue,
            device,
            surface,
            instance,
        })
    }

    pub fn resize(&mut self, width: u32, height: u32) {
        if width > 0 && height > 0 {
            self.config.width = width;
            self.config.height = height;
            self.is_surface_configured = true;
            self.surface.configure(&self.device, &self.config);
            self.depth_texture =
                texture::Texture::create_depth_texture(&self.device, &self.config, "depth_texture");
        } else {
            self.is_surface_configured = false;
        }
    }

    pub fn upload_camera(&mut self, camera_uniform: CameraUniform) {
        self.queue.write_buffer(
            &self.camera_buffer,
            0,
            bytemuck::cast_slice(&[camera_uniform]),
        );
    }

    pub fn upload_instances(&mut self, instances: &[InstanceRaw]) -> bool {
        let mut reallocated = false;
        
        if instances.len() > self.instance_buffer_capacity {
            let new_capacity = instances.len() + instances.len() / 2;
            self.instance_buffer = Self::create_instance_buffer(&self.device, new_capacity);
            self.instance_buffer_capacity = new_capacity;
            reallocated = true;
        }
        
        self.queue.write_buffer(
            &self.instance_buffer,
            0,
            bytemuck::cast_slice(instances),
        );
        
        reallocated
    }

    pub fn upload_lights(&mut self, lights: &[LightUniform]) {
        self.sorted_lights.clear();
        self.sorted_lights.extend_from_slice(lights);
        
        self.sorted_lights.sort_unstable_by_key(|l| l.light_type);

        let count = self.sorted_lights.len().min(light::MAX_LIGHTS);
        
        self.queue.write_buffer(
            &self.lights_buffer,
            0,
            bytemuck::cast_slice(&self.sorted_lights[..count]),
        );
    }

    pub fn render_batches(
        &mut self,
        camera: &CameraUniform,
        batches: &[(Arc<model::Model>, usize, Vec<InstanceRaw>)],
        egui_primitives: &[egui::ClippedPrimitive],
        egui_screen_descriptor: &egui_wgpu::ScreenDescriptor,
    ) -> anyhow::Result<()> {
        if !self.is_surface_configured {
            return Ok(());
        }

        self.instances_cache.clear();
        self.draw_calls_cache.clear();

        let shadow_splits = [0.1, 10.0, 30.0, 100.0, 300.0];
        let light_dir = self.sorted_lights.iter().find(|l| l.light_type == 0).map_or(glam::Vec3::new(0.0, -1.0, 0.3), |l| glam::Vec3::from(l.direction));
        let fov_y = (1.0 / camera.inv_proj_matrix.y_axis.y).atan() * 2.0;

        let shadow_uniform = shadow::calculate_cascade_matrices(
            camera.view_matrix,
            fov_y,
            self.config.width as f32 / self.config.height as f32,
            light_dir,
            shadow_splits
        );
        self.queue.write_buffer(&self.shadow_uniform_buffer, 0, bytemuck::cast_slice(&[shadow_uniform]));

        let cam_frustum = Frustum::from_view_proj(camera.view_proj);
        let cascade_frustums = [
            Frustum::from_view_proj(shadow_uniform.light_space_matrices[0]),
            Frustum::from_view_proj(shadow_uniform.light_space_matrices[1]),
            Frustum::from_view_proj(shadow_uniform.light_space_matrices[2]),
            Frustum::from_view_proj(shadow_uniform.light_space_matrices[3]),
        ];

        let mut shadow_draw_calls: [Vec<(Arc<model::Model>, usize, std::ops::Range<u32>)>; 4] = Default::default();

        for (model, mesh_idx, instances) in batches {
            let count = instances.len() as u32;
            if count == 0 { continue; }

            let mesh = &model.meshes[*mesh_idx];
            let local_center = glam::Vec3::from(mesh.bounding_center);
            let local_radius = mesh.bounding_radius;

            let mut cam_visible = Vec::new();
            let mut casc_visible = [Vec::new(), Vec::new(), Vec::new(), Vec::new()];

            for inst in instances {
                let scale = glam::Vec3::new(
                    inst.model.x_axis.truncate().length(),
                    inst.model.y_axis.truncate().length(),
                    inst.model.z_axis.truncate().length()
                ).max_element();

                let world_center = inst.model.transform_point3(local_center);
                let world_radius = local_radius * scale;

                if cam_frustum.intersects_sphere(world_center, world_radius) {
                    cam_visible.push(*inst);
                }
                
                for i in 0..4 {
                    if cascade_frustums[i].intersects_sphere(world_center, world_radius) {
                        casc_visible[i].push(*inst);
                    }
                }
            }

            if !cam_visible.is_empty() {
                let start = self.instances_cache.len() as u32;
                self.instances_cache.extend_from_slice(&cam_visible);
                self.draw_calls_cache.push((model.clone(), *mesh_idx, start..(start + cam_visible.len() as u32)));
            }

            for i in 0..4 {
                if !casc_visible[i].is_empty() {
                    let start = self.instances_cache.len() as u32;
                    self.instances_cache.extend_from_slice(&casc_visible[i]);
                    shadow_draw_calls[i].push((model.clone(), *mesh_idx, start..(start + casc_visible[i].len() as u32)));
                }
            }
        }

        let instances = std::mem::take(&mut self.instances_cache);
        self.upload_instances(&instances);
        self.instances_cache = instances;

        let shadow_splits = [0.1, 10.0, 30.0, 100.0, 300.0];
        let light_dir = self.sorted_lights.iter().find(|l| l.light_type == 0).map_or(glam::Vec3::new(0.0, -1.0, 0.3), |l| glam::Vec3::from(l.direction));
        let fov_y = (1.0 / camera.inv_proj_matrix.y_axis.y).atan() * 2.0;

        let shadow_uniform = shadow::calculate_cascade_matrices(
            camera.view_matrix, 
            fov_y,
            self.config.width as f32 / self.config.height as f32,
            light_dir, 
            shadow_splits
        );
        self.queue.write_buffer(&self.shadow_uniform_buffer, 0, bytemuck::cast_slice(&[shadow_uniform]));

        let output = match self.surface.get_current_texture() {
            wgpu::CurrentSurfaceTexture::Success(surface_texture) => surface_texture,
            wgpu::CurrentSurfaceTexture::Suboptimal(surface_texture) => surface_texture,
            wgpu::CurrentSurfaceTexture::Timeout
            | wgpu::CurrentSurfaceTexture::Occluded
            | wgpu::CurrentSurfaceTexture::Validation => return Ok(()),
            wgpu::CurrentSurfaceTexture::Outdated => {
                self.surface.configure(&self.device, &self.config);
                return Ok(());
            }
            wgpu::CurrentSurfaceTexture::Lost => {
                anyhow::bail!("Lost device");
            }
        };
        let view = output
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());

        let max_l = light::MAX_LIGHTS;
        let total_lights = self.sorted_lights.len().min(max_l) as u32;
        let dir_light_count = self.sorted_lights
            .iter()
            .take(max_l)
            .filter(|l| l.light_type == 0)
            .count() as u32;

        let cluster_data = ClusterData {
            grid_dim: [GRID_X, GRID_Y, GRID_Z],
            z_near: 0.1,
            z_far: 1000.0,
            tile_size: [self.config.width as f32 / GRID_X as f32, self.config.height as f32 / GRID_Y as f32],
            screen_size: [self.config.width as f32, self.config.height as f32],
            total_lights,
            dir_light_count,
            padding: 0,
        };
        self.queue.write_buffer(&self.cluster_data_buffer, 0, bytemuck::cast_slice(&[cluster_data]));

        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("Render Encoder"),
            });

        self.egui_renderer.update_buffers(
            &self.device,
            &self.queue,
            &mut encoder,
            egui_primitives,
            egui_screen_descriptor,
        );

        for i in 0..CASCADE_COUNT {
            let mut shadow_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some(&format!("Shadow Pass Cascade {}", i)),
                color_attachments: &[],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &self.cascade_views[i],
                    depth_ops: Some(wgpu::Operations { 
                        load: wgpu::LoadOp::Clear(1.0), 
                        store: wgpu::StoreOp::Store 
                    }),
                    stencil_ops: None,
                }),
                ..Default::default()
            });

            shadow_pass.set_pipeline(&self.shadow_pipeline);
            
            shadow_pass.set_bind_group(1, &self.camera_bind_group, &[]);
            shadow_pass.set_bind_group(2, &self.cascade_bind_groups[i], &[]);

            shadow_pass.set_vertex_buffer(1, self.instance_buffer.slice(..));

            for (model, mesh_idx, range) in &shadow_draw_calls[i] {
                let mesh = &model.meshes[*mesh_idx];
                let material = &model.materials[mesh.material];
                
                shadow_pass.set_vertex_buffer(0, mesh.vertex_buffer.slice(..));
                shadow_pass.set_index_buffer(mesh.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
                shadow_pass.set_bind_group(0, &material.bind_group, &[]); 
                shadow_pass.draw_indexed(0..mesh.num_elements, 0, range.clone());
            }
        }

        // compute pass (culling lights)
        {
            let mut compute_pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor::default());
            compute_pass.set_pipeline(&self.compute_pipeline);
            compute_pass.set_bind_group(0, &self.compute_bind_group, &[]);
            compute_pass.dispatch_workgroups((GRID_X + 7) / 8, (GRID_Y + 7) / 8, GRID_Z);
        }

        // z-prepass
        {
            let mut depth_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Depth Prepass"),
                color_attachments: &[],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &self.depth_texture.view,
                    depth_ops: Some(wgpu::Operations { load: wgpu::LoadOp::Clear(1.0), store: wgpu::StoreOp::Store }),
                    stencil_ops: None,
                }),
                ..Default::default()
            });

            depth_pass.set_pipeline(&self.depth_pipeline);
            depth_pass.set_vertex_buffer(1, self.instance_buffer.slice(..));
            depth_pass.set_bind_group(1, &self.camera_bind_group, &[]);
            depth_pass.set_bind_group(2, &self.shadow_map_bind_group, &[]);

            for (model, mesh_idx, range) in &self.draw_calls_cache {
                let mesh = &model.meshes[*mesh_idx];
                let material = &model.materials[mesh.material];
                depth_pass.set_vertex_buffer(0, mesh.vertex_buffer.slice(..));
                depth_pass.set_index_buffer(mesh.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
                depth_pass.set_bind_group(0, &material.bind_group, &[]);
                depth_pass.draw_indexed(0..mesh.num_elements, 0, range.clone());
            }
        }

        // color pass
        {
            let mut color_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Color Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations { load: wgpu::LoadOp::Clear(wgpu::Color { r: 0.1, g: 0.2, b: 0.3, a: 1.0 }), store: wgpu::StoreOp::Store },
                    depth_slice: None,
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &self.depth_texture.view,
                    depth_ops: Some(wgpu::Operations { load: wgpu::LoadOp::Load, store: wgpu::StoreOp::Store }),
                    stencil_ops: None,
                }),
                ..Default::default()
            });

            color_pass.set_pipeline(&self.render_pipeline);
            color_pass.set_vertex_buffer(1, self.instance_buffer.slice(..));

            color_pass.set_bind_group(1, &self.camera_bind_group, &[]);
            color_pass.set_bind_group(1, &self.camera_bind_group, &[]);
            color_pass.set_bind_group(2, &self.shadow_map_bind_group, &[]);

            for (model, mesh_idx, range) in &self.draw_calls_cache {
                let mesh = &model.meshes[*mesh_idx];
                let material = &model.materials[mesh.material];
                color_pass.set_vertex_buffer(0, mesh.vertex_buffer.slice(..));
                color_pass.set_index_buffer(mesh.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
                color_pass.set_bind_group(0, &material.bind_group, &[]);
                color_pass.draw_indexed(0..mesh.num_elements, 0, range.clone());
            }
        }
        
        {
            let egui_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Egui Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Load,
                        store: wgpu::StoreOp::Store,
                    },
                    depth_slice: None,
                })],
                depth_stencil_attachment: None,
                occlusion_query_set: None,
                timestamp_writes: None,
                multiview_mask: None,
            });
            
            let mut static_egui_pass = egui_pass.forget_lifetime();

            self.egui_renderer.render(
                &mut static_egui_pass, 
                egui_primitives, 
                egui_screen_descriptor
            );
        }

        self.queue.submit(std::iter::once(encoder.finish()));
        self.queue.present(output);

        Ok(())
    }

    fn create_instance_buffer(device: &wgpu::Device, capacity: usize) -> wgpu::Buffer {
        device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Instance Buffer"),
            size: (capacity * std::mem::size_of::<InstanceRaw>()) as u64,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        })
    }
}