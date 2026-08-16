use std::path::PathBuf;
use std::future::Future;
use std::sync::Arc;

use shared::BakedModel;

use wgpu::util::DeviceExt;

use crate::{model, texture};

fn resource_path(file_name: &str) -> PathBuf {
    #[cfg(debug_assertions)]
    {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../res")
            .join(file_name)
    }

    #[cfg(not(debug_assertions))]
    {
        std::env::current_exe()
            .expect("failed get path to executable file")
            .parent()
            .expect("failed get root directory")
            .join("res")
            .join(file_name)
    }
}

pub async fn load_string(file_name: &str) -> anyhow::Result<String> {
    let path = resource_path(file_name);
    let txt = tokio::fs::read_to_string(&path).await?;
    Ok(txt)
}

pub async fn load_binary(file_name: &str) -> anyhow::Result<Vec<u8>> {
    let path = resource_path(file_name);

    log::info!("trying load file from path: {:?}", path);

    let data = std::fs::read(&path)
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
        .map_err(|e| anyhow::anyhow!("Ошибка десериализации rkyv: {}", e))?;

    let mut materials = Vec::new();
    for m in &baked_model.materials {
        // Загрузка текстур с правильным цветовым пространством (sRGB только для Albedo)
        let albedo_texture = load_texture_callback(&m.diffuse_texture, true).await?;
        let normal_texture = load_texture_callback(&m.normal_texture, false).await?;
        let orm_texture = load_texture_callback(&m.orm_texture, false).await?;

        let uniform = model::MaterialUniform {
            base_color_factor: m.base_color_factor,
            metallic_factor: m.metallic_factor,
            roughness_factor: m.roughness_factor,
            padding: [0.0, 0.0],
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

        materials.push(model::Material {
            name: m.name.clone(),
            albedo_texture,
            normal_texture,
            metallic_roughness_texture: orm_texture,
            uniform_buffer,
            bind_group,
        });
    }

    let mut meshes = Vec::new();
    for m in &baked_model.meshes {
        let vertices: Vec<model::ModelVertex> = m.vertices.iter().map(|v| {
            model::ModelVertex {
                position: v.position,
                tex_coords: v.tex_coords,
                normal: v.normal,
                tangent: v.tangent, // Передаем честные тангенсы из кукера
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

        meshes.push(model::Mesh {
            name: m.name.clone(),
            vertex_buffer,
            index_buffer,
            num_elements: m.indices.len() as u32,
            material: m.material_id,
        });
    }

    Ok(model::Model { meshes, materials })
}