use rkyv::{Archive, Deserialize, Serialize};

#[derive(Archive, Deserialize, Serialize, Debug, PartialEq)]
pub struct BakedVertex {
    pub position: [f32; 3],
    pub tex_coords: [f32; 2],
    pub normal: [f32; 3],
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
    pub diffuse_texture: String, 
}

#[derive(Archive, Deserialize, Serialize, Debug, PartialEq)]
pub struct BakedNode {
    pub name: String,
    pub meshes: Vec<usize>, 
    pub position: [f32; 3],
    pub rotation: [f32; 4],
    pub scale: [f32; 3],
    pub children: Vec<usize>,
}

#[derive(Archive, Deserialize, Serialize, Debug, PartialEq)]
pub struct BakedModel {
    pub meshes: Vec<BakedMesh>,
    pub materials: Vec<BakedMaterial>,
    pub nodes: Vec<BakedNode>,
    pub roots: Vec<usize>,
}