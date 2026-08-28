module pbr;
import common;

// Trowbridge-Reitz GGX
public float distribution_ggx(float NdotH, float roughness) {
    float alpha = roughness * roughness;
    float alpha2 = alpha * alpha;
    float NdotH2 = NdotH * NdotH;

    float num = alpha2;
    float denom = (NdotH2 * (alpha2 - 1.0) + 1.0);
    return num / (PI * denom * denom);
}

// Fresnel-Schlick
public float3 fresnel_schlick(float HdotV, float3 F0) {
    float x = clamp(1.0 - HdotV, 0.0, 1.0);
    float x2 = x * x;
    float x5 = x2 * x2 * x;
    return F0 + (1.0 - F0) * x5;
}

// Schlick-GGX
public float visibility_schlick_ggx(float NdotL, float NdotV, float roughness) {
    float r = roughness + 1.0;
    float k = (r * r) / 8.0;

    float g_v = NdotV / (NdotV * (1.0 - k) + k);
    float g_l = NdotL / (NdotL * (1.0 - k) + k);

    return (g_v * g_l) / max(4.0 * NdotL * NdotV, 0.0001);
}

// Cook-Torrance
public float3 calc_pbr_lighting(
    float3 L,
    float3 V,
    float3 N,
    float NdotV,
    float3 radiance,
    float roughness,
    float3 F0,
    float3 c_diff
) {
    float3 H = normalize(V + L);

    float actual_NdotL = dot(N, L);
    if (actual_NdotL <= 0.0) return float3(0.0);

    float NdotL = actual_NdotL;
    float NdotH = max(dot(N, H), 0.0);
    float HdotV = max(dot(H, V), 0.0);

    float D = distribution_ggx(NdotH, roughness);
    float3 F = fresnel_schlick(HdotV, F0);
    float Vis = visibility_schlick_ggx(NdotL, NdotV, roughness);

    float3 f_specular = D * F * Vis;
    float3 kD = float3(1.0) - F;
    float3 f_diffuse = (kD * c_diff) / PI;

    return (f_diffuse + f_specular) * radiance * NdotL;
}