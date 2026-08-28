module postprocess;

public struct PostProcessUniform {
    public float exposure;
    public float contrast;
    public float saturation;
    public float bloom_intensity;

    public float vignette_intensity;
    public float vignette_radius;
    public float vignette_smoothness;
    public float grain_intensity;

    public float grain_time;
    public uint tonemapping_mode; // 0: None, 1: ACES, 2: Reinhard
    public uint flags;            // Bit 0: Vignette, Bit 1: Grain, Bit 2: Gamma
    public float _pad0;
};

[[vk::binding(0, 0)]] ConstantBuffer<PostProcessUniform> postprocess_data;
[[vk::binding(1, 0)]] Texture2D<float4> t_hdr;
[[vk::binding(2, 0)]] SamplerState s_sampler;
[[vk::binding(3, 0)]] Texture2D<float4> t_bloom;

struct FullscreenVertexOutput {
    float4 clip_position : SV_Position;
    [[vk::location(0)]] float2 uv;
};

[shader("vertex")]
FullscreenVertexOutput vs_main(uint vertex_id : SV_VertexID) {
    FullscreenVertexOutput out;
    out.uv = float2((vertex_id << 1) & 2, vertex_id & 2);
    out.clip_position = float4(out.uv * float2(2.0, -2.0) + float2(-1.0, 1.0), 0.0, 1.0);
    return out;
}

float3 aces_fitted(float3 x) {
    float a = 2.51;
    float b = 0.03;
    float c = 2.43;
    float d = 0.59;
    float e = 0.14;
    return clamp((x * (a * x + b)) / (x * (c * x + d) + e), 0.0, 1.0);
}

float3 reinhard(float3 x) {
    return x / (x + float3(1.0));
}

[shader("fragment")]
float4 fs_main(FullscreenVertexOutput in) : SV_Target {
    float4 hdr_sample = t_hdr.Sample(s_sampler, in.uv);
    float3 color = hdr_sample.rgb;

    // Blending with bloom
    if (postprocess_data.bloom_intensity > 0.001) {
        float3 bloom = t_bloom.Sample(s_sampler, in.uv).rgb;
        color += bloom * postprocess_data.bloom_intensity;
    }

    // 1. Exposure
    color *= postprocess_data.exposure;

    // 2. Saturation
    float luma = dot(color, float3(0.2126, 0.7152, 0.0722));
    color = lerp(float3(luma), color, postprocess_data.saturation);

    // 3. Contrast
    color = max(float3(0.0), (color - float3(0.5)) * postprocess_data.contrast + float3(0.5));

    // 4. Vinnette bit 0
    if ((postprocess_data.flags & (1 << 0)) != 0) {
        float2 coord = in.uv - 0.5;
        float dist = length(coord);
        float v = smoothstep(postprocess_data.vignette_radius, postprocess_data.vignette_radius - postprocess_data.vignette_smoothness, dist);
        color *= lerp(1.0 - postprocess_data.vignette_intensity, 1.0, v);
    }

    // 5. Tone Mapping
    if (postprocess_data.tonemapping_mode == 1) {
        color = aces_fitted(color);
    } else if (postprocess_data.tonemapping_mode == 2) {
        color = reinhard(color);
    } else {
        color = clamp(color, 0.0, 1.0);
    }

    // 6. Film Grain bit 1
    if ((postprocess_data.flags & (1 << 1)) != 0) {
        float2 noise_uv = in.uv * 1000.0 + float2(postprocess_data.grain_time * 12.9898, postprocess_data.grain_time * 78.233);
        float noise = frac(sin(dot(noise_uv, float2(12.9898, 78.233))) * 43758.5453);
        color += (noise - 0.5) * postprocess_data.grain_intensity;
        color = clamp(color, 0.0, 1.0);
    }

    // 7. Gamma Correction bit 2
    if ((postprocess_data.flags & (1 << 2)) != 0) {
        color = pow(max(color, float3(0.0)), float3(1.0 / 2.2));
    }

    return float4(color, hdr_sample.a);
}