module bloom;

public struct BloomDownsampleUniform {
    public float2 texel_size;
    public float threshold;
    public float knee;
    public uint is_first_pass;
    public float _pad0;
    public float _pad1;
    public float _pad2;
};

public struct BloomUpsampleUniform {
    public float2 texel_size;
    public float filter_radius;
    public float _pad;
};

[[vk::binding(0, 0)]] ConstantBuffer<BloomDownsampleUniform> down_data;
[[vk::binding(1, 0)]] Texture2D<float4> t_down_src;
[[vk::binding(2, 0)]] SamplerState s_sampler;

[[vk::binding(0, 0)]] ConstantBuffer<BloomUpsampleUniform> up_data;
[[vk::binding(1, 0)]] Texture2D<float4> t_up_src;

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

float luma(float3 color) {
    return dot(color, float3(0.2126, 0.7152, 0.0722));
}

float3 prefilter_threshold(float3 color, float threshold, float knee) {
    float lum = luma(color);
    float curve = clamp(lum - threshold + knee, 0.0, 2.0 * knee);
    float soft = (curve * curve) / (4.0 * knee + 1e-4);
    float factor = max(lum - threshold, soft) / max(lum, 1e-4);
    return color * max(factor, 0.0);
}

[shader("fragment")]
float4 fs_downsample(FullscreenVertexOutput in) : SV_Target {
    float2 ts = down_data.texel_size;

    float3 a = t_down_src.Sample(s_sampler, in.uv + float2(-2.0, -2.0) * ts).rgb;
    float3 b = t_down_src.Sample(s_sampler, in.uv + float2( 0.0, -2.0) * ts).rgb;
    float3 c = t_down_src.Sample(s_sampler, in.uv + float2( 2.0, -2.0) * ts).rgb;

    float3 d = t_down_src.Sample(s_sampler, in.uv + float2(-2.0,  0.0) * ts).rgb;
    float3 e = t_down_src.Sample(s_sampler, in.uv).rgb;
    float3 f = t_down_src.Sample(s_sampler, in.uv + float2( 2.0,  0.0) * ts).rgb;

    float3 g = t_down_src.Sample(s_sampler, in.uv + float2(-2.0,  2.0) * ts).rgb;
    float3 h = t_down_src.Sample(s_sampler, in.uv + float2( 0.0,  2.0) * ts).rgb;
    float3 i = t_down_src.Sample(s_sampler, in.uv + float2( 2.0,  2.0) * ts).rgb;

    float3 j = t_down_src.Sample(s_sampler, in.uv + float2(-1.0, -1.0) * ts).rgb;
    float3 k = t_down_src.Sample(s_sampler, in.uv + float2( 1.0, -1.0) * ts).rgb;
    float3 l = t_down_src.Sample(s_sampler, in.uv + float2(-1.0,  1.0) * ts).rgb;
    float3 m = t_down_src.Sample(s_sampler, in.uv + float2( 1.0,  1.0) * ts).rgb;

    float3 color;
    if (down_data.is_first_pass == 1) {
        a = prefilter_threshold(a, down_data.threshold, down_data.knee);
        b = prefilter_threshold(b, down_data.threshold, down_data.knee);
        c = prefilter_threshold(c, down_data.threshold, down_data.knee);
        d = prefilter_threshold(d, down_data.threshold, down_data.knee);
        e = prefilter_threshold(e, down_data.threshold, down_data.knee);
        f = prefilter_threshold(f, down_data.threshold, down_data.knee);
        g = prefilter_threshold(g, down_data.threshold, down_data.knee);
        h = prefilter_threshold(h, down_data.threshold, down_data.knee);
        i = prefilter_threshold(i, down_data.threshold, down_data.knee);
        j = prefilter_threshold(j, down_data.threshold, down_data.knee);
        k = prefilter_threshold(k, down_data.threshold, down_data.knee);
        l = prefilter_threshold(l, down_data.threshold, down_data.knee);
        m = prefilter_threshold(m, down_data.threshold, down_data.knee);

        float3 box1 = (a + b + d + e) * 0.25;
        float3 box2 = (b + c + e + f) * 0.25;
        float3 box3 = (d + e + g + h) * 0.25;
        float3 box4 = (e + f + h + i) * 0.25;
        float3 box5 = (j + k + l + m) * 0.25;

        float w1 = 1.0 / (1.0 + luma(box1));
        float w2 = 1.0 / (1.0 + luma(box2));
        float w3 = 1.0 / (1.0 + luma(box3));
        float w4 = 1.0 / (1.0 + luma(box4));
        float w5 = 1.0 / (1.0 + luma(box5));

        color = (box1 * w1 + box2 * w2 + box3 * w3 + box4 * w4 + box5 * w5) / (w1 + w2 + w3 + w4 + w5 + 1e-4);
    } else {
        color = e * 0.125;
        color += (a + c + g + i) * 0.03125;
        color += (b + d + f + h) * 0.0625;
        color += (j + k + l + m) * 0.125;
    }

    return float4(color, 1.0);
}

[shader("fragment")]
float4 fs_upsample(FullscreenVertexOutput in) : SV_Target {
    float2 ts = up_data.texel_size * up_data.filter_radius;

    float3 a = t_up_src.Sample(s_sampler, in.uv + float2(-1.0, -1.0) * ts).rgb;
    float3 b = t_up_src.Sample(s_sampler, in.uv + float2( 0.0, -1.0) * ts).rgb;
    float3 c = t_up_src.Sample(s_sampler, in.uv + float2( 1.0, -1.0) * ts).rgb;

    float3 d = t_up_src.Sample(s_sampler, in.uv + float2(-1.0,  0.0) * ts).rgb;
    float3 e = t_up_src.Sample(s_sampler, in.uv).rgb;
    float3 f = t_up_src.Sample(s_sampler, in.uv + float2( 1.0,  0.0) * ts).rgb;

    float3 g = t_up_src.Sample(s_sampler, in.uv + float2(-1.0,  1.0) * ts).rgb;
    float3 h = t_up_src.Sample(s_sampler, in.uv + float2( 0.0,  1.0) * ts).rgb;
    float3 i = t_up_src.Sample(s_sampler, in.uv + float2( 1.0,  1.0) * ts).rgb;

    float3 color = e * 4.0;
    color += (b + d + f + h) * 2.0;
    color += (a + c + g + i) * 1.0;
    color /= 16.0;

    return float4(color, 1.0);
}