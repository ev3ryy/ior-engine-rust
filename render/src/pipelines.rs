use crate::{model, texture, InstanceRaw, Renderer};

pub struct RenderPipelines {
    pub render_pipeline_layout: wgpu::PipelineLayout,
    pub vs_module: wgpu::ShaderModule,
    pub fs_module: wgpu::ShaderModule,
    pub depth_fs_module: wgpu::ShaderModule,

    pub depth_pipeline: wgpu::RenderPipeline,
    pub render_pipeline: wgpu::RenderPipeline,
}

impl RenderPipelines {
    pub fn new(
        device: &wgpu::Device,
        material_layout: &wgpu::BindGroupLayout,
        camera_layout: &wgpu::BindGroupLayout,
        shadow_layout: &wgpu::BindGroupLayout,
        samples: u32,
    ) -> Self {
        let render_pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Render Pipeline Layout"),
            bind_group_layouts: &[
                Some(material_layout),
                Some(camera_layout),
                Some(shadow_layout),
            ],
            immediate_size: 0,
        });

        let vs_module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("main_vs.wgsl"),
            source: wgpu::ShaderSource::Wgsl(include_str!(concat!(env!("OUT_DIR"), "/main_vs.wgsl")).into()),
        });

        let fs_module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("main_fs.wgsl"),
            source: wgpu::ShaderSource::Wgsl(include_str!(concat!(env!("OUT_DIR"), "/main_fs.wgsl")).into()),
        });

        let depth_fs_module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("depth_fs.wgsl"),
            source: wgpu::ShaderSource::Wgsl(include_str!(concat!(env!("OUT_DIR"), "/depth_fs.wgsl")).into()),
        });

        let depth_pipeline = Self::build_depth_pipeline(device, &render_pipeline_layout, &vs_module, &depth_fs_module, samples);
        let render_pipeline = Self::build_render_pipeline(device, &render_pipeline_layout, &vs_module, &fs_module, samples);

        Self {
            render_pipeline_layout,
            vs_module,
            fs_module,
            depth_fs_module,
            depth_pipeline,
            render_pipeline,
        }
    }

    pub fn rebuild_for_msaa(&mut self, device: &wgpu::Device, samples: u32) {
        self.depth_pipeline = Self::build_depth_pipeline(
            device,
            &self.render_pipeline_layout,
            &self.vs_module,
            &self.depth_fs_module,
            samples,
        );

        self.render_pipeline = Self::build_render_pipeline(
            device,
            &self.render_pipeline_layout,
            &self.vs_module,
            &self.fs_module,
            samples,
        );
    }

    pub fn build_depth_pipeline(
        device: &wgpu::Device,
        layout: &wgpu::PipelineLayout,
        vs_module: &wgpu::ShaderModule,
        depth_fs_module: &wgpu::ShaderModule,
        samples: u32,
    ) -> wgpu::RenderPipeline {
        device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Depth Prepass Pipeline"),
            layout: Some(layout),
            vertex: wgpu::VertexState {
                module: vs_module,
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
                format: texture::Texture::DEPTH_FORMAT,
                depth_write_enabled: Some(true),
                depth_compare: Some(wgpu::CompareFunction::Less),
                stencil: wgpu::StencilState::default(),
                bias: wgpu::DepthBiasState::default(),
            }),
            multisample: wgpu::MultisampleState {
                count: samples,
                mask: !0,
                alpha_to_coverage_enabled: false,
            },
            multiview_mask: None,
            cache: None,
        })
    }

    pub fn build_render_pipeline(
        device: &wgpu::Device,
        layout: &wgpu::PipelineLayout,
        vs_module: &wgpu::ShaderModule,
        fs_module: &wgpu::ShaderModule,
        samples: u32,
    ) -> wgpu::RenderPipeline {
        device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Render Pipeline (PBR Forward)"),
            layout: Some(layout),
            vertex: wgpu::VertexState {
                module: vs_module,
                entry_point: Some("vs_main"),
                buffers: &[Some(model::ModelVertex::desc()), Some(InstanceRaw::desc())],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: fs_module,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format: Renderer::HDR_FORMAT,
                    blend: Some(wgpu::BlendState::REPLACE),
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
                count: samples,
                mask: !0,
                alpha_to_coverage_enabled: false,
            },
            multiview_mask: None,
            cache: None,
        })
    }
}