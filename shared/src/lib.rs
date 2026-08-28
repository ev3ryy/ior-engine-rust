use rkyv::{Archive, Deserialize, Serialize};

#[derive(Archive, Deserialize, Serialize, Debug, PartialEq, Clone, Copy)]
#[repr(u32)]
pub enum BakedLightType {
    Directional = 0,
    Point = 1,
    Spot = 2,
}

#[derive(Archive, Deserialize, Serialize, Debug, PartialEq, Clone, Copy)]
#[repr(u8)]
pub enum BakedAlphaMode {
    Opaque = 0,
    Mask = 1,
    Blend = 2,
}

#[derive(Archive, Deserialize, Serialize, Debug, PartialEq, Clone)]
pub struct BakedLight {
    pub light_type: BakedLightType,
    pub color: [f32; 3],
    pub intensity: f32,
    pub range: f32,
    pub inner_angle: f32,
    pub outer_angle: f32,
}

#[derive(Archive, Deserialize, Serialize, Debug, PartialEq)]
pub struct BakedVertex {
    pub position: [f32; 3],
    pub tex_coords: [f32; 2],
    pub normal: [f32; 3],
    pub tangent: [f32; 4],
}

#[derive(Archive, Deserialize, Serialize, Debug, PartialEq)]
pub struct BakedMesh {
    pub name: String,
    pub vertices: Vec<BakedVertex>,
    pub indices: Vec<u32>,
    pub material_id: usize,
}

#[derive(Archive, Deserialize, Serialize, Debug, PartialEq)]
pub struct BakedMaterial {
    pub name: String,
    pub diffuse_texture: String,       // Albedo (Base Color)
    pub normal_texture: String,        // Normal Map
    pub orm_texture: String,           // Occlusion (R), Roughness (G), Metallic (B)
    pub base_color_factor: [f32; 4],
    pub metallic_factor: f32,
    pub roughness_factor: f32,
    pub alpha_cutoff: f32,
    pub alpha_mode: BakedAlphaMode,
}

#[derive(Archive, Deserialize, Serialize, Debug, PartialEq)]
pub struct BakedNode {
    pub name: String,
    pub meshes: Vec<usize>, 
    pub light: Option<usize>,
    pub position: [f32; 3],
    pub rotation: [f32; 4],
    pub scale: [f32; 3],
    pub children: Vec<usize>,
}

#[derive(Archive, Deserialize, Serialize, Debug, PartialEq)]
pub struct BakedModel {
    pub meshes: Vec<BakedMesh>,
    pub materials: Vec<BakedMaterial>,
    pub lights: Vec<BakedLight>,
    pub nodes: Vec<BakedNode>,
    pub roots: Vec<usize>,
}