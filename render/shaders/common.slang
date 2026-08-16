module common;

public const static float PI = 3.14159265359;

public struct CameraUniform {
    public column_major float4x4 view_proj;
    public float4 view_position;
}

public struct Light {
    public float3 position;
    public uint light_type;
    public float3 color;
    public float intensity;
    public float3 direction;
    public float range;
    public float inner_cone_cos;
    public float outer_cone_cos;
    public float2 _padding;
}

public struct LightsBuffer {
    public uint count;
    public uint _padding0;
    public uint _padding1;
    public uint _padding2;
    public Light lights[16];
}

public struct MaterialUniform {
    public float4 base_color_factor;
    public float metallic_factor;
    public float roughness_factor;
    public float2 padding;
}