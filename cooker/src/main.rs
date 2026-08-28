use anyhow::{Context, Result};
use shared::{BakedAlphaMode, BakedMaterial, BakedMesh, BakedModel, BakedNode, BakedVertex};
use std::collections::HashMap;
use std::env;
use std::fs::File;
use std::io::Write;

struct MeshBuffer<'a> {
    positions: &'a [[f32; 3]],
    normals: &'a [[f32; 3]],
    tex_coords: &'a [[f32; 2]],
    indices: &'a [u32],
    tangents: Vec<[f32; 4]>,
}

impl<'a> mikktspace::Geometry for MeshBuffer<'a> {
    fn num_faces(&self) -> usize {
        self.indices.len() / 3
    }

    fn num_vertices_of_face(&self, _face: usize) -> usize {
        3
    }

    fn position(&self, face: usize, vert: usize) -> [f32; 3] {
        let idx = self.indices[face * 3 + vert] as usize;
        self.positions[idx]
    }

    fn normal(&self, face: usize, vert: usize) -> [f32; 3] {
        let idx = self.indices[face * 3 + vert] as usize;
        self.normals[idx]
    }

    fn tex_coord(&self, face: usize, vert: usize) -> [f32; 2] {
        let idx = self.indices[face * 3 + vert] as usize;
        self.tex_coords[idx]
    }

    fn set_tangent_encoded(&mut self, tangent: [f32; 4], face: usize, vert: usize) {
        let idx = self.indices[face * 3 + vert] as usize;
        self.tangents[idx][0] += tangent[0];
        self.tangents[idx][1] += tangent[1];
        self.tangents[idx][2] += tangent[2];
        self.tangents[idx][3] = tangent[3];
    }
}

fn extract_or_save_texture(
    texture: Option<gltf::Texture>,
    default_name: &str,
    out_dir: &std::path::Path,
    images: &[gltf::image::Data],
) -> String {
    let Some(tex) = texture else {
        let file_name = format!("default_{}.png", default_name);
        let save_path = out_dir.join(&file_name);

        if !save_path.exists() {
            let color: [u8; 4] = match default_name {
                "normal" => [128, 128, 255, 255],
                "orm" => [255, 255, 0, 255],      // AO=1, Roughness=1, Metal=0
                _ => [255, 255, 255, 255],
            };
            
            println!("Generating fallback texture: {}", file_name);
            image::save_buffer(
                &save_path,
                &color,
                1, 1,
                image::ColorType::Rgba8
            ).unwrap_or_else(|e| panic!("Failed to save fallback texture {}: {}", file_name, e));
        }
        return file_name;
    };

    let source = tex.source();
    let mat_name = source.name().unwrap_or("texture");

    match source.source() {
        gltf::image::Source::Uri { uri, .. } => uri.to_string(),
        gltf::image::Source::View { .. } => {
            let img_data = &images[source.index()];
            
            let ext = match img_data.format {
                gltf::image::Format::R8G8B8A8 => "png",
                gltf::image::Format::R8G8B8 => "jpg",
                _ => "png",
            };
            
            let file_name = format!("{}_{}.{}", mat_name, default_name, ext);
            let save_path = out_dir.join(&file_name);

            if !save_path.exists() {
                let color_type = match img_data.format {
                    gltf::image::Format::R8 => image::ColorType::L8,
                    gltf::image::Format::R8G8 => image::ColorType::La8,
                    gltf::image::Format::R8G8B8 => image::ColorType::Rgb8,
                    gltf::image::Format::R8G8B8A8 => image::ColorType::Rgba8,
                    gltf::image::Format::R16 => image::ColorType::L16,
                    gltf::image::Format::R16G16 => image::ColorType::La16,
                    gltf::image::Format::R16G16B16 => image::ColorType::Rgb16,
                    gltf::image::Format::R16G16B16A16 => image::ColorType::Rgba16,
                    other => panic!("Unsupported image format in GLB: {:?}", other),
                };

                println!("Extracting texture: {}", file_name);
                image::save_buffer(
                    &save_path,
                    &img_data.pixels,
                    img_data.width,
                    img_data.height,
                    color_type,
                ).unwrap_or_else(|e| panic!("Failed to save texture {}: {}", file_name, e));
            }
            file_name
        }
    }
}

fn main() -> Result<()> {
    let args: Vec<String> = env::args().collect();
    if args.len() < 3 {
        eprintln!("Using: cargo run -p cooker <in.glb> <out.bin>");
        return Ok(());
    }
    let input_path = &args[1];
    let output_path = &args[2];

    println!("Baking file (PBR): {}", input_path);

    let (document, buffers, images) = gltf::import(input_path)
        .with_context(|| format!("failed to load glTF: {}", input_path))?;

    let out_dir = std::path::Path::new(output_path)
        .parent()
        .unwrap_or(std::path::Path::new(""));

    let mut baked_materials = Vec::new();
    for material in document.materials() {
        let name = material.name().unwrap_or("DefaultMaterial").to_string();
        let pbr = material.pbr_metallic_roughness();

        let base_color_factor = pbr.base_color_factor();
        let metallic_factor = pbr.metallic_factor();
        let roughness_factor = pbr.roughness_factor();

        let alpha_cutoff = material.alpha_cutoff().unwrap_or(0.5);
        let alpha_mode = match material.alpha_mode() {
            gltf::material::AlphaMode::Opaque => BakedAlphaMode::Opaque,
            gltf::material::AlphaMode::Mask => BakedAlphaMode::Mask,
            gltf::material::AlphaMode::Blend => BakedAlphaMode::Blend,
        };

        let diffuse_texture = extract_or_save_texture(
            pbr.base_color_texture().map(|t| t.texture()),
            "albedo",
            out_dir,
            &images,
        );

        let normal_texture = extract_or_save_texture(
            material.normal_texture().map(|t| t.texture()),
            "normal",
            out_dir,
            &images,
        );

        let orm_texture = extract_or_save_texture(
            pbr.metallic_roughness_texture().map(|t| t.texture()),
            "orm",
            out_dir,
            &images,
        );

        baked_materials.push(BakedMaterial {
            name,
            diffuse_texture,
            normal_texture,
            orm_texture,
            base_color_factor,
            metallic_factor,
            roughness_factor,
            alpha_cutoff,
            alpha_mode,
        });
    }

    let mut default_material_idx = None;
    if baked_materials.is_empty() {
        let diffuse_texture = extract_or_save_texture(None, "albedo", out_dir, &images);
        let normal_texture = extract_or_save_texture(None, "normal", out_dir, &images);
        let orm_texture = extract_or_save_texture(None, "orm", out_dir, &images);

        baked_materials.push(BakedMaterial {
            name: "DefaultFallbackMaterial".to_string(),
            diffuse_texture,
            normal_texture,
            orm_texture,
            base_color_factor: [1.0, 1.0, 1.0, 1.0],
            metallic_factor: 0.0,
            roughness_factor: 1.0,
            alpha_cutoff: 0.5,
            alpha_mode: BakedAlphaMode::Opaque,
        });
        default_material_idx = Some(0);
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
            let indices: Vec<u32> = reader.read_indices()
                .map(|i| i.into_u32().collect())
                .unwrap_or_else(|| (0..positions.len() as u32).collect());

            let mut mesh_buffer = MeshBuffer {
                positions: &positions,
                normals: &normals,
                tex_coords: &tex_coords,
                indices: &indices,
                tangents: vec![[1.0, 0.0, 0.0, 1.0]; positions.len()],
            };

            if reader.read_tangents().is_none() {
                mikktspace::generate_tangents(&mut mesh_buffer);

                for t in &mut mesh_buffer.tangents {
                    let len = (t[0] * t[0] + t[1] * t[1] + t[2] * t[2]).sqrt();
                    if len > 0.0001 {
                        t[0] /= len; t[1] /= len; t[2] /= len;
                    }
                }
            } else if let Some(tangents_iter) = reader.read_tangents() {
                for (i, t) in tangents_iter.enumerate() {
                    mesh_buffer.tangents[i] = t;
                }
            }

            let mut vertices = Vec::with_capacity(positions.len());
            for i in 0..positions.len() {
                vertices.push(BakedVertex {
                    position: positions[i],
                    normal: normals[i],
                    tex_coords: tex_coords[i],
                    tangent: mesh_buffer.tangents[i],
                });
            }

            let material_id = match primitive.material().index() {
                Some(idx) => idx,
                None => {
                    if let Some(def_idx) = default_material_idx {
                        def_idx
                    } else {
                        let diffuse_texture = extract_or_save_texture(None, "albedo", out_dir, &images);
                        let normal_texture = extract_or_save_texture(None, "normal", out_dir, &images);
                        let orm_texture = extract_or_save_texture(None, "orm", out_dir, &images);

                        let new_idx = baked_materials.len();
                        baked_materials.push(BakedMaterial {
                            name: "DefaultFallbackMaterial".to_string(),
                            diffuse_texture,
                            normal_texture,
                            orm_texture,
                            base_color_factor: [1.0, 1.0, 1.0, 1.0],
                            metallic_factor: 0.0,
                            roughness_factor: 1.0,
                            alpha_cutoff: 0.5,
                            alpha_mode: BakedAlphaMode::Opaque,
                        });
                        default_material_idx = Some(new_idx);
                        new_idx
                    }
                }
            };

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

    let mut baked_lights = Vec::new();
    let mut gltf_light_to_baked = HashMap::new();

    let mut baked_nodes = Vec::new();
    for node in document.nodes() {
        let (t, r, s) = node.transform().decomposed();
        let meshes = node.mesh()
            .and_then(|m| gltf_mesh_to_baked.get(&m.index()))
            .cloned()
            .unwrap_or_default();
        
        let mut light_idx = None;
        if let Some(light) = node.light() {
            let idx = light.index();
            
            if !gltf_light_to_baked.contains_key(&idx) {
                let (light_type, inner, outer) = match light.kind() {
                    gltf::khr_lights_punctual::Kind::Directional => 
                        (shared::BakedLightType::Directional, 0.0, 0.0),
                    gltf::khr_lights_punctual::Kind::Point => 
                        (shared::BakedLightType::Point, 0.0, 0.0),
                    gltf::khr_lights_punctual::Kind::Spot { inner_cone_angle, outer_cone_angle } => 
                        (shared::BakedLightType::Spot, inner_cone_angle, outer_cone_angle),
                };

                let baked_light = shared::BakedLight {
                    light_type,
                    color: light.color(),
                    intensity: (light.intensity() / 100.0).clamp(0.0, 10.0),
                    range: light.range().unwrap_or(100.0),
                    inner_angle: inner,
                    outer_angle: outer,
                };
                
                let b_idx = baked_lights.len();
                baked_lights.push(baked_light);
                gltf_light_to_baked.insert(idx, b_idx);
            }
            light_idx = Some(*gltf_light_to_baked.get(&idx).unwrap());
        }
        
        baked_nodes.push(BakedNode {
            name: node.name().unwrap_or("Node").to_string(),
            meshes,
            light: light_idx,
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
        lights: baked_lights,
        nodes: baked_nodes,
        roots,
    };

    let bytes = rkyv::to_bytes::<rkyv::rancor::Error>(&baked_model)
        .context("rkyv: failed serialization")?;

    let mut file = File::create(output_path)?;
    file.write_all(&bytes)?;

    println!("Success PBR baking: {} (Meshes: {}, Materials: {}, Lights: {})", 
        output_path, 
        baked_model.meshes.len(),
        baked_model.materials.len(),
        baked_model.lights.len()
    );

    Ok(())
}