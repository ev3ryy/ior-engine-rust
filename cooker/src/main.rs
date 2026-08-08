use anyhow::{Context, Result};
use shared::{BakedMaterial, BakedMesh, BakedModel, BakedVertex};
use std::env;
use std::fs::File;
use std::io::Write;

fn main() -> Result<()> {
    let args: Vec<String> = env::args().collect();
    if args.len() < 3 {
        eprintln!("Using: cargo run -p cooker <in.glb> <out.bin>");
        return Ok(());
    }
    let input_path = &args[1];
    let output_path = &args[2];

    println!("Baking file: {}", input_path);

    let (document, buffers, _images) = gltf::import(input_path)
        .with_context(|| format!("failed to load glTF: {}", input_path))?;

    let mut baked_materials = Vec::new();
    for material in document.materials() {
        let name = material.name().unwrap_or("DefaultMaterial").to_string();
        
        let mut diffuse_texture = "default.png".to_string();
        if let Some(pbr) = material.pbr_metallic_roughness().base_color_texture() {
            match pbr.texture().source().source() {
                gltf::image::Source::Uri { uri, .. } => {
                    diffuse_texture = uri.to_string();
                }
                gltf::image::Source::View { .. } => {
                    diffuse_texture = format!("{}_albedo.png", name);
                }
            }
        }

        baked_materials.push(BakedMaterial {
            name,
            diffuse_texture,
        });
    }

    if baked_materials.is_empty() {
        baked_materials.push(BakedMaterial {
            name: "DefaultMaterial".to_string(),
            diffuse_texture: "default.png".to_string(),
        });
    }

    let mut baked_meshes = Vec::new();
    for mesh in document.meshes() {
        let mesh_name = mesh.name().unwrap_or("UnnamedMesh").to_string();
        
        for primitive in mesh.primitives() {
            let reader = primitive.reader(|buffer| Some(&buffers[buffer.index()]));
            
            let mut vertices = Vec::new();
            
            let positions = reader.read_positions()
                .expect("Mesh doesn`t have vertex positions!")
                .collect::<Vec<_>>();
            
            let normals = if let Some(iter) = reader.read_normals() {
                iter.collect::<Vec<_>>()
            } else {
                vec![[0.0, 1.0, 0.0]; positions.len()]
            };

            let tex_coords = if let Some(iter) = reader.read_tex_coords(0).map(|v| v.into_f32()) {
                iter.collect::<Vec<_>>()
            } else {
                vec![[0.0, 0.0]; positions.len()]
            };

            for i in 0..positions.len() {
                vertices.push(BakedVertex {
                    position: positions[i],
                    normal: normals[i],
                    tex_coords: tex_coords[i],
                });
            }

            let indices = if let Some(iter) = reader.read_indices() {
                iter.into_u32().collect::<Vec<_>>()
            } else {
                (0..vertices.len() as u32).collect::<Vec<_>>()
            };

            let material_id = primitive.material().index().unwrap_or(0);

            baked_meshes.push(BakedMesh {
                name: mesh_name.clone(),
                vertices,
                indices,
                material_id,
            });
        }
    }

    let baked_model = BakedModel {
        meshes: baked_meshes,
        materials: baked_materials,
    };

    let bytes = rkyv::to_bytes::<rkyv::rancor::Error>(&baked_model)
        .context("rkyv: failed serialization")?;

    let mut file = File::create(output_path)?;
    file.write_all(&bytes)?;

    println!("Success baking model: {} (Vertex: {}, Materials: {})", 
        output_path, 
        baked_model.meshes.iter().map(|m| m.vertices.len()).sum::<usize>(),
        baked_model.materials.len()
    );

    Ok(())
}