module cull;
import common;

[[vk::binding(0, 0)]] ConstantBuffer<CameraUniform> camera;
[[vk::binding(1, 0)]] ConstantBuffer<ClusterData> cluster_data;
[[vk::binding(2, 0)]] StructuredBuffer<Light> lights;

[[vk::binding(3, 0)]] RWStructuredBuffer<ClusterLightGroup> cluster_grid;
[[vk::binding(4, 0)]] RWStructuredBuffer<uint> light_indices;

bool test_sphere_aabb(float3 sphere_center, float radius, float3 aabb_min, float3 aabb_max) {
    float sq_dist = 0.0;
    for (int i = 0; i < 3; i++) {
        float v = sphere_center[i];
        if (v < aabb_min[i]) sq_dist += (aabb_min[i] - v) * (aabb_min[i] - v);
        if (v > aabb_max[i]) sq_dist += (v - aabb_max[i]) * (v - aabb_max[i]);
    }
    return sq_dist <= (radius * radius);
}

[shader("compute")]
[numthreads(8, 8, 1)]
void cull_lights(uint3 global_id : SV_DispatchThreadID) {
    if (global_id.x >= cluster_data.grid_dim.x || 
        global_id.y >= cluster_data.grid_dim.y || 
        global_id.z >= cluster_data.grid_dim.z) return;

    float2 min_px = float2(global_id.xy) * cluster_data.tile_size;
    float2 max_px = float2(global_id.xy + 1) * cluster_data.tile_size;

    float2 ndc_min = (min_px / cluster_data.screen_size) * 2.0 - 1.0;
    float2 ndc_max = (max_px / cluster_data.screen_size) * 2.0 - 1.0;
    ndc_min.y = -ndc_min.y;
    ndc_max.y = -ndc_max.y;

    float4 corner0 = mul(camera.inv_proj_matrix, float4(ndc_min.x, ndc_min.y, 1.0, 1.0));
    float4 corner1 = mul(camera.inv_proj_matrix, float4(ndc_max.x, ndc_min.y, 1.0, 1.0));
    float4 corner2 = mul(camera.inv_proj_matrix, float4(ndc_min.x, ndc_max.y, 1.0, 1.0));
    float4 corner3 = mul(camera.inv_proj_matrix, float4(ndc_max.x, ndc_max.y, 1.0, 1.0));

    float3 ray0 = corner0.xyz / corner0.w;
    float3 ray1 = corner1.xyz / corner1.w;
    float3 ray2 = corner2.xyz / corner2.w;
    float3 ray3 = corner3.xyz / corner3.w;

    float z_near = cluster_data.z_near;
    float z_far = cluster_data.z_far;
    float num_slices = float(cluster_data.grid_dim.z);
    
    float tile_near_z = -z_near * pow(z_far / z_near, float(global_id.z) / num_slices);
    float tile_far_z  = -z_near * pow(z_far / z_near, float(global_id.z + 1) / num_slices);

    float3 p0 = ray0 * (tile_near_z / ray0.z);
    float3 p1 = ray1 * (tile_near_z / ray1.z);
    float3 p2 = ray2 * (tile_near_z / ray2.z);
    float3 p3 = ray3 * (tile_near_z / ray3.z);
    float3 p4 = ray0 * (tile_far_z / ray0.z);
    float3 p5 = ray1 * (tile_far_z / ray1.z);
    float3 p6 = ray2 * (tile_far_z / ray2.z);
    float3 p7 = ray3 * (tile_far_z / ray3.z);

    float3 aabb_min = min(min(min(p0, p1), min(p2, p3)), min(min(p4, p5), min(p6, p7)));
    float3 aabb_max = max(max(max(p0, p1), max(p2, p3)), max(max(p4, p5), max(p6, p7)));

    uint visible_lights[MAX_LIGHTS_PER_CLUSTER];
    uint light_count = 0;

    for (uint i = cluster_data.dir_light_count; i < cluster_data.total_lights && light_count < MAX_LIGHTS_PER_CLUSTER; i++) {
        Light light = lights[i];
        
        float3 view_pos = mul(camera.view_matrix, float4(light.position, 1.0)).xyz;

        if (view_pos.z + light.range < tile_far_z || view_pos.z - light.range > tile_near_z) {
            continue;
        }

        if (test_sphere_aabb(view_pos, light.range, aabb_min, aabb_max)) {
            visible_lights[light_count++] = i;
        }
    }

    uint cluster_idx = global_id.x + 
                       (global_id.y * cluster_data.grid_dim.x) + 
                       (global_id.z * cluster_data.grid_dim.x * cluster_data.grid_dim.y);
                       
    uint offset = cluster_idx * MAX_LIGHTS_PER_CLUSTER;
    
    cluster_grid[cluster_idx].offset = offset;
    cluster_grid[cluster_idx].count = light_count;
    
    for(uint j = 0; j < light_count; j++) {
        light_indices[offset + j] = visible_lights[j];
    }
}