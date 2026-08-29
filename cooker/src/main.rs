use anyhow::{Context, Result};
use glam::Vec3;
use image::{ImageBuffer, RgbaImage};
use rayon::prelude::*;
use sha2::{Digest, Sha256};
use shared::{
    BakedAlphaMode, BakedLight, BakedLightType, BakedMaterial, BakedMesh, BakedModel, BakedNode,
    BakedVertex,
};
use std::collections::{HashMap, HashSet};
use std::env;
use std::fs::{self, File};
use std::io::{BufWriter, Write};
use std::path::{Path};

// --- Mikktspace Tangent Generator ---
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
        self.tangents[idx] = tangent;
    }
}

// --- Fast Parallel Texture Processing ---
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum TextureSemantic {
    Albedo,
    Normal,
    Orm,
    Emissive,
}

#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub enum TextureSource {
    Embedded(usize),
    Uri(String),
}

#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub struct TextureCookJob {
    pub source: TextureSource,
    pub semantic: TextureSemantic,
}

fn compute_sha256(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    let hash_result = hasher.finalize();
    hash_result.iter().map(|b| format!("{:02x}", b)).collect()
}

fn cook_single_texture(
    job: &TextureCookJob,
    textures_dir: &Path,
    images: &[gltf::image::Data],
    input_parent: &Path,
) -> Result<String> {
    let (rgba_img, hash_str) = match &job.source {
        TextureSource::Embedded(idx) => {
            let img_data = &images[*idx];
            let raw_pixels = &img_data.pixels;

            let img: RgbaImage = match img_data.format {
                gltf::image::Format::R8G8B8 => {
                    let mut rgba = Vec::with_capacity((img_data.width * img_data.height * 4) as usize);
                    for chunk in raw_pixels.chunks_exact(3) {
                        rgba.extend_from_slice(&[chunk[0], chunk[1], chunk[2], 255]);
                    }
                    ImageBuffer::from_raw(img_data.width, img_data.height, rgba)
                        .context("Failed to create RgbaImage from RGB")?
                }
                gltf::image::Format::R8G8B8A8 => {
                    ImageBuffer::from_raw(img_data.width, img_data.height, raw_pixels.clone())
                        .context("Failed to create RgbaImage from RGBA")?
                }
                _ => anyhow::bail!("Unsupported glTF image format: {:?}", img_data.format),
            };

            let hash = compute_sha256(raw_pixels);
            (img, hash)
        }
        TextureSource::Uri(uri) => {
            let file_path = input_parent.join(uri);
            let file_bytes = fs::read(&file_path)
                .with_context(|| format!("Failed to read texture file: {:?}", file_path))?;
            let dynamic = image::load_from_memory(&file_bytes)
                .with_context(|| format!("Failed to decode image: {:?}", file_path))?;

            let hash = compute_sha256(&file_bytes);
            (dynamic.to_rgba8(), hash)
        }
    };

    let semantic_name = match job.semantic {
        TextureSemantic::Albedo => "albedo",
        TextureSemantic::Normal => "normal",
        TextureSemantic::Orm => "orm",
        TextureSemantic::Emissive => "emissive",
    };

    let dds_filename = format!("{}_{}.dds", &hash_str[..16], semantic_name);
    let dds_path = textures_dir.join(&dds_filename);
    let rel_return_path = format!("textures/{}", dds_filename);

    if dds_path.exists() {
        return Ok(rel_return_path);
    }

    let format = match job.semantic {
        TextureSemantic::Albedo | TextureSemantic::Emissive => image_dds::ImageFormat::BC7RgbaUnormSrgb,
        TextureSemantic::Normal => image_dds::ImageFormat::BC5RgUnorm,
        TextureSemantic::Orm => image_dds::ImageFormat::BC7RgbaUnorm,
    };

    let dds = image_dds::dds_from_image(
        &rgba_img,
        format,
        image_dds::Quality::Fast,
        image_dds::Mipmaps::GeneratedAutomatic,
    ).with_context(|| format!("Failed to compress texture to DDS: {:?}", dds_filename))?;

    let out_file = File::create(&dds_path)?;
    let mut writer = BufWriter::new(out_file);
    dds.write(&mut writer)?;

    println!("  [Parallel Cooked] -> {:?}", dds_filename);

    Ok(rel_return_path)
}

// --- Main Pipeline ---
fn main() -> Result<()> {
    let args: Vec<String> = env::args().collect();
    if args.len() < 3 {
        eprintln!("Usage: cooker <input.glb/gltf> <output.bin>");
        return Ok(());
    }

    let input_path = Path::new(&args[1]);
    let output_path = Path::new(&args[2]);

    let out_dir = output_path.parent().unwrap_or(Path::new(""));
    let textures_dir = out_dir.join("textures");
    fs::create_dir_all(&textures_dir)?;

    let input_parent = input_path.parent().unwrap_or(Path::new(""));

    let start_time = std::time::Instant::now();
    println!("=== Baking Model: {:?} ===", input_path);

    let (document, buffers, images) = gltf::import(input_path)
        .with_context(|| format!("Failed to import glTF from {:?}", input_path))?;

    let mut unique_jobs = HashSet::new();
    let mut mat_job_map = Vec::new();

    for material in document.materials() {
        let pbr = material.pbr_metallic_roughness();

        let extract_job = |tex_opt: Option<gltf::Texture>, semantic: TextureSemantic| -> Option<TextureCookJob> {
            let tex = tex_opt?;
            let source = match tex.source().source() {
                gltf::image::Source::View { .. } => TextureSource::Embedded(tex.source().index()),
                gltf::image::Source::Uri { uri, .. } => TextureSource::Uri(uri.to_string()),
            };
            Some(TextureCookJob { source, semantic })
        };

        let albedo_job = extract_job(pbr.base_color_texture().map(|t| t.texture()), TextureSemantic::Albedo);
        let normal_job = extract_job(material.normal_texture().map(|t| t.texture()), TextureSemantic::Normal);
        let orm_job = extract_job(pbr.metallic_roughness_texture().map(|t| t.texture()), TextureSemantic::Orm);
        let emissive_job = extract_job(material.emissive_texture().map(|t| t.texture()), TextureSemantic::Emissive);

        if let Some(j) = &albedo_job { unique_jobs.insert(j.clone()); }
        if let Some(j) = &normal_job { unique_jobs.insert(j.clone()); }
        if let Some(j) = &orm_job { unique_jobs.insert(j.clone()); }
        if let Some(j) = &emissive_job { unique_jobs.insert(j.clone()); }

        mat_job_map.push((albedo_job, normal_job, orm_job, emissive_job));
    }

    let jobs_vec: Vec<TextureCookJob> = unique_jobs.into_iter().collect();
    let cooked_results: Result<HashMap<TextureCookJob, String>> = jobs_vec
        .into_par_iter()
        .map(|job| {
            let path = cook_single_texture(&job, &textures_dir, &images, input_parent)?;
            Ok((job, path))
        })
        .collect();

    let texture_lookup = cooked_results?;

    let mut baked_materials = Vec::new();
    for (i, material) in document.materials().enumerate() {
        let name = material.name().unwrap_or("Material").to_string();
        let pbr = material.pbr_metallic_roughness();

        let (albedo_job, normal_job, orm_job, emissive_job) = &mat_job_map[i];

        let diffuse_texture = albedo_job.as_ref().and_then(|j| texture_lookup.get(j).cloned());
        let normal_texture = normal_job.as_ref().and_then(|j| texture_lookup.get(j).cloned());
        let orm_texture = orm_job.as_ref().and_then(|j| texture_lookup.get(j).cloned());
        let emissive_texture = emissive_job.as_ref().and_then(|j| texture_lookup.get(j).cloned());

        let alpha_mode = match material.alpha_mode() {
            gltf::material::AlphaMode::Opaque => BakedAlphaMode::Opaque,
            gltf::material::AlphaMode::Mask => BakedAlphaMode::Mask,
            gltf::material::AlphaMode::Blend => BakedAlphaMode::Blend,
        };

        baked_materials.push(BakedMaterial {
            name,
            diffuse_texture,
            normal_texture,
            orm_texture,
            emissive_texture,
            base_color_factor: pbr.base_color_factor(),
            emissive_factor: material.emissive_factor(),
            metallic_factor: pbr.metallic_factor(),
            roughness_factor: pbr.roughness_factor(),
            alpha_cutoff: material.alpha_cutoff().unwrap_or(0.5),
            alpha_mode,
        });
    }

    if baked_materials.is_empty() {
        baked_materials.push(BakedMaterial {
            name: "DefaultMaterial".to_string(),
            diffuse_texture: None,
            normal_texture: None,
            orm_texture: None,
            emissive_texture: None,
            base_color_factor: [1.0, 1.0, 1.0, 1.0],
            emissive_factor: [0.0, 0.0, 0.0],
            metallic_factor: 0.0,
            roughness_factor: 1.0,
            alpha_cutoff: 0.5,
            alpha_mode: BakedAlphaMode::Opaque,
        });
    }

    let mut baked_meshes = Vec::new();
    let mut gltf_mesh_to_baked = HashMap::new();

    for mesh in document.meshes() {
        let mesh_name = mesh.name().unwrap_or("UnnamedMesh").to_string();
        let mut prim_indices = Vec::new();

        for primitive in mesh.primitives() {
            let reader = primitive.reader(|buffer| Some(&buffers[buffer.index()]));

            let positions = reader.read_positions().expect("Mesh has no positions!").collect::<Vec<_>>();
            let normals = reader.read_normals().map(|i| i.collect()).unwrap_or_else(|| vec![[0.0, 1.0, 0.0]; positions.len()]);
            let tex_coords = reader.read_tex_coords(0).map(|i| i.into_f32().collect()).unwrap_or_else(|| vec![[0.0, 0.0]; positions.len()]);
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
            } else if let Some(tangents_iter) = reader.read_tangents() {
                for (i, t) in tangents_iter.enumerate() {
                    mesh_buffer.tangents[i] = t;
                }
            }

            let mut aabb_min = Vec3::splat(f32::MAX);
            let mut aabb_max = Vec3::splat(f32::MIN);

            let mut vertices = Vec::with_capacity(positions.len());
            for i in 0..positions.len() {
                let pos = Vec3::from(positions[i]);
                aabb_min = aabb_min.min(pos);
                aabb_max = aabb_max.max(pos);

                vertices.push(BakedVertex {
                    position: positions[i],
                    tex_coords: tex_coords[i],
                    normal: normals[i],
                    tangent: mesh_buffer.tangents[i],
                });
            }

            let bounding_center = (aabb_min + aabb_max) * 0.5;
            let mut max_radius_sq: f32 = 0.0;
            for pos_arr in &positions {
                let pos = Vec3::from(*pos_arr);
                max_radius_sq = max_radius_sq.max(pos.distance_squared(bounding_center));
            }
            let bounding_radius = max_radius_sq.sqrt();

            let material_id = primitive.material().index().unwrap_or(0);
            let baked_idx = baked_meshes.len();

            baked_meshes.push(BakedMesh {
                name: mesh_name.clone(),
                vertices,
                indices,
                material_id,
                bounding_center: bounding_center.into(),
                bounding_radius,
                aabb_min: aabb_min.into(),
                aabb_max: aabb_max.into(),
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
                    gltf::khr_lights_punctual::Kind::Directional => {
                        (BakedLightType::Directional, 0.0, 0.0)
                    }
                    gltf::khr_lights_punctual::Kind::Point => (BakedLightType::Point, 0.0, 0.0),
                    gltf::khr_lights_punctual::Kind::Spot {
                        inner_cone_angle,
                        outer_cone_angle,
                    } => (BakedLightType::Spot, inner_cone_angle, outer_cone_angle),
                };

                let baked_light = BakedLight {
                    light_type,
                    color: light.color(),
                    intensity: light.intensity(),
                    range: light.range().unwrap_or(50.0),
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
        .context("Failed to serialize BakedModel via rkyv")?;

    let mut file = File::create(output_path)?;
    file.write_all(&bytes)?;

    let elapsed = start_time.elapsed();
    println!(
        "=== Bake Successful in {:.2?}s: {:?} (Meshes: {}, Materials: {}, Nodes: {}) ===\n",
        elapsed.as_secs_f32(),
        output_path,
        baked_model.meshes.len(),
        baked_model.materials.len(),
        baked_model.nodes.len()
    );

    Ok(())
}