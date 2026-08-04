// NAT3D Forward Rendering Shader
// PBR (Physically Based Rendering) with Cook-Torrance BRDF

struct CameraUniforms {
    view_proj: mat4x4<f32>,
    view: mat4x4<f32>,
    proj: mat4x4<f32>,
    camera_pos: vec4<f32>,
}

struct ModelUniforms {
    model: mat4x4<f32>,
    normal: mat4x4<f32>,
}

struct MaterialUniforms {
    base_color: vec4<f32>,
    metallic: f32,
    roughness: f32,
    ao: f32,
    emissive: f32,
    simulation_mode: u32,
}

@group(0) @binding(0)
var<uniform> camera: CameraUniforms;

@group(1) @binding(0)
var<uniform> model: ModelUniforms;

@group(2) @binding(0)
var<uniform> material: MaterialUniforms;

struct VertexInput {
    @location(0) position: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) tex_coords: vec2<f32>,
    @location(3) color: vec4<f32>,
    @location(4) physiological_signal: f32,
}

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) world_position: vec3<f32>,
    @location(1) world_normal: vec3<f32>,
    @location(2) tex_coords: vec2<f32>,
    @location(3) color: vec4<f32>,
    @location(4) physiological_signal: f32,
}

@vertex
fn vs_main(in: VertexInput) -> VertexOutput {
    var out: VertexOutput;

    let world_pos = model.model * vec4<f32>(in.position, 1.0);
    out.world_position = world_pos.xyz;
    out.clip_position = camera.view_proj * world_pos;
    out.world_normal = normalize((model.normal * vec4<f32>(in.normal, 0.0)).xyz);
    out.tex_coords = in.tex_coords;
    out.color = in.color;
    out.physiological_signal = in.physiological_signal;

    return out;
}

// PBR lighting functions
const PI: f32 = 3.14159265359;

fn distribution_ggx(n: vec3<f32>, h: vec3<f32>, roughness: f32) -> f32 {
    let a = roughness * roughness;
    let a2 = a * a;
    let n_dot_h = max(dot(n, h), 0.0);
    let n_dot_h2 = n_dot_h * n_dot_h;

    let denom = n_dot_h2 * (a2 - 1.0) + 1.0;
    return a2 / (PI * denom * denom);
}

fn geometry_schlick_ggx(n_dot_v: f32, roughness: f32) -> f32 {
    let r = roughness + 1.0;
    let k = (r * r) / 8.0;
    return n_dot_v / (n_dot_v * (1.0 - k) + k);
}

fn geometry_smith(n: vec3<f32>, v: vec3<f32>, l: vec3<f32>, roughness: f32) -> f32 {
    let n_dot_v = max(dot(n, v), 0.0);
    let n_dot_l = max(dot(n, l), 0.0);
    let ggx2 = geometry_schlick_ggx(n_dot_v, roughness);
    let ggx1 = geometry_schlick_ggx(n_dot_l, roughness);
    return ggx1 * ggx2;
}

fn fresnel_schlick(cos_theta: f32, f0: vec3<f32>) -> vec3<f32> {
    return f0 + (1.0 - f0) * pow(1.0 - cos_theta, 5.0);
}


fn voltage_to_color(v: f32) -> vec3<f32> {
    let v_min = -90.0;
    let v_max = 50.0;
    let t = clamp((v - v_min) / (v_max - v_min), 0.0, 1.0);
    var color: vec3<f32>;
    if (t < 0.25) {
        let local_t = t / 0.25;
        color = mix(vec3<f32>(0.0, 0.0, 0.3), vec3<f32>(0.0, 0.5, 0.8), local_t);
    } else if (t < 0.5) {
        let local_t = (t - 0.25) / 0.25;
        color = mix(vec3<f32>(0.0, 0.5, 0.8), vec3<f32>(0.0, 0.7, 0.2), local_t);
    } else if (t < 0.75) {
        let local_t = (t - 0.5) / 0.25;
        color = mix(vec3<f32>(0.0, 0.7, 0.2), vec3<f32>(1.0, 0.9, 0.0), local_t);
    } else {
        let local_t = (t - 0.75) / 0.25;
        color = mix(vec3<f32>(1.0, 0.9, 0.0), vec3<f32>(1.0, 0.1, 0.0), local_t);
    }
    return color;
}

fn o2_to_color(sat: f32) -> vec3<f32> {
    return mix(vec3<f32>(0.0, 0.0, 1.0), vec3<f32>(1.0, 0.0, 0.0), clamp(sat, 0.0, 1.0));
}

fn temp_to_color(t: f32) -> vec3<f32> {
    let t_min = 35.0;
    let t_max = 42.0;
    let factor = clamp((t - t_min) / (t_max - t_min), 0.0, 1.0);
    return mix(vec3<f32>(0.0, 0.5, 1.0), vec3<f32>(1.0, 0.2, 0.0), factor);
}

fn metabolic_to_color(m: f32) -> vec3<f32> {
    let t = clamp(m, 0.0, 1.0);
    if (t < 0.5) {
        return mix(vec3<f32>(0.3, 0.0, 0.5), vec3<f32>(0.0, 0.8, 0.4), t * 2.0);
    } else {
        return mix(vec3<f32>(0.0, 0.8, 0.4), vec3<f32>(1.0, 1.0, 0.0), (t - 0.5) * 2.0);
    }
}

fn pressure_to_color(p: f32) -> vec3<f32> {
    return mix(vec3<f32>(0.0, 0.6, 0.6), vec3<f32>(1.0, 0.5, 0.0), clamp(p, 0.0, 1.0));
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    
    var albedo = material.base_color.rgb * in.color.rgb;
    if (material.simulation_mode != 0u) {
        if (material.simulation_mode == 1u) {
            albedo = voltage_to_color(in.physiological_signal);
        } else if (material.simulation_mode == 2u) {
            albedo = o2_to_color(in.physiological_signal);
        } else if (material.simulation_mode == 3u) {
            albedo = temp_to_color(in.physiological_signal);
        } else if (material.simulation_mode == 4u) {
            albedo = metabolic_to_color(in.physiological_signal);
        } else if (material.simulation_mode == 5u) {
            albedo = pressure_to_color(in.physiological_signal);
        }
    }
    let metallic = material.metallic;
    let roughness = max(material.roughness, 0.04);
    let ao = material.ao;

    let n = normalize(in.world_normal);
    let v = normalize(camera.camera_pos.xyz - in.world_position);

    // Simple directional light (warm sunlight)
    let light_dir = normalize(vec3<f32>(0.5, 1.0, 0.3));
    let light_color = vec3<f32>(1.0, 0.98, 0.95);
    let light_intensity = 2.0;

    // Calculate reflectance at normal incidence
    var f0 = vec3<f32>(0.04);
    f0 = mix(f0, albedo, metallic);

    // Calculate per-light radiance
    let l = light_dir;
    let h = normalize(v + l);
    let radiance = light_color * light_intensity;

    // Cook-Torrance BRDF
    let ndf = distribution_ggx(n, h, roughness);
    let g = geometry_smith(n, v, l, roughness);
    let f = fresnel_schlick(max(dot(h, v), 0.0), f0);

    let kS = f;
    var kD = vec3<f32>(1.0) - kS;
    kD *= 1.0 - metallic;

    let numerator = ndf * g * f;
    let denominator = 4.0 * max(dot(n, v), 0.0) * max(dot(n, l), 0.0) + 0.0001;
    let specular = numerator / denominator;

    let n_dot_l = max(dot(n, l), 0.0);
    var lo = (kD * albedo / PI + specular) * radiance * n_dot_l;

    // Add fill light from below (subtle blue)
    let fill_dir = normalize(vec3<f32>(-0.3, -0.5, -0.2));
    let fill_intensity = 0.3;
    let fill_n_dot_l = max(dot(n, fill_dir), 0.0);
    lo += albedo * fill_intensity * fill_n_dot_l;

    // Ambient
    let ambient = vec3<f32>(0.15) * albedo * ao;
    var color = ambient + lo;

    // Emissive
    color += albedo * material.emissive;

    // HDR tonemapping (Reinhard)
    color = color / (color + vec3<f32>(1.0));

    // Gamma correction
    color = pow(color, vec3<f32>(1.0 / 2.2));

    return vec4<f32>(color, material.base_color.a * in.color.a);
}
