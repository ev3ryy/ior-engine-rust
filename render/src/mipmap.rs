pub struct MipmapGenerator {
    bind_group_layout: wgpu::BindGroupLayout,
    sampler: wgpu::Sampler,
    pipeline_srgb: wgpu::RenderPipeline,
    pipeline_linear: wgpu::RenderPipeline,
}

impl MipmapGenerator {
    pub fn new(device: &wgpu::Device) -> Self {
        let vs_module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("blit_vs.wgsl"),
            source: wgpu::ShaderSource::Wgsl(include_str!(concat!(env!("OUT_DIR"), "/blit_vs.wgsl")).into()),
        });

        let fs_module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("blit_fs.wgsl"),
            source: wgpu::ShaderSource::Wgsl(include_str!(concat!(env!("OUT_DIR"), "/blit_fs.wgsl")).into()),
        });

        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Mipmap Bind Group Layout"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Mipmap Pipeline Layout"),
            bind_group_layouts: &[Some(&bind_group_layout)],
            immediate_size: 0,
        });

        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("Mipmap Linear Sampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::MipmapFilterMode::Nearest,
            ..Default::default()
        });

        let create_pipeline = |format: wgpu::TextureFormat, label: &str| {
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some(label),
                layout: Some(&pipeline_layout),
                vertex: wgpu::VertexState {
                    module: &vs_module,
                    entry_point: Some("vs_main"),
                    buffers: &[],
                    compilation_options: Default::default(),
                },
                fragment: Some(wgpu::FragmentState {
                    module: &fs_module,
                    entry_point: Some("fs_main"),
                    targets: &[Some(wgpu::ColorTargetState {
                        format,
                        blend: None,
                        write_mask: wgpu::ColorWrites::ALL,
                    })],
                    compilation_options: Default::default(),
                }),
                primitive: wgpu::PrimitiveState {
                    topology: wgpu::PrimitiveTopology::TriangleList,
                    ..Default::default()
                },
                depth_stencil: None,
                multisample: wgpu::MultisampleState::default(),
                multiview_mask: None,
                cache: None,
            })
        };

        let pipeline_srgb = create_pipeline(wgpu::TextureFormat::Rgba8UnormSrgb, "Mipmap Pipeline (sRGB)");
        let pipeline_linear = create_pipeline(wgpu::TextureFormat::Rgba8Unorm, "Mipmap Pipeline (Linear)");

        Self {
            bind_group_layout,
            sampler,
            pipeline_srgb,
            pipeline_linear,
        }
    }

    pub fn generate_mipmaps(
        &self,
        device: &wgpu::Device,
        encoder: &mut wgpu::CommandEncoder,
        texture: &wgpu::Texture,
        format: wgpu::TextureFormat,
        mip_count: u32,
    ) {
        if mip_count <= 1 {
            return;
        }

        let pipeline = match format {
            wgpu::TextureFormat::Rgba8UnormSrgb => &self.pipeline_srgb,
            wgpu::TextureFormat::Rgba8Unorm => &self.pipeline_linear,
            _ => panic!("Unsupported format for mipmap generation: {:?}", format),
        };

        let views: Vec<wgpu::TextureView> = (0..mip_count)
            .map(|mip| {
                texture.create_view(&wgpu::TextureViewDescriptor {
                    label: Some(&format!("Mip View {mip}")),
                    format: None,
                    dimension: Some(wgpu::TextureViewDimension::D2),
                    base_mip_level: mip,
                    mip_level_count: Some(1),
                    ..Default::default()
                })
            })
            .collect();

        for target_mip in 1..mip_count {
            let src_view = &views[(target_mip - 1) as usize];
            let dst_view = &views[target_mip as usize];

            let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("Mipmap Step BindGroup"),
                layout: &self.bind_group_layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: wgpu::BindingResource::TextureView(src_view),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::Sampler(&self.sampler),
                    },
                ],
            });

            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some(&format!("Mipmap Pass {target_mip}")),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: dst_view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                        store: wgpu::StoreOp::Store,
                    },
                    depth_slice: None,
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });

            pass.set_pipeline(pipeline);
            pass.set_bind_group(0, &bind_group, &[]);
            pass.draw(0..3, 0..1);
        }
    }
}