struct CameraUniform {
    float4x4 view_proj;
    float4x4 view_matrix;
    float4x4 inv_proj_matrix;
    float4 view_position;
};

[[vk::binding(0, 0)]] 
ConstantBuffer<CameraUniform> camera;

struct OutlineUniform {
    float4x4 model_matrix;
    float4   outline_color;
    float    outline_width;
    float    _pad0;
    float    _pad1;
    float    _pad2;
};

[[vk::binding(0, 1)]]
ConstantBuffer<OutlineUniform> outline_data;

struct VertexInput {
    [[vk::location(0)]] float3 position : POSITION;
    [[vk::location(2)]] float3 normal   : NORMAL;
};

struct VertexOutput {
    float4 position : SV_Position;
    [[vk::location(0)]] float4 color;
};

[shader("vertex")]
VertexOutput vs_main(VertexInput input) {
    VertexOutput output;

    float3 world_pos = mul(outline_data.model_matrix, float4(input.position, 1.0)).xyz;
    float3 world_norm = normalize(mul((float3x3)outline_data.model_matrix, input.normal));

    float dist = length(camera.view_position.xyz - world_pos);
    float offset = outline_data.outline_width * max(dist * 0.08, 0.4);
    world_pos += world_norm * offset;

    float4 clip_pos = mul(camera.view_proj, float4(world_pos, 1.0));

    clip_pos.z -= 0.0004 * clip_pos.w;

    output.position = clip_pos;
    output.color = outline_data.outline_color;
    return output;
}

[shader("fragment")]
float4 fs_main(VertexOutput input) : SV_Target {
    return input.color;
}