use rkyv::{Archive, Deserialize, Serialize};
use bytemuck::{Pod, Zeroable};

#[derive(Archive, Deserialize, Serialize, Debug, PartialEq, Eq, Clone, Copy)]
#[repr(u32)]
pub enum BakedLightType {
    Directional = 0,
    Point = 1,
    Spot = 2,
}

#[derive(Archive, Deserialize, Serialize, Debug, PartialEq, Eq, Clone, Copy)]
#[repr(u8)]
pub enum BakedAlphaMode {
    Opaque = 0,
    Mask = 1,
    Blend = 2,
}

#[derive(Archive, Deserialize, Serialize, Debug, PartialEq, Clone, Copy)]
pub struct BakedLight {
    pub light_type: BakedLightType,
    pub color: [f32; 3],
    pub intensity: f32,
    pub range: f32,
    pub inner_angle: f32,
    pub outer_angle: f32,
}

#[derive(Archive, Deserialize, Serialize, Debug, Clone, Copy, PartialEq, Pod, Zeroable)]
#[repr(C)]
pub struct BakedVertex {
    pub position: [f32; 3],   // Offset: 0,  Size: 12
    pub tex_coords: [f32; 2], // Offset: 12, Size: 8
    pub normal: [f32; 3],     // Offset: 20, Size: 12
    pub tangent: [f32; 4],    // Offset: 32, Size: 16 (XYZ = Tangent, W = Sign/Bitangent handedness)
}

#[derive(Archive, Deserialize, Serialize, Debug, Clone, PartialEq)]
pub struct BakedMesh {
    pub name: String,
    pub vertices: Vec<BakedVertex>,
    pub indices: Vec<u32>,
    pub material_id: usize,

    // --- Bounding Volumes ---
    pub bounding_center: [f32; 3],
    pub bounding_radius: f32,
    pub aabb_min: [f32; 3],
    pub aabb_max: [f32; 3],
}

#[derive(Archive, Deserialize, Serialize, Debug, Clone, PartialEq)]
pub struct BakedMaterial {
    pub name: String,
    pub diffuse_texture: Option<String>,       // Albedo (Base Color, sRGB)
    pub normal_texture: Option<String>,        // Normal Map (Linear, BC5/RG)
    pub orm_texture: Option<String>,           // Occlusion (R), Roughness (G), Metallic (B)
    pub emissive_texture: Option<String>,      // Emissive Map (sRGB)

    pub base_color_factor: [f32; 4],
    pub emissive_factor: [f32; 3],
    pub metallic_factor: f32,
    pub roughness_factor: f32,
    pub alpha_cutoff: f32,
    pub alpha_mode: BakedAlphaMode,
}

#[derive(Archive, Deserialize, Serialize, Debug, Clone, PartialEq)]
pub struct BakedNode {
    pub name: String,
    pub meshes: Vec<usize>, 
    pub light: Option<usize>,
    pub position: [f32; 3],
    pub rotation: [f32; 4],
    pub scale: [f32; 3],
    pub children: Vec<usize>,
}

#[derive(Archive, Deserialize, Serialize, Debug, Clone, PartialEq)]
pub struct BakedModel {
    pub meshes: Vec<BakedMesh>,
    pub materials: Vec<BakedMaterial>,
    pub lights: Vec<BakedLight>,
    pub nodes: Vec<BakedNode>,
    pub roots: Vec<usize>,
}