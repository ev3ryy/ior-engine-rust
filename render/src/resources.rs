use std::future::Future;
use std::path::PathBuf;
use std::sync::Arc;

use shared::BakedModel;
use wgpu::util::DeviceExt;

use crate::{model, texture};

pub struct DefaultTextures {
    pub white_albedo: Arc<texture::Texture>,
    pub flat_normal: Arc<texture::Texture>,
    pub default_orm: Arc<texture::Texture>,
    pub sampler: Arc<wgpu::Sampler>,
}

impl DefaultTextures {
    pub fn new(device: &wgpu::Device, queue: &wgpu::Queue) -> Self {
        Self {
            white_albedo: Arc::new(texture::Texture::create_solid_color(
                device, queue, [255, 255, 255, 255], true, "Default White Albedo",
            )),
            flat_normal: Arc::new(texture::Texture::create_solid_color(
                device, queue, [128, 128, 255, 255], false, "Default Flat Normal",
            )),
            default_orm: Arc::new(texture::Texture::create_solid_color(
                device, queue, [255, 255, 0, 255], false, "Default ORM (Rough=1)",
            )),
            sampler: Arc::new(texture::Texture::create_default_sampler(device)),
        }
    }
}

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

pub async fn load_string(file_name: &str) -> anyhow::Result<String> {
    let path = resolve_path(file_name);
    let txt = tokio::fs::read_to_string(&path).await?;
    Ok(txt)
}

pub async fn load_binary(file_name: &str) -> anyhow::Result<Vec<u8>> {
    let path = resolve_path(file_name);
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
    default_textures: &DefaultTextures,
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
        let albedo_texture = match &m.diffuse_texture {
            Some(path) => load_texture_callback(path, true).await?,
            None => default_textures.white_albedo.clone(),
        };

        let normal_texture = match &m.normal_texture {
            Some(path) => load_texture_callback(path, false).await?,
            None => default_textures.flat_normal.clone(),
        };

        let orm_texture = match &m.orm_texture {
            Some(path) => load_texture_callback(path, false).await?,
            None => default_textures.default_orm.clone(),
        };

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
                wgpu::BindGroupEntry { binding: 4, resource: wgpu::BindingResource::Sampler(&default_textures.sampler) },
            ],
            label: Some(&format!("{} Bind Group", m.name)),
        });

        materials.push(model::Material {
            name: m.name.clone(),
            albedo_texture,
            normal_texture,
            metallic_roughness_texture: orm_texture,
            uniform_buffer,
            bind_group,
            has_alpha_test: m.alpha_mode == shared::BakedAlphaMode::Mask,
        });
    }

    if materials.is_empty() {
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
                wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::TextureView(&default_textures.white_albedo.view) },
                wgpu::BindGroupEntry { binding: 2, resource: wgpu::BindingResource::TextureView(&default_textures.flat_normal.view) },
                wgpu::BindGroupEntry { binding: 3, resource: wgpu::BindingResource::TextureView(&default_textures.default_orm.view) },
                wgpu::BindGroupEntry { binding: 4, resource: wgpu::BindingResource::Sampler(&default_textures.sampler) },
            ],
            label: Some("Default Fallback Bind Group"),
        });

        materials.push(model::Material {
            name: "DefaultFallbackMaterial".to_string(),
            albedo_texture: default_textures.white_albedo.clone(),
            normal_texture: default_textures.flat_normal.clone(),
            metallic_roughness_texture: default_textures.default_orm.clone(),
            uniform_buffer,
            bind_group,
            has_alpha_test: false,
        });
    }

    let mut meshes = Vec::new();
    for m in &baked_model.meshes {
        let vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some(&format!("{:?} Vertex Buffer", file_name)),
            contents: bytemuck::cast_slice(&m.vertices),
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
            bounding_center: m.bounding_center,
            bounding_radius: m.bounding_radius,
            aabb_min: m.aabb_min,
            aabb_max: m.aabb_max,
        });
    }

    Ok(model::Model { meshes, materials })
}