use anyhow::{Context, Result};
use shared::{BakedMaterial, BakedMesh, BakedModel, BakedNode, BakedVertex};
use std::collections::HashMap;
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

    let (document, buffers, images) = gltf::import(input_path)
        .with_context(|| format!("failed to load glTF: {}", input_path))?;

    let out_dir = std::path::Path::new(output_path)
        .parent()
        .unwrap_or(std::path::Path::new(""));

    let mut baked_materials = Vec::new();
    for material in document.materials() {
        let name = material.name().unwrap_or("DefaultMaterial").to_string();
        
        let mut diffuse_texture = "default.png".to_string();
        if let Some(pbr) = material.pbr_metallic_roughness().base_color_texture() {
            let tex = pbr.texture();
            let source = tex.source();

            match source.source() {
                gltf::image::Source::Uri { uri, .. } => {
                    diffuse_texture = uri.to_string();
                }
                gltf::image::Source::View { .. } => {
                    let img_name = source.name().unwrap_or(&name);
                    diffuse_texture = format!("{}_albedo.png", img_name);
                    
                    let save_path = out_dir.join(&diffuse_texture);
                    
                    if !save_path.exists() {
                        let img_data = &images[source.index()];
                        
                        let color_type = match img_data.format {
                            gltf::image::Format::R8 => image::ColorType::L8,
                            gltf::image::Format::R8G8 => image::ColorType::La8,
                            gltf::image::Format::R8G8B8 => image::ColorType::Rgb8,
                            gltf::image::Format::R8G8B8A8 => image::ColorType::Rgba8,
                            _ => panic!("Unsupported image format in GLB"),
                        };

                        println!("Extracting embedded texture: {}", diffuse_texture);
                        
                        image::save_buffer(
                            &save_path,
                            &img_data.pixels,
                            img_data.width,
                            img_data.height,
                            color_type,
                        ).expect("Failed to save extracted texture to disk");
                    }
                }
            }
        }

        baked_materials.push(BakedMaterial {
            name,
            diffuse_texture,
        });
    }

    let mut baked_meshes = Vec::new();
    let mut gltf_mesh_to_baked = HashMap::new();

    for mesh in document.meshes() {
        let mesh_name = mesh.name().unwrap_or("UnnamedMesh").to_string();
        let mut prim_indices = Vec::new();
        
        for primitive in mesh.primitives() {
            let reader = primitive.reader(|buffer| Some(&buffers[buffer.index()]));
            
            let positions = reader.read_positions().expect("No positions!").collect::<Vec<_>>();
            let normals = reader.read_normals().map(|i| i.collect()).unwrap_or(vec![[0.0, 1.0, 0.0]; positions.len()]);
            let tex_coords = reader.read_tex_coords(0).map(|i| i.into_f32().collect()).unwrap_or(vec![[0.0, 0.0]; positions.len()]);

            let mut vertices = Vec::with_capacity(positions.len());
            for i in 0..positions.len() {
                vertices.push(BakedVertex {
                    position: positions[i],
                    normal: normals[i],
                    tex_coords: tex_coords[i],
                });
            }

            let indices = reader.read_indices().map(|i| i.into_u32().collect()).unwrap_or_else(|| (0..vertices.len() as u32).collect());
            let material_id = primitive.material().index().unwrap_or(0);

            let baked_idx = baked_meshes.len();
            baked_meshes.push(BakedMesh {
                name: mesh_name.clone(),
                vertices,
                indices,
                material_id,
            });
            prim_indices.push(baked_idx);
        }
        gltf_mesh_to_baked.insert(mesh.index(), prim_indices);
    }

    let mut baked_nodes = Vec::new();
    for node in document.nodes() {
        let (t, r, s) = node.transform().decomposed();
        
        let meshes = if let Some(mesh) = node.mesh() {
            gltf_mesh_to_baked.get(&mesh.index()).cloned().unwrap_or_default()
        } else {
            Vec::new()
        };
        
        baked_nodes.push(BakedNode {
            name: node.name().unwrap_or("Node").to_string(),
            meshes,
            position: t,
            rotation: r,
            scale: s,
            children: node.children().map(|c| c.index()).collect(),
        });
    }

    let mut roots = Vec::new();
    if let Some(scene) = document.default_scene() {
        for node in scene.nodes() {
            roots.push(node.index());
        }
    }

    let baked_model = BakedModel {
        meshes: baked_meshes,
        materials: baked_materials,
        nodes: baked_nodes,
        roots,
    };

    let bytes = rkyv::to_bytes::<rkyv::rancor::Error>(&baked_model)
        .context("rkyv: failed serialization")?;

    let mut file = File::create(output_path)?;
    file.write_all(&bytes)?;

    println!("Success baking model: {} (Nodes: {}, Meshes: {})", 
        output_path, 
        baked_model.nodes.len(),
        baked_model.meshes.len()
    );

    Ok(())
}