use std::io::{BufReader, Cursor};
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
) -> anyhow::Result<texture::Texture> {
    let data = load_binary(file_name).await?;
    texture::Texture::from_bytes(device, queue, &data, file_name)
}

#[allow(deprecated)]
pub async fn load_model<F, Fut>(
    file_name: &str,
    device: &wgpu::Device,
    _queue: &wgpu::Queue,
    layout: &wgpu::BindGroupLayout,
    mut load_texture_callback: F, 
) -> anyhow::Result<model::Model>
where
    F: FnMut(&str) -> Fut,
    Fut: Future<Output = anyhow::Result<Arc<texture::Texture>>>,
{
    let obj_text = load_string(file_name).await?;
    let obj_cursor = Cursor::new(obj_text);
    let mut obj_reader = BufReader::new(obj_cursor);

    let (models, obj_materials) = tobj::load_obj_buf_async(
        &mut obj_reader,
        &tobj::LoadOptions {
            triangulate: true,
            single_index: true,
            ..Default::default()
        },
        |p| async move {
            let mat_text = load_string(&p).await.unwrap();
            tobj::load_mtl_buf(&mut BufReader::new(Cursor::new(mat_text)))
        },
    )
    .await?;

    let mut materials = Vec::new();
    for m in obj_materials? {
        let tex_name = m.diffuse_texture.as_deref().unwrap_or("default.png");
        
        let diffuse_texture = load_texture_callback(tex_name).await?;

        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&diffuse_texture.view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&diffuse_texture.sampler),
                },
            ],
            label: Some(&format!("{} Bind Group", m.name)),
        });

        materials.push(model::Material {
            name: m.name,
            diffuse_texture,
            bind_group,
        });
    }

    let meshes = models
        .into_iter()
        .map(|m| {
            let mut vertices = Vec::new();
            for i in 0..m.mesh.positions.len() / 3 {
                vertices.push(model::ModelVertex {
                    position: [
                        m.mesh.positions[i * 3],
                        m.mesh.positions[i * 3 + 1],
                        m.mesh.positions[i * 3 + 2],
                    ],
                    tex_coords: if m.mesh.texcoords.is_empty() {
                        [0.0, 0.0]
                    } else {
                        [m.mesh.texcoords[i * 2], m.mesh.texcoords[i * 2 + 1]]
                    },
                    normal: if m.mesh.normals.is_empty() {
                        [0.0, 1.0, 0.0]
                    } else {
                        [
                            m.mesh.normals[i * 3],
                            m.mesh.normals[i * 3 + 1],
                            m.mesh.normals[i * 3 + 2],
                        ]
                    },
                });
            }

            let vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some(&format!("{:?} Vertex Buffer", file_name)),
                contents: bytemuck::cast_slice(&vertices),
                usage: wgpu::BufferUsages::VERTEX,
            });
            let index_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some(&format!("{:?} Index Buffer", file_name)),
                contents: bytemuck::cast_slice(&m.mesh.indices),
                usage: wgpu::BufferUsages::INDEX,
            });

            log::info!("Mesh: {}", m.name);
            model::Mesh {
                name: file_name.to_string(),
                vertex_buffer,
                index_buffer,
                num_elements: m.mesh.indices.len() as u32,
                material: m.mesh.material_id.unwrap_or(0),
            }
        })
        .collect::<Vec<_>>();

    Ok(model::Model { meshes, materials })
}

pub async fn load_binary_model<F, Fut>(
    file_name: &str,
    device: &wgpu::Device,
    _queue: &wgpu::Queue,
    layout: &wgpu::BindGroupLayout,
    mut load_texture_callback: F,
) -> anyhow::Result<model::Model>
where
    F: FnMut(&str) -> Fut,
    Fut: Future<Output = anyhow::Result<Arc<texture::Texture>>>,
{
    let data = load_binary(file_name).await?;
    
    let baked_model = rkyv::from_bytes::<BakedModel, rkyv::rancor::Error>(&data)
        .map_err(|e| anyhow::anyhow!("Ошибка десериализации rkyv: {}", e))?;

    let mut materials = Vec::new();
    for m in &baked_model.materials {
        let tex_name = if m.diffuse_texture.is_empty() {
            "default.png"
        } else {
            &m.diffuse_texture
        };

        let diffuse_texture = load_texture_callback(tex_name).await?;

        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&diffuse_texture.view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&diffuse_texture.sampler),
                },
            ],
            label: Some(&format!("{} Bind Group", m.name)),
        });

        materials.push(model::Material {
            name: m.name.clone(),
            diffuse_texture,
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