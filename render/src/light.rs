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
    pub outer_cone_cos: f32, // Для Spot (outer cos)
    pub padding: [f32; 2],   // 64 byte
}

// limit for uniform buffer
pub const MAX_LIGHTS: usize = 16; 

#[repr(C)]
#[derive(Debug, Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
pub struct LightsBuffer {
    pub count: u32,
    pub _padding: [u32; 3], // 16 byte
    pub lights: [LightUniform; MAX_LIGHTS],
}