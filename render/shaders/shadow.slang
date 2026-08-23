import common;

public struct ShadowUniform {
    public column_major float4x4 light_space_matrices[4];
    public float4 cascade_splits;
}

[[vk::binding(5, 1)]] ConstantBuffer<ShadowUniform> shadow_data;

struct CascadeData {
    uint index;
}
[[vk::binding(0, 2)]] ConstantBuffer<CascadeData> cascade;

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
    
    float4x4 light_matrix = shadow_data.light_space_matrices[cascade.index];

    VertexOutput out;
    out.clip_position = mul(light_matrix, world_pos);
    out.tex_coords = tex_coords; 
    return out;
}