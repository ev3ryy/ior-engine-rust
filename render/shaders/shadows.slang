module shadows;
import common;

// --- binding group 2---
[[vk::binding(0, 2)]] public DepthTexture2DArray shadow_map;
[[vk::binding(1, 2)]] public SamplerComparisonState shadow_sampler;
[[vk::binding(2, 2)]] public DepthTexture2DArray spot_shadow_map;
[[vk::binding(3, 2)]] public ConstantBuffer<SpotShadowUniform> spot_shadow_data;
[[vk::binding(4, 2)]] public DepthTextureCubeArray point_shadow_map;
[[vk::binding(5, 2)]] public ConstantBuffer<PointShadowUniform> point_shadow_data;

// --- Sample PCF shadows ---
public float sample_spot_shadow(
    float3 world_pos,
    float3 N,
    float3 L,
    uint spot_idx,
    float light_dist,
    float outer_cone_cos
) {
    float half_angle = acos(clamp(outer_cone_cos, -1.0, 0.9999));
    float tan_half_fov = tan(half_angle);
    float texel_size_world = (2.0 * light_dist * tan_half_fov) * SPOT_SHADOW_TEXEL_SIZE;

    float cos_theta = clamp(dot(N, L), 0.0, 1.0);
    float sin_theta = sqrt(clamp(1.0 - cos_theta * cos_theta, 0.0, 1.0));
    float normal_bias = texel_size_world * (sin_theta * 1.5 + 0.5);

    float3 biased_pos = world_pos + N * normal_bias;
    float4 light_space_pos = mul(spot_shadow_data.shadow_matrices[spot_idx], float4(biased_pos, 1.0));
    if (light_space_pos.w <= 0.0) return 1.0;

    float3 proj_coords = light_space_pos.xyz / light_space_pos.w;
    proj_coords.x = proj_coords.x * 0.5 + 0.5;
    proj_coords.y = proj_coords.y * -0.5 + 0.5;

    if (proj_coords.x < 0.0 || proj_coords.x > 1.0 ||
        proj_coords.y < 0.0 || proj_coords.y > 1.0 ||
        proj_coords.z < 0.0 || proj_coords.z > 1.0) {
        return 1.0;
    }

    float compare_depth = proj_coords.z;
    float shadow = 0.0;

    for (int x = -1; x <= 1; ++x) {
        for (int y = -1; y <= 1; ++y) {
            float2 offset = float2(x, y) * SPOT_SHADOW_TEXEL_SIZE;
            shadow += spot_shadow_map.SampleCmp(
                shadow_sampler,
                float3(proj_coords.xy + offset, spot_idx),
                compare_depth
            );
        }
    }

    return shadow / 9.0;
}

// --- Sample point shadows ---
public float sample_point_shadow(
    float3 world_pos,
    float3 N,
    float3 light_pos,
    uint point_idx,
    float light_range
) {
    float3 light_to_frag = world_pos - light_pos;
    float dist = length(light_to_frag);
    if (dist >= light_range) return 1.0;

    float near_plane = point_shadow_data.light_params[point_idx].x;
    float far_plane = point_shadow_data.light_params[point_idx].y;

    float normal_bias = 0.03 * (dist / far_plane);
    float3 biased_pos = world_pos + N * normal_bias;
    float3 biased_dir = biased_pos - light_pos;

    float3 abs_dir = abs(biased_dir);
    float max_axis = max(abs_dir.x, max(abs_dir.y, abs_dir.z));

    float compare_depth = (far_plane / (far_plane - near_plane)) - (far_plane * near_plane) / ((far_plane - near_plane) * max_axis);
    compare_depth = clamp(compare_depth - 0.002, 0.0, 1.0);

    return point_shadow_map.SampleCmp(
        shadow_sampler,
        float4(biased_dir, float(point_idx)),
        compare_depth
    );
}

// --- Select cascade shadow ---
public float sample_cascade_shadow(
    float3 world_pos,
    float3 N,
    float3 L,
    uint cascade_idx,
    ConstantBuffer<ShadowUniform> shadow_data
) {
    float texel_size_world = shadow_data.cascade_texel_sizes[cascade_idx];

    float cos_theta = clamp(dot(N, L), 0.0, 1.0);
    float sin_theta = sqrt(clamp(1.0 - cos_theta * cos_theta, 0.0, 1.0));
    float normal_bias = texel_size_world * (sin_theta * 1.5 + 0.5);

    float3 biased_pos = world_pos + N * normal_bias;
    float4 light_space_pos = mul(shadow_data.light_space_matrices[cascade_idx], float4(biased_pos, 1.0));
    if (light_space_pos.w <= 0.0) return 1.0;

    float3 proj_coords = light_space_pos.xyz / light_space_pos.w;
    proj_coords.x = proj_coords.x * 0.5 + 0.5;
    proj_coords.y = proj_coords.y * -0.5 + 0.5;

    if (proj_coords.x < 0.0 || proj_coords.x > 1.0 ||
        proj_coords.y < 0.0 || proj_coords.y > 1.0 ||
        proj_coords.z < 0.0 || proj_coords.z > 1.0) {
        return 1.0;
    }

    float compare_depth = proj_coords.z;
    float shadow = 0.0;

    for (int x = -1; x <= 1; ++x) {
        for (int y = -1; y <= 1; ++y) {
            float2 offset = float2(x, y) * SHADOW_TEXEL_SIZE;
            shadow += shadow_map.SampleCmp(
                shadow_sampler,
                float3(proj_coords.xy + offset, cascade_idx),
                compare_depth
            );
        }
    }

    return shadow / 9.0;
}

// --- CSM shadows ---
public float calc_csm_shadow(
    float3 world_pos,
    float view_z,
    float3 N,
    float3 L,
    ConstantBuffer<ShadowUniform> shadow_data
) {
    if (view_z > shadow_data.cascade_splits[3]) {
        return 1.0;
    }

    uint cascade_idx = 3;
    for (uint i = 0; i < 4; ++i) {
        if (view_z <= shadow_data.cascade_splits[i]) {
            cascade_idx = i;
            break;
        }
    }

    float shadow = sample_cascade_shadow(world_pos, N, L, cascade_idx, shadow_data);

    if (cascade_idx < 3) {
        float cascade_far = shadow_data.cascade_splits[cascade_idx];
        float cascade_near = (cascade_idx > 0) ? shadow_data.cascade_splits[cascade_idx - 1] : 0.0;

        float cascade_range = cascade_far - cascade_near;
        float blend_width = cascade_range * 0.15;
        float blend_start = cascade_far - blend_width;

        if (view_z > blend_start) {
            float blend_t = smoothstep(blend_start, cascade_far, view_z);
            float next_shadow = sample_cascade_shadow(world_pos, N, L, cascade_idx + 1, shadow_data);
            shadow = lerp(shadow, next_shadow, blend_t);
        }
    }

    return shadow;
}