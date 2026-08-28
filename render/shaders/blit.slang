struct VertexOutput {
    float4 position : SV_Position;
    [[vk::location(0)]] float2 tex_coords;
};

[shader("vertex")]
VertexOutput vs_main(uint in_vertex_index : SV_VertexID) {
    VertexOutput out;
    float x = (in_vertex_index == 1) ? 3.0 : -1.0;
    float y = (in_vertex_index == 2) ? 3.0 : -1.0;

    out.position = float4(x, y, 0.0, 1.0);
    out.tex_coords = float2(x * 0.5 + 0.5, -y * 0.5 + 0.5);
    return out;
}

[[vk::binding(0, 0)]] Texture2D<float4> t_diffuse;
[[vk::binding(1, 0)]] SamplerState s_diffuse;

[shader("fragment")]
float4 fs_main([[vk::location(0)]] float2 tex_coords) : SV_Target {
    return t_diffuse.Sample(s_diffuse, tex_coords);
}