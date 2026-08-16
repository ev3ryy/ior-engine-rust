import common;
import pbr;

[[vk::binding(0, 1)]] ConstantBuffer<CameraUniform> camera;
[[vk::binding(1, 1)]] ConstantBuffer<LightsBuffer> light_data;
[[vk::binding(0, 0)]] ConstantBuffer<MaterialUniform> material;

[[vk::binding(1, 0)]] Texture2D<float4> t_albedo;
[[vk::binding(2, 0)]] Texture2D<float4> t_normal;
[[vk::binding(3, 0)]] Texture2D<float4> t_orm;
[[vk::binding(4, 0)]] SamplerState s_sampler;

struct VertexOutput {
    float4 clip_position : SV_Position;
    [[vk::location(0)]] float2 tex_coords;
    [[vk::location(1)]] float3 world_position;
    [[vk::location(2)]] float3 world_normal;
    [[vk::location(3)]] float3 world_tangent;
    [[vk::location(4)]] float3 world_bitangent;
};

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
    
    out.world_normal = normalize(mul(normal_matrix, float4(normal, 0.0)).xyz);
    out.world_tangent = normalize(mul(model_matrix, float4(tangent.xyz, 0.0)).xyz);
    out.world_bitangent = cross(out.world_normal, out.world_tangent) * tangent.w;

    out.clip_position = mul(camera.view_proj, world_pos);
    return out;
}

[shader("fragment")]
float4 fs_main(
    [[vk::location(0)]] float2 tex_coords,
    [[vk::location(1)]] float3 world_position,
    [[vk::location(2)]] float3 world_normal,
    [[vk::location(3)]] float3 world_tangent,
    [[vk::location(4)]] float3 world_bitangent
) : SV_Target {
    
    float4 albedo_tex = t_albedo.Sample(s_sampler, tex_coords);
    float3 albedo = albedo_tex.rgb * material.base_color_factor.rgb;
    
    float4 orm = t_orm.Sample(s_sampler, tex_coords);
    float ao = orm.r;
    float roughness = clamp(orm.g * material.roughness_factor, 0.05, 1.0);
    float metallic = clamp(orm.b * material.metallic_factor, 0.0, 1.0);
    
    float3 raw_normal = t_normal.Sample(s_sampler, tex_coords).xyz * 2.0 - 1.0;
    float3 map_normal = normalize(raw_normal);

    float3x3 TBN = transpose(float3x3(
        normalize(world_tangent),
        normalize(world_bitangent),
        normalize(world_normal)
    ));
    
    float3 N = normalize(mul(TBN, map_normal));
    float3 V = normalize(camera.view_position.xyz - world_position);
    
    float3 F0 = lerp(float3(0.04), albedo, metallic);
    float3 c_diff = albedo * (1.0 - metallic);

    float3 Lo = float3(0.0);
    float NdotV = max(dot(N, V), 0.0001);

    for (uint i = 0; i < light_data.count; i++) {
        Light light = light_data.lights[i];
        float3 L = float3(0.0);
        float attenuation = 1.0;
        
        if (light.light_type == 0) {
            L = normalize(-light.direction);
        } else {
            float3 light_vec = light.position - world_position;
            float distance = length(light_vec);
            L = normalize(light_vec);
            
            float dist_sq = max(distance * distance, 0.0001);
            float dist_ratio = distance / max(light.range, 0.001);
            float dist_ratio4 = dist_ratio * dist_ratio * dist_ratio * dist_ratio;
            float falloff = max(0.0, 1.0 - dist_ratio4);
            attenuation = (1.0 / dist_sq) * (falloff * falloff);
            
            if (light.light_type == 2) {
                float theta = dot(L, normalize(-light.direction));
                float epsilon = light.inner_cone_cos - light.outer_cone_cos;
                float spot_intensity = clamp((theta - light.outer_cone_cos) / epsilon, 0.0, 1.0);
                attenuation *= spot_intensity * spot_intensity;
            }
        }
        
        float3 radiance = light.color * light.intensity * attenuation;
        
        float3 H = normalize(V + L);
        float NdotL = max(dot(N, L), 0.0001);
        float NdotH = max(dot(N, H), 0.0);
        float HdotV = max(dot(H, V), 0.0);
        
        float D = distribution_ggx(NdotH, roughness);
        float3 F = fresnel_schlick(HdotV, F0);
        float Vis = visibility_schlick_ggx(NdotL, NdotV, roughness);
        
        float3 f_specular = D * F * Vis;
        float3 kD = float3(1.0) - F; 
        float3 f_diffuse = (kD * c_diff) / PI;
        
        Lo += (f_diffuse + f_specular) * radiance * NdotL;
    }

    float3 ambient = float3(0.02) * albedo * ao;
    float3 final_color = ambient + Lo;

    float a = 2.51; float b = 0.03; float c = 2.43; float d = 0.59; float e = 0.14;
    final_color = clamp((final_color * (a * final_color + b)) / (final_color * (c * final_color + d) + e), 0.0, 1.0);
    
    return float4(final_color, albedo_tex.a);
}