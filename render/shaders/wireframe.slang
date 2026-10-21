struct CameraUniform {
    float4x4 view_proj;
    float4x4 view_matrix;
    float4x4 inv_proj_matrix;
    float4 view_position;
};

[[vk::binding(0, 0)]] 
ConstantBuffer<CameraUniform> camera;

struct VertexInput {
    [[vk::location(0)]] float3 position : POSITION;
    [[vk::location(1)]] float4 color    : COLOR;
};

struct VertexOutput {
    float4 position : SV_Position;
    [[vk::location(0)]] float4 color;
};

[shader("vertex")]
VertexOutput vs_main(VertexInput input) {
    VertexOutput output;
    output.position = mul(camera.view_proj, float4(input.position, 1.0));
    output.color = input.color;
    return output;
}

[shader("fragment")]
float4 fs_main(VertexOutput input) : SV_Target {
    return input.color;
}