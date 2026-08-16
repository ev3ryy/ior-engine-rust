module pbr;
import common;

public float distribution_ggx(float NdotH, float roughness) {
    float alpha = roughness * roughness;
    float alpha2 = alpha * alpha;
    float NdotH2 = NdotH * NdotH;
    
    float num = alpha2;
    float denom = (NdotH2 * (alpha2 - 1.0) + 1.0);
    return num / (PI * denom * denom);
}

public float3 fresnel_schlick(float HdotV, float3 F0) {
    float x = clamp(1.0 - HdotV, 0.0, 1.0);
    float x2 = x * x;
    float x5 = x2 * x2 * x;
    return F0 + (1.0 - F0) * x5;
}

public float visibility_schlick_ggx(float NdotL, float NdotV, float roughness) {
    float r = (roughness + 1.0);
    float k = (r * r) / 8.0;
    
    float g_v = NdotV / (NdotV * (1.0 - k) + k);
    float g_l = NdotL / (NdotL * (1.0 - k) + k);
    
    return (g_v * g_l) / max(4.0 * NdotL * NdotV, 0.0001);
}