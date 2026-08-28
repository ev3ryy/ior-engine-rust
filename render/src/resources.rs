use std::path::{PathBuf};
use std::future::Future;
use std::sync::Arc;

use shared::BakedModel;
use wgpu::util::DeviceExt;

use crate::{model, texture};

fn resolve_path(file_name: &str) -> PathBuf {
    let cwd_res = PathBuf::from("res").join(file_name);
    if cwd_res.exists() {
        return cwd_res;
    }

    if let Ok(exe_path) = std::env::current_exe() {
        if let Some(exe_dir) = exe_path.parent() {
            let candidate = exe_dir.join("res").join(file_name);
            if candidate.exists() {
                return candidate;
            }
        }
    }

    let mut cur = std::env::current_dir().unwrap_or_default();
    loop {
        let candidate = cur.join("res").join(file_name);
        if candidate.exists() {
            return candidate;
        }
        if !cur.pop() {
            break;
        }
    }

    PathBuf::from("res").join(file_name)
}

fn generate_fallback_png(file_name: &str) -> Option<Vec<u8>> {
    let color: [u8; 4] = match file_name {
        "default_normal.png" | "normal" => [128, 128, 255, 255], // normal (flat Z)
        "default_orm.png" | "orm" => [255, 255, 0, 255],         // AO=1, Roughness=1, Metal=0
        "default_albedo.png" | "albedo" => [255, 255, 255, 255], // white albedo
        _ => return None,
    };

    let mut buffer = std::io::Cursor::new(Vec::new());
    let img = image::RgbaImage::from_pixel(1, 1, image::Rgba(color));
    if img.write_to(&mut buffer, image::ImageFormat::Png).is_ok() {
        Some(buffer.into_inner())
    } else {
        None
    }
}

pub async fn load_string(file_name: &str) -> anyhow::Result<String> {
    let path = resolve_path(file_name);
    let txt = tokio::fs::read_to_string(&path).await?;
    Ok(txt)
}

pub async fn load_binary(file_name: &str) -> anyhow::Result<Vec<u8>> {
    let path = resolve_path(file_name);

    if !path.exists() {
        if let Some(bytes) = generate_fallback_png(file_name) {
            return Ok(bytes);
        }
    }

    log::info!("Loading binary file from path: {:?}", path);

    let data = tokio::fs::read(&path)
        .await
        .map_err(|e| anyhow::anyhow!("failed to load file {:?}: {}", path, e))?;

    Ok(data)
}

pub async fn load_texture(
    file_name: &str,
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    is_srgb: bool,
) -> anyhow::Result<texture::Texture> {
    let data = load_binary(file_name).await?;
    texture::Texture::from_bytes(device, queue, &data, file_name, is_srgb)
}

pub async fn load_binary_model<F, Fut>(
    file_name: &str,
    device: &wgpu::Device,
    _queue: &wgpu::Queue,
    layout: &wgpu::BindGroupLayout,
    mut load_texture_callback: F,
) -> anyhow::Result<model::Model>
where
    F: FnMut(&str, bool) -> Fut,
    Fut: Future<Output = anyhow::Result<Arc<texture::Texture>>>,
{
    let data = load_binary(file_name).await?;
    let baked_model = rkyv::from_bytes::<BakedModel, rkyv::rancor::Error>(&data)
        .map_err(|e| anyhow::anyhow!("deserialization error rkyv: {}", e))?;

    let mut materials = Vec::new();
    for m in &baked_model.materials {
        let albedo_texture = load_texture_callback(&m.diffuse_texture, true).await?;
        let normal_texture = load_texture_callback(&m.normal_texture, false).await?;
        let orm_texture = load_texture_callback(&m.orm_texture, false).await?;

        let uniform = model::MaterialUniform {
            base_color_factor: m.base_color_factor,
            metallic_factor: m.metallic_factor,
            roughness_factor: m.roughness_factor,
            alpha_cutoff: m.alpha_cutoff,
            _padding: 0.0,
        };

        let uniform_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some(&format!("{} Material Uniform", m.name)),
            contents: bytemuck::cast_slice(&[uniform]),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            layout,
            entries: &[
                wgpu::BindGroupEntry { binding: 0, resource: uniform_buffer.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::TextureView(&albedo_texture.view) },
                wgpu::BindGroupEntry { binding: 2, resource: wgpu::BindingResource::TextureView(&normal_texture.view) },
                wgpu::BindGroupEntry { binding: 3, resource: wgpu::BindingResource::TextureView(&orm_texture.view) },
                wgpu::BindGroupEntry { binding: 4, resource: wgpu::BindingResource::Sampler(&albedo_texture.sampler) },
            ],
            label: Some(&format!("{} Bind Group", m.name)),
        });

        let has_alpha_test = m.alpha_mode == shared::BakedAlphaMode::Mask;

        materials.push(model::Material {
            name: m.name.clone(),
            albedo_texture,
            normal_texture,
            metallic_roughness_texture: orm_texture,
            uniform_buffer,
            bind_group,
            has_alpha_test,
        });
    }

    if materials.is_empty() {
        let albedo_texture = load_texture_callback("default_albedo.png", true).await?;
        let normal_texture = load_texture_callback("default_normal.png", false).await?;
        let orm_texture = load_texture_callback("default_orm.png", false).await?;

        let uniform = model::MaterialUniform {
            base_color_factor: [1.0, 1.0, 1.0, 1.0],
            metallic_factor: 0.0,
            roughness_factor: 1.0,
            alpha_cutoff: 0.5,
            _padding: 0.0,
        };

        let uniform_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Default Fallback Material Uniform"),
            contents: bytemuck::cast_slice(&[uniform]),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            layout,
            entries: &[
                wgpu::BindGroupEntry { binding: 0, resource: uniform_buffer.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::TextureView(&albedo_texture.view) },
                wgpu::BindGroupEntry { binding: 2, resource: wgpu::BindingResource::TextureView(&normal_texture.view) },
                wgpu::BindGroupEntry { binding: 3, resource: wgpu::BindingResource::TextureView(&orm_texture.view) },
                wgpu::BindGroupEntry { binding: 4, resource: wgpu::BindingResource::Sampler(&albedo_texture.sampler) },
            ],
            label: Some("Default Fallback Bind Group"),
        });

        materials.push(model::Material {
            name: "DefaultFallbackMaterial".to_string(),
            albedo_texture,
            normal_texture,
            metallic_roughness_texture: orm_texture,
            uniform_buffer,
            bind_group,
            has_alpha_test: false,
        });
    }

    let mut meshes = Vec::new();
    for m in &baked_model.meshes {
        let mut min = glam::Vec3::splat(f32::MAX);
        let mut max = glam::Vec3::splat(f32::MIN);

        for v in &m.vertices {
            let pos = glam::Vec3::from(v.position);
            min = min.min(pos);
            max = max.max(pos);
        }

        let center = (min + max) * 0.5;
        let mut max_radius_sq: f32 = 0.0;

        for v in &m.vertices {
            let pos = glam::Vec3::from(v.position);
            max_radius_sq = max_radius_sq.max(pos.distance_squared(center));
        }
        let radius = max_radius_sq.sqrt();

        let vertices: Vec<model::ModelVertex> = m.vertices.iter().map(|v| {
            model::ModelVertex {
                position: v.position,
                tex_coords: v.tex_coords,
                normal: v.normal,
                tangent: v.tangent,
            }
        }).collect();

        let vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some(&format!("{:?} Vertex Buffer", file_name)),
            contents: bytemuck::cast_slice(&vertices),
            usage: wgpu::BufferUsages::VERTEX,
        });

        let index_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some(&format!("{:?} Index Buffer", file_name)),
            contents: bytemuck::cast_slice(&m.indices),
            usage: wgpu::BufferUsages::INDEX,
        });

        let valid_material_id = if m.material_id < materials.len() {
            m.material_id
        } else {
            0
        };

        meshes.push(model::Mesh {
            name: m.name.clone(),
            vertex_buffer,
            index_buffer,
            num_elements: m.indices.len() as u32,
            material: valid_material_id,
            bounding_center: center.into(),
            bounding_radius: radius,
        });
    }

    Ok(model::Model { meshes, materials })
}