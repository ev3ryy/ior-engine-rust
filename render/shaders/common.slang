module common;

public const static float PI = 3.14159265359;
public const static uint MAX_LIGHTS_PER_CLUSTER = 256;

public struct CameraUniform {
    public column_major float4x4 view_proj;
    public column_major float4x4 view_matrix;
    public column_major float4x4 inv_proj_matrix;
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

public struct MaterialUniform {
    public float4 base_color_factor;
    public float metallic_factor;
    public float roughness_factor;
    public float2 padding;
}

public struct ClusterData {
    public uint3 grid_dim;     
    public float z_near;       
    
    public float2 tile_size;   
    public float2 screen_size; 
    
    public float z_far;        
    public uint total_lights;
    public uint dir_light_count;
    public uint _padding;
}

public struct ClusterLightGroup {
    public uint offset; // offset in global light index array
    public uint count;  // count lights in cluster
}