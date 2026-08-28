pub mod model;
pub mod resources;
pub mod texture;
pub mod mipmap;
pub mod light;
pub mod shadow;
pub mod spot_shadow;
pub mod point_shadow;
pub mod postprocess;
pub mod bloom;
pub mod frustum;
pub mod shadow_system;
pub mod light_cluster;
pub mod pipelines;

use glam::{Mat4, Vec4};
use std::sync::Arc;

pub use frustum::Frustum;
use light::LightUniform;
use postprocess::PostProcessSettings;

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
pub struct InstanceRaw {
    #[allow(dead_code)]
    pub model: Mat4,
    pub normal_matrix: Mat4,
}

impl InstanceRaw {
    pub fn new(model_matrix: Mat4) -> Self {
        let normal_matrix = model_matrix.inverse().transpose();
        Self { model: model_matrix, normal_matrix }
    }

    fn desc() -> wgpu::VertexBufferLayout<'static> {
        use std::mem;
        wgpu::VertexBufferLayout {
            array_stride: mem::size_of::<InstanceRaw>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Instance,
            attributes: &[
                wgpu::VertexAttribute { offset: 0, shader_location: 5, format: wgpu::VertexFormat::Float32x4 },
                wgpu::VertexAttribute { offset: mem::size_of::<Vec4>() as wgpu::BufferAddress, shader_location: 6, format: wgpu::VertexFormat::Float32x4 },
                wgpu::VertexAttribute { offset: (mem::size_of::<Vec4>() * 2) as wgpu::BufferAddress, shader_location: 7, format: wgpu::VertexFormat::Float32x4 },
                wgpu::VertexAttribute { offset: (mem::size_of::<Vec4>() * 3) as wgpu::BufferAddress, shader_location: 8, format: wgpu::VertexFormat::Float32x4 },
                wgpu::VertexAttribute { offset: (mem::size_of::<Vec4>() * 4) as wgpu::BufferAddress, shader_location: 9, format: wgpu::VertexFormat::Float32x4 },
                wgpu::VertexAttribute { offset: (mem::size_of::<Vec4>() * 5) as wgpu::BufferAddress, shader_location: 10, format: wgpu::VertexFormat::Float32x4 },
                wgpu::VertexAttribute { offset: (mem::size_of::<Vec4>() * 6) as wgpu::BufferAddress, shader_location: 11, format: wgpu::VertexFormat::Float32x4 },
                wgpu::VertexAttribute { offset: (mem::size_of::<Vec4>() * 7) as wgpu::BufferAddress, shader_location: 12, format: wgpu::VertexFormat::Float32x4 },
            ],
        }
    }
}

pub struct InstanceBuffer {
    pub buffer: wgpu::Buffer,
    pub capacity: usize,
}

impl InstanceBuffer {
    pub fn new(device: &wgpu::Device, capacity: usize) -> Self {
        let buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Instance Buffer"),
            size: (capacity * std::mem::size_of::<InstanceRaw>()) as u64,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        Self { buffer, capacity }
    }

    pub fn upload(&mut self, device: &wgpu::Device, queue: &wgpu::Queue, instances: &[InstanceRaw]) -> bool {
        let mut reallocated = false;
        if instances.len() > self.capacity {
            let new_capacity = instances.len() + instances.len() / 2;
            self.buffer = device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("Instance Buffer"),
                size: (new_capacity * std::mem::size_of::<InstanceRaw>()) as u64,
                usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
            self.capacity = new_capacity;
            reallocated = true;
        }

        queue.write_buffer(
            &self.buffer,
            0,
            bytemuck::cast_slice(instances),
        );

        reallocated
    }
}

pub struct Renderer {
    pub device: Arc<wgpu::Device>,
    pub queue: Arc<wgpu::Queue>,
    surface: wgpu::Surface<'static>,
    pub config: wgpu::SurfaceConfiguration,
    pub is_surface_configured: bool,

    // Subsystems
    pub shadow_system: shadow_system::ShadowSystem,
    pub light_cluster: light_cluster::LightClusterSystem,
    pub pipelines: pipelines::RenderPipelines,

    // Textures & MSAA
    pub msaa_samples: u32,
    pub depth_texture: texture::Texture,
    pub hdr_texture: wgpu::Texture,
    pub hdr_view: wgpu::TextureView,
    msaa_hdr_texture: Option<wgpu::Texture>,
    msaa_hdr_view: Option<wgpu::TextureView>,
    msaa_depth_texture: Option<wgpu::Texture>,
    msaa_depth_view: Option<wgpu::TextureView>,

    // Bloom & Postprocess
    bloom_pyramid: bloom::BloomPyramid,
    bloom_bind_group_layout: Arc<wgpu::BindGroupLayout>,
    bloom_downsample_pipeline: wgpu::RenderPipeline,
    bloom_upsample_pipeline: wgpu::RenderPipeline,

    pub postprocess_settings: PostProcessSettings,
    postprocess_uniform_buffer: wgpu::Buffer,
    postprocess_bind_group_layout: Arc<wgpu::BindGroupLayout>,
    postprocess_bind_group: wgpu::BindGroup,
    postprocess_pipeline: wgpu::RenderPipeline,
    postprocess_sampler: wgpu::Sampler,
    total_time: f32,

    // UI
    pub egui_renderer: egui_wgpu::Renderer,

    // Instances & Caches
    pub instance_buffer: InstanceBuffer,
    instances_cache: Vec<InstanceRaw>,
    draw_calls_cache: Vec<(Arc<model::Model>, usize, std::ops::Range<u32>)>,
    cam_scratch: Vec<InstanceRaw>,

    pub material_bind_group_layout: Arc<wgpu::BindGroupLayout>,
    #[allow(dead_code)]
    instance: wgpu::Instance,
}

impl Renderer {
    pub const HDR_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba16Float;

    fn create_msaa_hdr(device: &wgpu::Device, width: u32, height: u32, samples: u32) -> (wgpu::Texture, wgpu::TextureView) {
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("MSAA HDR Texture"),
            size: wgpu::Extent3d { width: width.max(1), height: height.max(1), depth_or_array_layers: 1 },
            mip_level_count: 1,
            sample_count: samples,
            dimension: wgpu::TextureDimension::D2,
            format: Self::HDR_FORMAT,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            view_formats: &[],
        });
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        (texture, view)
    }

    fn create_msaa_depth(device: &wgpu::Device, width: u32, height: u32, samples: u32) -> (wgpu::Texture, wgpu::TextureView) {
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("MSAA Depth Texture"),
            size: wgpu::Extent3d { width: width.max(1), height: height.max(1), depth_or_array_layers: 1 },
            mip_level_count: 1,
            sample_count: samples,
            dimension: wgpu::TextureDimension::D2,
            format: texture::Texture::DEPTH_FORMAT,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            view_formats: &[],
        });
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        (texture, view)
    }

    fn create_hdr_texture(device: &wgpu::Device, width: u32, height: u32) -> (wgpu::Texture, wgpu::TextureView) {
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("HDR Color Texture"),
            size: wgpu::Extent3d { width: width.max(1), height: height.max(1), depth_or_array_layers: 1 },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: Self::HDR_FORMAT,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });
        let view = texture.create_view(&wgpu::TextureViewDescriptor {
            label: Some("HDR Color Texture View"),
            ..Default::default()
        });
        (texture, view)
    }

    fn create_postprocess_bind_group(
        device: &wgpu::Device,
        layout: &wgpu::BindGroupLayout,
        uniform_buffer: &wgpu::Buffer,
        hdr_view: &wgpu::TextureView,
        sampler: &wgpu::Sampler,
        bloom_view: &wgpu::TextureView,
    ) -> wgpu::BindGroup {
        device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Postprocess Bind Group"),
            layout,
            entries: &[
                wgpu::BindGroupEntry { binding: 0, resource: uniform_buffer.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::TextureView(hdr_view) },
                wgpu::BindGroupEntry { binding: 2, resource: wgpu::BindingResource::Sampler(sampler) },
                wgpu::BindGroupEntry { binding: 3, resource: wgpu::BindingResource::TextureView(bloom_view) },
            ],
        })
    }

    pub async fn new(
        window: impl Into<wgpu::SurfaceTarget<'static>>,
        width: u32,
        height: u32,
        msaa_samples: u32,
    ) -> anyhow::Result<Self> {
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
            backends: wgpu::Backends::PRIMARY,
            flags: Default::default(),
            memory_budget_thresholds: Default::default(),
            backend_options: Default::default(),
            display: None,
        });

        let surface = instance.create_surface(window).unwrap();
        let adapter = instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::default(),
            compatible_surface: Some(&surface),
            force_fallback_adapter: false,
            apply_limit_buckets: true,
        }).await.unwrap();

        let (device, queue) = adapter.request_device(&wgpu::DeviceDescriptor {
            label: None,
            required_features: wgpu::Features::INDIRECT_FIRST_INSTANCE | wgpu::Features::IMMEDIATES,
            experimental_features: wgpu::ExperimentalFeatures::disabled(),
            required_limits: wgpu::Limits::default(),
            memory_hints: Default::default(),
            trace: wgpu::Trace::Off,
        }).await.unwrap();

        let device = Arc::new(device);
        let queue = Arc::new(queue);

        let surface_caps = surface.get_capabilities(&adapter);
        let surface_format = surface_caps.formats.iter().copied().find(|f| f.is_srgb()).unwrap_or(surface_caps.formats[0]);

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

        let material_bind_group_layout = Arc::new(device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("material_bind_group_layout"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer { ty: wgpu::BufferBindingType::Uniform, has_dynamic_offset: false, min_binding_size: None },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture { multisampled: false, view_dimension: wgpu::TextureViewDimension::D2, sample_type: wgpu::TextureSampleType::Float { filterable: true } },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture { multisampled: false, view_dimension: wgpu::TextureViewDimension::D2, sample_type: wgpu::TextureSampleType::Float { filterable: true } },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 3,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture { multisampled: false, view_dimension: wgpu::TextureViewDimension::D2, sample_type: wgpu::TextureSampleType::Float { filterable: true } },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 4,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        }));

        let depth_fs_module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("depth_fs.wgsl"),
            source: wgpu::ShaderSource::Wgsl(include_str!(concat!(env!("OUT_DIR"), "/depth_fs.wgsl")).into()),
        });

        let shadow_system = shadow_system::ShadowSystem::new(&device, &material_bind_group_layout, &depth_fs_module);
        let light_cluster = light_cluster::LightClusterSystem::new(&device, &shadow_system.shadow_uniform_buffer);

        let msaa_samples = match msaa_samples {
            2 => 2,
            4 => 4,
            _ => 1,
        };

        let pipelines = pipelines::RenderPipelines::new(
            &device,
            &material_bind_group_layout,
            &light_cluster.camera_bind_group_layout,
            &shadow_system.shadow_map_bind_group_layout,
            msaa_samples,
        );

        let initial_capacity = 100;
        let instance_buffer = InstanceBuffer::new(&device, initial_capacity);
        let depth_texture = texture::Texture::create_depth_texture(&device, &config, "depth_texture");
        let (hdr_texture, hdr_view) = Self::create_hdr_texture(&device, config.width, config.height);

        let (msaa_hdr_texture, msaa_hdr_view) = if msaa_samples > 1 {
            let (tex, view) = Self::create_msaa_hdr(&device, width, height, msaa_samples);
            (Some(tex), Some(view))
        } else {
            (None, None)
        };

        let (msaa_depth_texture, msaa_depth_view) = if msaa_samples > 1 {
            let (tex, view) = Self::create_msaa_depth(&device, width, height, msaa_samples);
            (Some(tex), Some(view))
        } else {
            (None, None)
        };

        let postprocess_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("Postprocess Sampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::MipmapFilterMode::Nearest,
            ..Default::default()
        });

        let bloom_bind_group_layout = Arc::new(device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Bloom Bind Group Layout"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer { ty: wgpu::BufferBindingType::Uniform, has_dynamic_offset: false, min_binding_size: None },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture { sample_type: wgpu::TextureSampleType::Float { filterable: true }, view_dimension: wgpu::TextureViewDimension::D2, multisampled: false },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        }));

        let bloom_pyramid = bloom::BloomPyramid::new(&device, &bloom_bind_group_layout, &postprocess_sampler, &hdr_view, config.width, config.height);

        let bloom_pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Bloom Pipeline Layout"),
            bind_group_layouts: &[Some(&bloom_bind_group_layout)],
            immediate_size: 0,
        });

        let bloom_vs_module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("bloom_vs.wgsl"),
            source: wgpu::ShaderSource::Wgsl(include_str!(concat!(env!("OUT_DIR"), "/bloom_vs.wgsl")).into()),
        });

        let bloom_down_fs_module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("bloom_down_fs.wgsl"),
            source: wgpu::ShaderSource::Wgsl(include_str!(concat!(env!("OUT_DIR"), "/bloom_down_fs.wgsl")).into()),
        });

        let bloom_up_fs_module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("bloom_up_fs.wgsl"),
            source: wgpu::ShaderSource::Wgsl(include_str!(concat!(env!("OUT_DIR"), "/bloom_up_fs.wgsl")).into()),
        });

        let bloom_downsample_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Bloom Downsample Pipeline"),
            layout: Some(&bloom_pipeline_layout),
            vertex: wgpu::VertexState { module: &bloom_vs_module, entry_point: Some("vs_main"), buffers: &[], compilation_options: Default::default() },
            fragment: Some(wgpu::FragmentState {
                module: &bloom_down_fs_module,
                entry_point: Some("fs_downsample"),
                targets: &[Some(wgpu::ColorTargetState { format: wgpu::TextureFormat::Rgba16Float, blend: Some(wgpu::BlendState::REPLACE), write_mask: wgpu::ColorWrites::ALL })],
                compilation_options: Default::default(),
            }),
            primitive: wgpu::PrimitiveState { topology: wgpu::PrimitiveTopology::TriangleList, cull_mode: None, ..Default::default() },
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview_mask: None,
            cache: None,
        });

        let bloom_upsample_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Bloom Upsample Pipeline (Additive)"),
            layout: Some(&bloom_pipeline_layout),
            vertex: wgpu::VertexState { module: &bloom_vs_module, entry_point: Some("vs_main"), buffers: &[], compilation_options: Default::default() },
            fragment: Some(wgpu::FragmentState {
                module: &bloom_up_fs_module,
                entry_point: Some("fs_upsample"),
                targets: &[Some(wgpu::ColorTargetState {
                    format: wgpu::TextureFormat::Rgba16Float,
                    blend: Some(wgpu::BlendState {
                        color: wgpu::BlendComponent { src_factor: wgpu::BlendFactor::One, dst_factor: wgpu::BlendFactor::One, operation: wgpu::BlendOperation::Add },
                        alpha: wgpu::BlendComponent::REPLACE,
                    }),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            primitive: wgpu::PrimitiveState { topology: wgpu::PrimitiveTopology::TriangleList, cull_mode: None, ..Default::default() },
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview_mask: None,
            cache: None,
        });

        let postprocess_uniform_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Postprocess Uniform Buffer"),
            size: std::mem::size_of::<postprocess::PostProcessUniform>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let postprocess_bind_group_layout = Arc::new(device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Postprocess Bind Group Layout"),
            entries: &[
                wgpu::BindGroupLayoutEntry { binding: 0, visibility: wgpu::ShaderStages::FRAGMENT, ty: wgpu::BindingType::Buffer { ty: wgpu::BufferBindingType::Uniform, has_dynamic_offset: false, min_binding_size: None }, count: None },
                wgpu::BindGroupLayoutEntry { binding: 1, visibility: wgpu::ShaderStages::FRAGMENT, ty: wgpu::BindingType::Texture { sample_type: wgpu::TextureSampleType::Float { filterable: true }, view_dimension: wgpu::TextureViewDimension::D2, multisampled: false }, count: None },
                wgpu::BindGroupLayoutEntry { binding: 2, visibility: wgpu::ShaderStages::FRAGMENT, ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering), count: None },
                wgpu::BindGroupLayoutEntry { binding: 3, visibility: wgpu::ShaderStages::FRAGMENT, ty: wgpu::BindingType::Texture { sample_type: wgpu::TextureSampleType::Float { filterable: true }, view_dimension: wgpu::TextureViewDimension::D2, multisampled: false }, count: None },
            ],
        }));

        let postprocess_bind_group = Self::create_postprocess_bind_group(
            &device,
            &postprocess_bind_group_layout,
            &postprocess_uniform_buffer,
            &hdr_view,
            &postprocess_sampler,
            &bloom_pyramid.mips[0].view,
        );

        let postprocess_pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Postprocess Pipeline Layout"),
            bind_group_layouts: &[Some(&postprocess_bind_group_layout)],
            immediate_size: 0,
        });

        let postprocess_vs_module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("postprocess_vs.wgsl"),
            source: wgpu::ShaderSource::Wgsl(include_str!(concat!(env!("OUT_DIR"), "/postprocess_vs.wgsl")).into()),
        });

        let postprocess_fs_module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("postprocess_fs.wgsl"),
            source: wgpu::ShaderSource::Wgsl(include_str!(concat!(env!("OUT_DIR"), "/postprocess_fs.wgsl")).into()),
        });

        let postprocess_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Postprocess Composite Pipeline"),
            layout: Some(&postprocess_pipeline_layout),
            vertex: wgpu::VertexState { module: &postprocess_vs_module, entry_point: Some("vs_main"), buffers: &[], compilation_options: Default::default() },
            fragment: Some(wgpu::FragmentState {
                module: &postprocess_fs_module,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState { format: config.format, blend: Some(wgpu::BlendState::REPLACE), write_mask: wgpu::ColorWrites::ALL })],
                compilation_options: Default::default(),
            }),
            primitive: wgpu::PrimitiveState { topology: wgpu::PrimitiveTopology::TriangleList, cull_mode: None, ..Default::default() },
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview_mask: None,
            cache: None,
        });

        Ok(Self {
            device,
            queue,
            surface,
            config,
            is_surface_configured: true,

            shadow_system,
            light_cluster,
            pipelines,

            msaa_samples,
            depth_texture,
            hdr_texture,
            hdr_view,
            msaa_hdr_texture,
            msaa_hdr_view,
            msaa_depth_texture,
            msaa_depth_view,

            bloom_pyramid,
            bloom_bind_group_layout,
            bloom_downsample_pipeline,
            bloom_upsample_pipeline,

            postprocess_settings: PostProcessSettings::default(),
            postprocess_uniform_buffer,
            postprocess_bind_group_layout,
            postprocess_bind_group,
            postprocess_pipeline,
            postprocess_sampler,
            total_time: 0.0,

            egui_renderer,

            instance_buffer,
            instances_cache: Vec::with_capacity(10000),
            draw_calls_cache: Vec::with_capacity(1000),
            cam_scratch: Vec::with_capacity(1000),

            material_bind_group_layout,
            instance,
        })
    }

    pub fn set_msaa_samples(&mut self, samples: u32) {
        let valid_samples = match samples {
            2 => 2,
            4 => 4,
            _ => 1,
        };

        if self.msaa_samples == valid_samples {
            return;
        }

        self.msaa_samples = valid_samples;

        if self.msaa_samples > 1 {
            let (tex, view) = Self::create_msaa_hdr(&self.device, self.config.width, self.config.height, self.msaa_samples);
            self.msaa_hdr_texture = Some(tex);
            self.msaa_hdr_view = Some(view);

            let (d_tex, d_view) = Self::create_msaa_depth(&self.device, self.config.width, self.config.height, self.msaa_samples);
            self.msaa_depth_texture = Some(d_tex);
            self.msaa_depth_view = Some(d_view);
        } else {
            self.msaa_hdr_texture = None;
            self.msaa_hdr_view = None;
            self.msaa_depth_texture = None;
            self.msaa_depth_view = None;
        }

        self.pipelines.rebuild_for_msaa(&self.device, self.msaa_samples);
    }

    pub fn resize(&mut self, width: u32, height: u32) {
        if width > 0 && height > 0 {
            self.config.width = width;
            self.config.height = height;
            self.is_surface_configured = true;
            self.surface.configure(&self.device, &self.config);
            self.depth_texture = texture::Texture::create_depth_texture(&self.device, &self.config, "depth_texture");

            let (hdr_tex, hdr_view) = Self::create_hdr_texture(&self.device, width, height);
            self.hdr_texture = hdr_tex;
            self.hdr_view = hdr_view;

            if self.msaa_samples > 1 {
                let (tex, view) = Self::create_msaa_hdr(&self.device, width, height, self.msaa_samples);
                self.msaa_hdr_texture = Some(tex);
                self.msaa_hdr_view = Some(view);

                let (d_tex, d_view) = Self::create_msaa_depth(&self.device, width, height, self.msaa_samples);
                self.msaa_depth_texture = Some(d_tex);
                self.msaa_depth_view = Some(d_view);
            }

            self.bloom_pyramid = bloom::BloomPyramid::new(
                &self.device,
                &self.bloom_bind_group_layout,
                &self.postprocess_sampler,
                &self.hdr_view,
                width,
                height,
            );

            self.postprocess_bind_group = Self::create_postprocess_bind_group(
                &self.device,
                &self.postprocess_bind_group_layout,
                &self.postprocess_uniform_buffer,
                &self.hdr_view,
                &self.postprocess_sampler,
                &self.bloom_pyramid.mips[0].view,
            );
        } else {
            self.is_surface_configured = false;
        }
    }

    pub fn upload_camera(&mut self, camera_uniform: CameraUniform) {
        self.light_cluster.upload_camera(&self.queue, &camera_uniform);
    }

    pub fn upload_instances(&mut self, instances: &[InstanceRaw]) -> bool {
        self.instance_buffer.upload(&self.device, &self.queue, instances)
    }

    pub fn upload_lights(&mut self, lights: &[LightUniform], camera: &CameraUniform) {
        self.light_cluster.upload_lights(&self.queue, lights, camera);
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
        self.shadow_system.clear_caches();

        // 1. Prepare Shadows (без единой аллокации в куче)
        self.shadow_system.prepare_shadow_uniforms(
            &self.queue,
            &self.light_cluster.sorted_lights,
            camera,
            self.config.width,
            self.config.height,
        );

        let cam_frustum = Frustum::from_view_proj(camera.view_proj);

        // Раздельное заимствование полей ShadowSystem для предотвращения конфликтов Borrow Checker
        let shadow_system::ShadowSystem {
            ref cascade_frustums,
            ref active_spot_frustums,
            ref active_point_lights,
            ref mut casc_scratch,
            ref mut spot_scratch,
            ref mut point_scratch,
            ref mut shadow_draw_calls_cache,
            ref mut spot_shadow_draw_calls_cache,
            ref mut point_shadow_draw_calls_cache,
            ..
        } = self.shadow_system;

        // 2. Frustum Culling and Draw Call generation
        for (model, mesh_idx, instances) in batches {
            if instances.is_empty() { continue; }

            let mesh = &model.meshes[*mesh_idx];
            let local_center = glam::Vec3::from(mesh.bounding_center);
            let local_radius = mesh.bounding_radius;

            self.cam_scratch.clear();
            for s in &mut *casc_scratch { s.clear(); }
            for s in &mut *spot_scratch { s.clear(); }
            for s in &mut *point_scratch { s.clear(); }

            for inst in instances {
                let scale = glam::Vec3::new(
                    inst.model.x_axis.truncate().length(),
                    inst.model.y_axis.truncate().length(),
                    inst.model.z_axis.truncate().length(),
                ).max_element();

                let world_center = inst.model.transform_point3(local_center);
                let world_radius = local_radius * scale;

                // Main Camera
                if cam_frustum.intersects_sphere(world_center, world_radius) {
                    self.cam_scratch.push(*inst);
                }

                // Cascades
                for i in 0..4 {
                    if cascade_frustums[i].intersects_sphere(world_center, world_radius) {
                        casc_scratch[i].push(*inst);
                    }
                }

                // Spots
                for &(idx, ref frustum) in active_spot_frustums.iter() {
                    if frustum.intersects_sphere(world_center, world_radius) {
                        spot_scratch[idx].push(*inst);
                    }
                }

                // Point lights
                for (p_idx, l_pos, l_range, face_frustums) in active_point_lights.iter() {
                    let dist_sq = world_center.distance_squared(*l_pos);
                    let max_dist = world_radius + *l_range;
                    if dist_sq <= max_dist * max_dist {
                        for f in 0..6 {
                            if face_frustums[f].intersects_sphere(world_center, world_radius) {
                                let global_idx = p_idx * point_shadow::POINT_SHADOW_FACES + f;
                                point_scratch[global_idx].push(*inst);
                            }
                        }
                    }
                }
            }

            if !self.cam_scratch.is_empty() {
                let start = self.instances_cache.len() as u32;
                self.instances_cache.extend_from_slice(&self.cam_scratch);
                self.draw_calls_cache.push((
                    model.clone(),
                    *mesh_idx,
                    start..(start + self.cam_scratch.len() as u32),
                ));
            }

            for i in 0..4 {
                if !casc_scratch[i].is_empty() {
                    let start = self.instances_cache.len() as u32;
                    self.instances_cache.extend_from_slice(&casc_scratch[i]);
                    shadow_draw_calls_cache[i].push((
                        model.clone(),
                        *mesh_idx,
                        start..(start + casc_scratch[i].len() as u32),
                    ));
                }
            }

            for &(idx, _) in active_spot_frustums.iter() {
                if !spot_scratch[idx].is_empty() {
                    let start = self.instances_cache.len() as u32;
                    self.instances_cache.extend_from_slice(&spot_scratch[idx]);
                    spot_shadow_draw_calls_cache[idx].push((
                        model.clone(),
                        *mesh_idx,
                        start..(start + spot_scratch[idx].len() as u32),
                    ));
                }
            }

            for (p_idx, _, _, _) in active_point_lights.iter() {
                for f in 0..6 {
                    let global_idx = p_idx * point_shadow::POINT_SHADOW_FACES + f;
                    if !point_scratch[global_idx].is_empty() {
                        let start = self.instances_cache.len() as u32;
                        self.instances_cache.extend_from_slice(&point_scratch[global_idx]);
                        point_shadow_draw_calls_cache[global_idx].push((
                            model.clone(),
                            *mesh_idx,
                            start..(start + point_scratch[global_idx].len() as u32),
                        ));
                    }
                }
            }
        }

        self.instance_buffer.upload(&self.device, &self.queue, &self.instances_cache);

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
            wgpu::CurrentSurfaceTexture::Lost => anyhow::bail!("Lost device"),
        };
        let view = output.texture.create_view(&wgpu::TextureViewDescriptor::default());

        // Update Cluster Data Uniform
        self.light_cluster.update_cluster_data(&self.queue, self.config.width, self.config.height);

        let mut encoder = self.device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("Render Encoder"),
        });

        self.egui_renderer.update_buffers(
            &self.device,
            &self.queue,
            &mut encoder,
            egui_primitives,
            egui_screen_descriptor,
        );

        // --- 1. Shadow Passes ---
        self.shadow_system.record_render_passes(
            &mut encoder,
            &self.instance_buffer.buffer,
        );

        // --- 2. Compute Culling Pass ---
        self.light_cluster.dispatch_culling(&mut encoder);

        let active_depth_view = if self.msaa_samples > 1 {
            self.msaa_depth_view.as_ref().unwrap()
        } else {
            &self.depth_texture.view
        };

        // --- 3. Depth Prepass ---
        {
            let mut depth_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Depth Prepass"),
                color_attachments: &[],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: active_depth_view,
                    depth_ops: Some(wgpu::Operations { load: wgpu::LoadOp::Clear(1.0), store: wgpu::StoreOp::Store }),
                    stencil_ops: None,
                }),
                ..Default::default()
            });

            depth_pass.set_pipeline(&self.pipelines.depth_pipeline);
            depth_pass.set_vertex_buffer(1, self.instance_buffer.buffer.slice(..));
            depth_pass.set_bind_group(1, &self.light_cluster.camera_bind_group, &[]);
            depth_pass.set_bind_group(2, &self.shadow_system.shadow_map_bind_group, &[]);

            for (model, mesh_idx, range) in &self.draw_calls_cache {
                let mesh = &model.meshes[*mesh_idx];
                let material = &model.materials[mesh.material];
                depth_pass.set_vertex_buffer(0, mesh.vertex_buffer.slice(..));
                depth_pass.set_index_buffer(mesh.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
                depth_pass.set_bind_group(0, &material.bind_group, &[]);
                depth_pass.draw_indexed(0..mesh.num_elements, 0, range.clone());
            }
        }

        // --- 4. Main PBR Forward Pass (HDR + MSAA Resolve) ---
        {
            let color_attachment = if self.msaa_samples > 1 {
                wgpu::RenderPassColorAttachment {
                    view: self.msaa_hdr_view.as_ref().unwrap(),
                    resolve_target: Some(&self.hdr_view),
                    ops: wgpu::Operations { load: wgpu::LoadOp::Clear(wgpu::Color::BLACK), store: wgpu::StoreOp::Store },
                    depth_slice: None,
                }
            } else {
                wgpu::RenderPassColorAttachment {
                    view: &self.hdr_view,
                    resolve_target: None,
                    ops: wgpu::Operations { load: wgpu::LoadOp::Clear(wgpu::Color::BLACK), store: wgpu::StoreOp::Store },
                    depth_slice: None,
                }
            };

            let mut color_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Color Pass (HDR + MSAA Resolve)"),
                color_attachments: &[Some(color_attachment)],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: active_depth_view,
                    depth_ops: Some(wgpu::Operations { load: wgpu::LoadOp::Load, store: wgpu::StoreOp::Store }),
                    stencil_ops: None,
                }),
                ..Default::default()
            });

            color_pass.set_pipeline(&self.pipelines.render_pipeline);
            color_pass.set_vertex_buffer(1, self.instance_buffer.buffer.slice(..));
            color_pass.set_bind_group(1, &self.light_cluster.camera_bind_group, &[]);
            color_pass.set_bind_group(2, &self.shadow_system.shadow_map_bind_group, &[]);

            for (model, mesh_idx, range) in &self.draw_calls_cache {
                let mesh = &model.meshes[*mesh_idx];
                let material = &model.materials[mesh.material];
                color_pass.set_vertex_buffer(0, mesh.vertex_buffer.slice(..));
                color_pass.set_index_buffer(mesh.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
                color_pass.set_bind_group(0, &material.bind_group, &[]);
                color_pass.draw_indexed(0..mesh.num_elements, 0, range.clone());
            }
        }

        // --- 5. Bloom Passes ---
        if self.postprocess_settings.enabled && self.postprocess_settings.bloom_enabled {
            let mut src_w = self.config.width as f32;
            let mut src_h = self.config.height as f32;

            for i in 0..bloom::BLOOM_MIP_COUNT {
                let down_uniform = bloom::BloomDownsampleUniform {
                    texel_size: [1.0 / src_w, 1.0 / src_h],
                    threshold: self.postprocess_settings.bloom_threshold,
                    knee: self.postprocess_settings.bloom_knee,
                    is_first_pass: if i == 0 { 1 } else { 0 },
                    _pad: [0.0; 3],
                };
                self.queue.write_buffer(&self.bloom_pyramid.down_buffers[i], 0, bytemuck::cast_slice(&[down_uniform]));

                let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: Some(&format!("Bloom Down Pass {}", i)),
                    color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                        view: &self.bloom_pyramid.mips[i].view,
                        resolve_target: None,
                        ops: wgpu::Operations { load: wgpu::LoadOp::Clear(wgpu::Color::BLACK), store: wgpu::StoreOp::Store },
                        depth_slice: None,
                    })],
                    depth_stencil_attachment: None,
                    ..Default::default()
                });

                pass.set_pipeline(&self.bloom_downsample_pipeline);
                pass.set_bind_group(0, &self.bloom_pyramid.down_bind_groups[i], &[]);
                pass.draw(0..3, 0..1);

                src_w = self.bloom_pyramid.mips[i].width as f32;
                src_h = self.bloom_pyramid.mips[i].height as f32;
            }

            for i in (0..(bloom::BLOOM_MIP_COUNT - 1)).rev() {
                let src_mip = &self.bloom_pyramid.mips[i + 1];
                let up_uniform = bloom::BloomUpsampleUniform {
                    texel_size: [1.0 / src_mip.width as f32, 1.0 / src_mip.height as f32],
                    filter_radius: self.postprocess_settings.bloom_scatter,
                    _pad: 0.0,
                };
                self.queue.write_buffer(&self.bloom_pyramid.up_buffers[i], 0, bytemuck::cast_slice(&[up_uniform]));

                let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: Some(&format!("Bloom Up Pass {}", i)),
                    color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                        view: &self.bloom_pyramid.mips[i].view,
                        resolve_target: None,
                        ops: wgpu::Operations { load: wgpu::LoadOp::Load, store: wgpu::StoreOp::Store },
                        depth_slice: None,
                    })],
                    depth_stencil_attachment: None,
                    ..Default::default()
                });

                pass.set_pipeline(&self.bloom_upsample_pipeline);
                pass.set_bind_group(0, &self.bloom_pyramid.up_bind_groups[i], &[]);
                pass.draw(0..3, 0..1);
            }
        }

        // --- 6. Post-Process Composite Pass ---
        {
            self.total_time += 0.016;
            let uniform_data = self.postprocess_settings.to_uniform(self.total_time);
            self.queue.write_buffer(
                &self.postprocess_uniform_buffer,
                0,
                bytemuck::cast_slice(&[uniform_data]),
            );

            let mut pp_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Post-Process Composite Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations { load: wgpu::LoadOp::Clear(wgpu::Color::BLACK), store: wgpu::StoreOp::Store },
                    depth_slice: None,
                })],
                depth_stencil_attachment: None,
                occlusion_query_set: None,
                timestamp_writes: None,
                multiview_mask: None,
            });

            pp_pass.set_pipeline(&self.postprocess_pipeline);
            pp_pass.set_bind_group(0, &self.postprocess_bind_group, &[]);
            pp_pass.draw(0..3, 0..1);
        }

        // --- 7. Egui Pass ---
        {
            let egui_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Egui Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations { load: wgpu::LoadOp::Load, store: wgpu::StoreOp::Store },
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
                egui_screen_descriptor,
            );
        }

        self.queue.submit(std::iter::once(encoder.finish()));
        self.queue.present(output);

        Ok(())
    }
}