import common;
import pbr;
import shadows;

// --- materials group 0---
[[vk::binding(0, 0)]] ConstantBuffer<MaterialUniform> material;
[[vk::binding(1, 0)]] Texture2D<float4> t_albedo;
[[vk::binding(2, 0)]] Texture2D<float4> t_normal;
[[vk::binding(3, 0)]] Texture2D<float4> t_orm;
[[vk::binding(4, 0)]] SamplerState s_sampler;

// --- global frame and clustering resources group 0 ---
[[vk::binding(0, 1)]] ConstantBuffer<CameraUniform> camera;
[[vk::binding(1, 1)]] StructuredBuffer<Light> lights;
[[vk::binding(2, 1)]] ConstantBuffer<ClusterData> cluster_data;
[[vk::binding(3, 1)]] StructuredBuffer<ClusterLightGroup> cluster_grid;
[[vk::binding(4, 1)]] StructuredBuffer<uint> light_indices;
[[vk::binding(5, 1)]] ConstantBuffer<ShadowUniform> shadow_data;

struct VertexOutput {
    float4 clip_position : SV_Position;
    [[vk::location(0)]] float2 tex_coords;
    [[vk::location(1)]] float3 world_position;
    [[vk::location(2)]] float3 world_normal;
    [[vk::location(3)]] float3 world_tangent;
    [[vk::location(4)]] float3 world_bitangent;
};

// --- Vertex Shader ---
[shader("vertex")]
VertexOutput vs_main(
    [[vk::location(0)]] float3 position,
    [[vk::location(1)]] float2 tex_coords,
    [[vk::location(2)]] float3 normal,
    [[vk::location(3)]] float4 tangent,

    [[vk::location(5)]] float4 model_0,
    [[vk::location(6)]] float4 model_1,
    [[vk::location(7)]] float4 model_2,
    [[vk::location(8)]] float4 model_3,

    [[vk::location(9)]]  float4 normal_m_0,
    [[vk::location(10)]] float4 normal_m_1,
    [[vk::location(11)]] float4 normal_m_2,
    [[vk::location(12)]] float4 normal_m_3
) {
    float4x4 model_matrix = transpose(float4x4(model_0, model_1, model_2, model_3));
    float4x4 normal_matrix = transpose(float4x4(normal_m_0, normal_m_1, normal_m_2, normal_m_3));

    VertexOutput out;
    out.tex_coords = tex_coords;

    float4 world_pos = mul(model_matrix, float4(position, 1.0));
    out.world_position = world_pos.xyz;

    float3 world_n = mul(normal_matrix, float4(normal, 0.0)).xyz;
    out.world_normal = length(world_n) > 1e-4 ? normalize(world_n) : float3(0.0, 1.0, 0.0);

    float3 world_t = mul(model_matrix, float4(tangent.xyz, 0.0)).xyz;
    if (length(world_t) > 1e-4) {
        out.world_tangent = normalize(world_t);
        out.world_bitangent = cross(out.world_normal, out.world_tangent) * (tangent.w != 0.0 ? tangent.w : 1.0);
    } else {
        out.world_tangent = float3(0.0);
        out.world_bitangent = float3(0.0);
    }

    out.clip_position = mul(camera.view_proj, world_pos);
    return out;
}

// --- Depth Prepass Fragment Shader ---
[shader("fragment")]
void depth_fs_main([[vk::location(0)]] float2 tex_coords) {
    float4 albedo_tex = t_albedo.Sample(s_sampler, tex_coords);
    float cutoff = material.alpha_cutoff > 0.001 ? material.alpha_cutoff : 0.5;

    if (albedo_tex.a * material.base_color_factor.a < cutoff) {
        discard;
    }
}

// --- Main Forward+ Fragment Shader ---
[shader("fragment")]
float4 fs_main(
    [[vk::location(0)]] float2 tex_coords,
    [[vk::location(1)]] float3 world_position,
    [[vk::location(2)]] float3 world_normal,
    [[vk::location(3)]] float3 world_tangent,
    [[vk::location(4)]] float3 world_bitangent,
    float4 frag_coord : SV_Position
) : SV_Target {
    // 1. Alpha testing
    float4 albedo_tex = t_albedo.Sample(s_sampler, tex_coords);
    float alpha = albedo_tex.a * material.base_color_factor.a;
    if (alpha < material.alpha_cutoff || alpha < 0.5) {
        discard;
    }

    // 2. Material properties
    float3 albedo = albedo_tex.rgb;
    float4 orm = t_orm.Sample(s_sampler, tex_coords);
    float ao = orm.r;
    float roughness = clamp(orm.g * material.roughness_factor, 0.25, 1.0);
    float metallic = clamp(orm.b * material.metallic_factor, 0.0, 1.0);

    // 3. TBN
    float3 N = normalize(world_normal);
    if (length(world_tangent) > 0.5) {
        float3 raw_normal = t_normal.Sample(s_sampler, tex_coords).xyz * 2.0 - 1.0;
        float3 map_normal = normalize(raw_normal);
        N = normalize(
            map_normal.x * world_tangent +
            map_normal.y * world_bitangent +
            map_normal.z * world_normal
        );
    }

    float3 V = normalize(camera.view_position.xyz - world_position);
    float3 F0 = lerp(float3(0.04), albedo, metallic);
    float3 c_diff = albedo * (1.0 - metallic);
    float NdotV = max(dot(N, V), 0.0001);

    // 4. Cluster indexing
    uint tile_x = uint(frag_coord.x / cluster_data.tile_size.x);
    uint tile_y = uint(frag_coord.y / cluster_data.tile_size.y);

    float3 view_pos_vec = mul(camera.view_matrix, float4(world_position, 1.0)).xyz;
    float view_z = -view_pos_vec.z;
    view_z = clamp(view_z, cluster_data.z_near + 0.0001, cluster_data.z_far);

    float z_ratio = max(0.0, log(view_z / cluster_data.z_near));
    float z_scale = float(cluster_data.grid_dim.z) / log(cluster_data.z_far / cluster_data.z_near);
    uint slice_z = clamp(uint(z_ratio * z_scale), 0, cluster_data.grid_dim.z - 1);

    uint cluster_index = tile_x +
                        (tile_y * cluster_data.grid_dim.x) +
                        (slice_z * cluster_data.grid_dim.x * cluster_data.grid_dim.y);

    ClusterLightGroup cluster = cluster_grid[cluster_index];
    float3 Lo = float3(0.0);

    // 5. Directional light (CSM)
    for (uint d = 0; d < cluster_data.dir_light_count; d++) {
        Light light = lights[d];
        float3 L = normalize(-light.direction);

        float shadow = 1.0;
        if (d == 0) {
            shadow = calc_csm_shadow(world_position, view_z, N, L, shadow_data);
        }

        float3 dir_radiance = light.color * light.intensity * shadow;
        Lo += calc_pbr_lighting(L, V, N, NdotV, dir_radiance, roughness, F0, c_diff);
    }

    // 6. Clustered lights (Point and Spot)
    for (uint i = 0; i < cluster.count; i++) {
        uint light_idx = light_indices[cluster.offset + i];
        Light light = lights[light_idx];

        float3 light_vec = light.position - world_position;
        float distance = length(light_vec);
        float3 L = normalize(light_vec);

        float dist_sq = max(distance * distance, 0.0001);
        float dist_ratio = distance / max(light.range, 0.001);
        float dist_ratio4 = dist_ratio * dist_ratio * dist_ratio * dist_ratio;
        float falloff = max(0.0, 1.0 - dist_ratio4);
        float attenuation = (1.0 / dist_sq) * (falloff * falloff);

        if (attenuation < 0.001) continue;

        // Point Light Shadows
        if (light.light_type == LIGHT_TYPE_POINT) {
            if (light.shadow_index >= 0) {
                float point_shadow = sample_point_shadow(
                    world_position,
                    N,
                    light.position,
                    uint(light.shadow_index),
                    light.range
                );
                attenuation *= point_shadow;
            }
        }
        // Spot Light shadows
        else if (light.light_type == LIGHT_TYPE_SPOT) {
            float theta = dot(L, normalize(-light.direction));
            float epsilon = max(light.inner_cone_cos - light.outer_cone_cos, 0.001);
            float spot_intensity = clamp((theta - light.outer_cone_cos) / epsilon, 0.0, 1.0);
            attenuation *= spot_intensity * spot_intensity;

            if (light.shadow_index >= 0) {
                float spot_shadow = sample_spot_shadow(
                    world_position,
                    N,
                    L,
                    uint(light.shadow_index),
                    distance,
                    light.outer_cone_cos
                );
                attenuation *= spot_shadow;
            }
        }

        float3 radiance = light.color * light.intensity * attenuation;
        Lo += calc_pbr_lighting(L, V, N, NdotV, radiance, roughness, F0, c_diff);
    }

    // 7. Ambient
    float3 ambient = float3(0.03) * albedo * ao;
    float3 final_color = ambient + Lo;

    return float4(final_color, 1.0);
}