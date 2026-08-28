import common;

// --- Material resources group 0 ---
[[vk::binding(0, 0)]] ConstantBuffer<MaterialUniform> material;
[[vk::binding(1, 0)]] Texture2D<float4> t_albedo;
[[vk::binding(4, 0)]] SamplerState s_sampler;

// --- Cascade resources group 1 ---
[[vk::binding(0, 1)]] ConstantBuffer<CascadeUniform> cascade;

struct VertexOutput {
    float4 clip_position : SV_Position;
    [[vk::location(0)]] float2 tex_coords;
};

[shader("vertex")]
VertexOutput vs_main(
    [[vk::location(0)]] float3 position,
    [[vk::location(1)]] float2 tex_coords,

    [[vk::location(5)]] float4 model_0,
    [[vk::location(6)]] float4 model_1,
    [[vk::location(7)]] float4 model_2,
    [[vk::location(8)]] float4 model_3
) {
    float4x4 model_matrix = transpose(float4x4(model_0, model_1, model_2, model_3));
    float4 world_pos = mul(model_matrix, float4(position, 1.0));

    VertexOutput out;
    out.clip_position = mul(cascade.light_space_matrix, world_pos);
    out.tex_coords = tex_coords;
    return out;
}

[shader("fragment")]
void depth_fs_main([[vk::location(0)]] float2 tex_coords) {
    float4 albedo_tex = t_albedo.Sample(s_sampler, tex_coords);
    float cutoff = material.alpha_cutoff > 0.001 ? material.alpha_cutoff : 0.5;
    if (albedo_tex.a * material.base_color_factor.a < cutoff) {
        discard;
    }
}