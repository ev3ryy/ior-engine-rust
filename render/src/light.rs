#[repr(C)]
#[derive(Debug, Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
pub struct LightUniform {
    pub position: [f32; 3],
    pub light_type: u32,     // 0 = Directional, 1 = Point, 2 = Spot

    pub color: [f32; 3],
    pub intensity: f32,

    pub direction: [f32; 3],
    pub range: f32,          // radius

    pub inner_cone_cos: f32, // for spot (inner cos)
    pub outer_cone_cos: f32, // for Spot (outer cos)
    
    pub shadow_index: i32,
    pub shadow_bias: f32, 
}

pub const MAX_LIGHTS: usize = 10000;