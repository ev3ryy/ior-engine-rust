use anyhow::*;
use image::GenericImageView;

pub struct Texture {
    #[allow(unused)]
    pub texture: wgpu::Texture,
    pub view: wgpu::TextureView,
    //pub sampler: wgpu::Sampler,
}

impl Texture {
    pub const DEPTH_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Depth32Float;

    pub fn create_default_sampler(device: &wgpu::Device) -> wgpu::Sampler {
        device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("Default Texture Sampler (Linear Repeat)"),
            address_mode_u: wgpu::AddressMode::Repeat,
            address_mode_v: wgpu::AddressMode::Repeat,
            address_mode_w: wgpu::AddressMode::Repeat,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::MipmapFilterMode::Linear,
            anisotropy_clamp: 1,
            ..Default::default()
        })
    }

    pub fn create_depth_texture(
        device: &wgpu::Device,
        config: &wgpu::SurfaceConfiguration,
        label: &str,
    ) -> Self {
        let size = wgpu::Extent3d {
            width: config.width.max(1),
            height: config.height.max(1),
            depth_or_array_layers: 1,
        };
        let desc = wgpu::TextureDescriptor {
            label: Some(label),
            size,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: Self::DEPTH_FORMAT,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[Self::DEPTH_FORMAT],
        };
        let texture = device.create_texture(&desc);
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());

        Self { texture, view }
    }

    pub fn create_solid_color(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        color: [u8; 4],
        is_srgb: bool,
        label: &str,
    ) -> Self {
        let size = wgpu::Extent3d {
            width: 1,
            height: 1,
            depth_or_array_layers: 1,
        };
        let format = if is_srgb {
            wgpu::TextureFormat::Rgba8UnormSrgb
        } else {
            wgpu::TextureFormat::Rgba8Unorm
        };

        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some(label),
            size,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });

        queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            &color,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(4),
                rows_per_image: Some(1),
            },
            size,
        );

        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());

        Self { texture, view }
    }

    pub fn from_dds_bytes(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        bytes: &[u8],
        label: &str,
    ) -> Result<Self> {
        use image_dds::ddsfile::{Dds, DxgiFormat};

        let dds = Dds::read(bytes).context("Failed to parse DDS header")?;
        let width = dds.get_width();
        let height = dds.get_height();
        let mip_count = dds.get_num_mipmap_levels().max(1);

        let (format, block_size) = match dds.get_dxgi_format() {
            Some(DxgiFormat::BC7_UNorm_sRGB) => (wgpu::TextureFormat::Bc7RgbaUnormSrgb, 16),
            Some(DxgiFormat::BC7_UNorm) => (wgpu::TextureFormat::Bc7RgbaUnorm, 16),
            Some(DxgiFormat::BC5_UNorm) => (wgpu::TextureFormat::Bc5RgUnorm, 16),
            Some(DxgiFormat::BC1_UNorm_sRGB) => (wgpu::TextureFormat::Bc1RgbaUnormSrgb, 8),
            Some(DxgiFormat::BC1_UNorm) => (wgpu::TextureFormat::Bc1RgbaUnorm, 8),
            Some(DxgiFormat::R8G8B8A8_UNorm_sRGB) => (wgpu::TextureFormat::Rgba8UnormSrgb, 4),
            Some(DxgiFormat::R8G8B8A8_UNorm) => (wgpu::TextureFormat::Rgba8Unorm, 4),
            other => anyhow::bail!("Unsupported DXGI format in DDS: {:?}", other),
        };

        let is_bc = format.is_compressed();

        let size = wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        };

        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some(label),
            size,
            mip_level_count: mip_count,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });

        let mut offset = 0;
        for mip in 0..mip_count {
            let mip_w = (width >> mip).max(1);
            let mip_h = (height >> mip).max(1);

            let (bytes_per_row, rows_per_image, mip_bytes, copy_w, copy_h) = if is_bc {
                let blocks_x = (mip_w + 3) / 4;
                let blocks_y = (mip_h + 3) / 4;
                let pitch = blocks_x * block_size;
                let total = pitch * blocks_y;
                (
                    Some(pitch),
                    Some(blocks_y),
                    total as usize,
                    blocks_x * 4,
                    blocks_y * 4,
                )
            } else {
                let pitch = mip_w * block_size;
                let total = pitch * mip_h;
                (
                    Some(pitch),
                    Some(mip_h),
                    total as usize,
                    mip_w,
                    mip_h,
                )
            };

            if offset + mip_bytes > dds.data.len() {
                log::warn!("DDS data truncated at mip level {} (expected {} bytes, available {})", mip, mip_bytes, dds.data.len() - offset);
                break;
            }

            let mip_slice = &dds.data[offset..offset + mip_bytes];
            offset += mip_bytes;

            queue.write_texture(
                wgpu::TexelCopyTextureInfo {
                    texture: &texture,
                    mip_level: mip,
                    origin: wgpu::Origin3d::ZERO,
                    aspect: wgpu::TextureAspect::All,
                },
                mip_slice,
                wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row,
                    rows_per_image,
                },
                wgpu::Extent3d {
                    width: copy_w,
                    height: copy_h,
                    depth_or_array_layers: 1,
                },
            );
        }

        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());

        Ok(Self { texture, view })
    }

    pub fn from_image(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        img: &image::DynamicImage,
        label: Option<&str>,
        is_srgb: bool,
    ) -> Result<Self> {
        let dimensions = img.dimensions();
        let rgba = img.to_rgba8();

        let size = wgpu::Extent3d {
            width: dimensions.0,
            height: dimensions.1,
            depth_or_array_layers: 1,
        };

        let format = if is_srgb {
            wgpu::TextureFormat::Rgba8UnormSrgb
        } else {
            wgpu::TextureFormat::Rgba8Unorm
        };

        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label,
            size,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });

        queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                aspect: wgpu::TextureAspect::All,
                texture: &texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
            },
            &rgba,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(4 * dimensions.0),
                rows_per_image: Some(dimensions.1),
            },
            size,
        );

        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());

        Ok(Self { texture, view })
    }

    pub fn from_bytes(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        bytes: &[u8],
        label: &str,
        is_srgb: bool,
    ) -> Result<Self> {
        if bytes.starts_with(b"DDS ") {
            Self::from_dds_bytes(device, queue, bytes, label)
        } else {
            let img = image::load_from_memory(bytes)?;
            Self::from_image(device, queue, &img, Some(label), is_srgb)
        }
    }
}