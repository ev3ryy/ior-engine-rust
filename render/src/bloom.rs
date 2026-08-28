use bytemuck::{Pod, Zeroable};

pub const BLOOM_MIP_COUNT: usize = 5;

#[repr(C)]
#[derive(Debug, Copy, Clone, Pod, Zeroable)]
pub struct BloomDownsampleUniform {
    pub texel_size: [f32; 2],
    pub threshold: f32,
    pub knee: f32,
    pub is_first_pass: u32,
    pub _pad: [f32; 3],
}

#[repr(C)]
#[derive(Debug, Copy, Clone, Pod, Zeroable)]
pub struct BloomUpsampleUniform {
    pub texel_size: [f32; 2],
    pub filter_radius: f32,
    pub _pad: f32,
}

pub struct BloomMip {
    pub texture: wgpu::Texture,
    pub view: wgpu::TextureView,
    pub width: u32,
    pub height: u32,
}

pub struct BloomPyramid {
    pub mips: [BloomMip; BLOOM_MIP_COUNT],
    pub down_buffers: [wgpu::Buffer; BLOOM_MIP_COUNT],
    pub down_bind_groups: [wgpu::BindGroup; BLOOM_MIP_COUNT],
    pub up_buffers: [wgpu::Buffer; BLOOM_MIP_COUNT - 1],
    pub up_bind_groups: [wgpu::BindGroup; BLOOM_MIP_COUNT - 1],
}

impl BloomPyramid {
    pub fn new(
        device: &wgpu::Device,
        layout: &wgpu::BindGroupLayout,
        sampler: &wgpu::Sampler,
        hdr_view: &wgpu::TextureView,
        width: u32,
        height: u32,
    ) -> Self {
        let mut mips_vec = Vec::with_capacity(BLOOM_MIP_COUNT);
        let mut mip_w = (width / 2).max(1);
        let mut mip_h = (height / 2).max(1);

        for i in 0..BLOOM_MIP_COUNT {
            let texture = device.create_texture(&wgpu::TextureDescriptor {
                label: Some(&format!("Bloom Mip Texture {}", i)),
                size: wgpu::Extent3d {
                    width: mip_w,
                    height: mip_h,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::Rgba16Float,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
                view_formats: &[],
            });

            let view = texture.create_view(&wgpu::TextureViewDescriptor {
                label: Some(&format!("Bloom Mip View {}", i)),
                ..Default::default()
            });

            mips_vec.push(BloomMip { texture, view, width: mip_w, height: mip_h });

            mip_w = (mip_w / 2).max(1);
            mip_h = (mip_h / 2).max(1);
        }

        let mips: [BloomMip; BLOOM_MIP_COUNT] = mips_vec.try_into().ok().unwrap();

        // Downsample Buffers & BindGroups
        let mut down_buffers = Vec::new();
        let mut down_bind_groups = Vec::new();

        for i in 0..BLOOM_MIP_COUNT {
            let buffer = device.create_buffer(&wgpu::BufferDescriptor {
                label: Some(&format!("Bloom Down Buffer {}", i)),
                size: std::mem::size_of::<BloomDownsampleUniform>() as u64,
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });

            let src_view = if i == 0 { hdr_view } else { &mips[i - 1].view };

            let bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some(&format!("Bloom Down BG {}", i)),
                layout,
                entries: &[
                    wgpu::BindGroupEntry { binding: 0, resource: buffer.as_entire_binding() },
                    wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::TextureView(src_view) },
                    wgpu::BindGroupEntry { binding: 2, resource: wgpu::BindingResource::Sampler(sampler) },
                ],
            });

            down_buffers.push(buffer);
            down_bind_groups.push(bg);
        }

        // Upsample Buffers & BindGroups (от mips[i+1] в mips[i])
        let mut up_buffers = Vec::new();
        let mut up_bind_groups = Vec::new();

        for i in 0..(BLOOM_MIP_COUNT - 1) {
            let buffer = device.create_buffer(&wgpu::BufferDescriptor {
                label: Some(&format!("Bloom Up Buffer {}", i)),
                size: std::mem::size_of::<BloomUpsampleUniform>() as u64,
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });

            let src_view = &mips[i + 1].view;

            let bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some(&format!("Bloom Up BG {}", i)),
                layout,
                entries: &[
                    wgpu::BindGroupEntry { binding: 0, resource: buffer.as_entire_binding() },
                    wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::TextureView(src_view) },
                    wgpu::BindGroupEntry { binding: 2, resource: wgpu::BindingResource::Sampler(sampler) },
                ],
            });

            up_buffers.push(buffer);
            up_bind_groups.push(bg);
        }

        Self {
            mips,
            down_buffers: down_buffers.try_into().ok().unwrap(),
            down_bind_groups: down_bind_groups.try_into().ok().unwrap(),
            up_buffers: up_buffers.try_into().ok().unwrap(),
            up_bind_groups: up_bind_groups.try_into().ok().unwrap(),
        }
    }
}