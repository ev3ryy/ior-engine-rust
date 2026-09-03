module common;

// --- constants ---
public const static float PI = 3.14159265359;
public const static uint MAX_LIGHTS_PER_CLUSTER = 256;

public const static float SHADOW_MAP_SIZE = 2048.0;
public const static float SHADOW_TEXEL_SIZE = 1.0 / SHADOW_MAP_SIZE;

public const static float SPOT_SHADOW_MAP_SIZE = 1024.0;
public const static float SPOT_SHADOW_TEXEL_SIZE = 1.0 / SPOT_SHADOW_MAP_SIZE;

public const static uint MAX_POINT_SHADOWS = 4;
public const static float POINT_SHADOW_MAP_SIZE = 256.0;
public const static float POINT_SHADOW_TEXEL_SIZE = 1.0 / POINT_SHADOW_MAP_SIZE;

// --- light types ---
public const static uint LIGHT_TYPE_DIR = 0;
public const static uint LIGHT_TYPE_POINT = 1;
public const static uint LIGHT_TYPE_SPOT = 2;

// --- camera & scene uniforms ---
public struct CameraUniform {
    public column_major float4x4 view_proj;
    public column_major float4x4 view_matrix;
    public column_major float4x4 inv_proj_matrix;
    public float4 view_position;
};

public struct Light {
    public float3 position;
    public uint light_type;
    public float3 color;
    public float intensity;
    public float3 direction;
    public float range;
    public float inner_cone_cos;
    public float outer_cone_cos;
    public int shadow_index;
    public float shadow_bias;
};

public struct MaterialUniform {
    public float4 base_color_factor;
    public float metallic_factor;
    public float roughness_factor;
    public float alpha_cutoff;
    public float _padding;
};

// --- clustered forward rendering ---
public struct ClusterData {
    public uint3 grid_dim;
    public float z_near;
    public float2 tile_size;
    public float2 screen_size;
    public float z_far;
    public uint total_lights;
    public uint dir_light_count;
    public uint _padding;
};

public struct ClusterLightGroup {
    public uint offset; 
    public uint count;
};

// --- shadows ---
public struct ShadowUniform {
    public column_major float4x4 light_space_matrices[4];
    public float4 cascade_splits;
    public float4 cascade_texel_sizes;
};

public struct SpotShadowUniform {
    public column_major float4x4 shadow_matrices[8];
};

public struct PointShadowUniform {
    public float4 light_params[4]; // x: near_plane, y: far_plane, z: bias, w: padding
};

public struct CascadeUniform {
    public column_major float4x4 light_space_matrix;
};