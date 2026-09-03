use bytemuck::{Pod, Zeroable};

#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum TonemappingMode {
    None = 0,
    ACES = 1,
    Reinhard = 2,
}

#[repr(C)]
#[derive(Debug, Copy, Clone, Pod, Zeroable)]
pub struct PostProcessUniform {
    pub exposure: f32,
    pub contrast: f32,
    pub saturation: f32,
    pub bloom_intensity: f32,

    pub vignette_intensity: f32,
    pub vignette_radius: f32,
    pub vignette_smoothness: f32,
    pub grain_intensity: f32,

    pub grain_time: f32,
    pub tonemapping_mode: u32,
    pub flags: u32,
    pub _pad0: f32,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct PostProcessSettings {
    pub enabled: bool,
    pub exposure: f32,
    pub contrast: f32,
    pub saturation: f32,
    pub tonemapping_mode: TonemappingMode,

    // Bloom
    pub bloom_enabled: bool,
    pub bloom_intensity: f32,
    pub bloom_threshold: f32,
    pub bloom_knee: f32,
    pub bloom_scatter: f32,

    // Vignette
    pub vignette_enabled: bool,
    pub vignette_intensity: f32,
    pub vignette_radius: f32,
    pub vignette_smoothness: f32,

    // Film Grain
    pub grain_enabled: bool,
    pub grain_intensity: f32,

    // Manual Gamma
    pub manual_gamma_correction: bool,
}

impl Default for PostProcessSettings {
    fn default() -> Self {
        Self {
            enabled: true,
            exposure: 1.0,
            contrast: 1.0,
            saturation: 1.0,
            tonemapping_mode: TonemappingMode::ACES,

            bloom_enabled: true,
            bloom_intensity: 0.08,
            bloom_threshold: 1.0,
            bloom_knee: 0.5,
            bloom_scatter: 1.0,

            vignette_enabled: false,
            vignette_intensity: 0.35,
            vignette_radius: 0.75,
            vignette_smoothness: 0.45,

            grain_enabled: true,
            grain_intensity: 0.04,

            manual_gamma_correction: false,
        }
    }
}

impl PostProcessSettings {
    pub fn to_uniform(&self, time: f32) -> PostProcessUniform {
        if !self.enabled {
            return PostProcessUniform {
                exposure: 1.0,
                contrast: 1.0,
                saturation: 1.0,
                bloom_intensity: 0.0,
                vignette_intensity: 0.0,
                vignette_radius: 1.0,
                vignette_smoothness: 0.0,
                grain_intensity: 0.0,
                grain_time: 0.0,
                tonemapping_mode: TonemappingMode::None as u32,
                flags: 0,
                _pad0: 0.0,
            };
        }

        let mut flags = 0u32;
        if self.vignette_enabled { flags |= 1 << 0; }
        if self.grain_enabled { flags |= 1 << 1; }
        if self.manual_gamma_correction { flags |= 1 << 2; }

        let bloom_intensity = if self.bloom_enabled { self.bloom_intensity } else { 0.0 };

        PostProcessUniform {
            exposure: self.exposure.max(0.001),
            contrast: self.contrast.max(0.0),
            saturation: self.saturation.max(0.0),
            bloom_intensity,
            vignette_intensity: self.vignette_intensity,
            vignette_radius: self.vignette_radius,
            vignette_smoothness: self.vignette_smoothness.max(0.001),
            grain_intensity: self.grain_intensity,
            grain_time: time,
            tonemapping_mode: self.tonemapping_mode as u32,
            flags,
            _pad0: 0.0,
        }
    }
}