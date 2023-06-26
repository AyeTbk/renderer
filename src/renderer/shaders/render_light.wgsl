struct SceneUniform {
    projection: mat4x4f,
    view: mat4x4f,
    camera_transform: mat4x4f,
    ambient_light: vec4f,
};
@group(0) @binding(0)
var<uniform> scene: SceneUniform;

struct VertexOutput {
    @builtin(position) @invariant clip_position: vec4f, // @invariant is necessary because lighting pipelines want to compare if depth is equal.
    @location(0) frag_pos: vec3f,
    @location(1) normal: vec3f,
    @location(2) uv: vec2f,
};

struct MaterialUniform {
    base_color: vec4f,
    billboard_mode: u32, // Off: 0, On: 1, Fixed-size: 2
    unlit: u32,
};
@group(1) @binding(0)
var<uniform> material: MaterialUniform;
@group(1) @binding(1)
var base_color_texture: texture_2d<f32>;
@group(1) @binding(2)
var material_sampler: sampler;

struct LightUniform {
    transform: mat4x4f,
    color: vec4f,
    radius: f32,
    kind: u32, // Directional=0, Point=1
};
@group(3) @binding(0)
var<uniform> light: LightUniform;


@fragment
fn fs_main_blinn_phong(in: VertexOutput) -> @location(0) vec4f {
    if material.unlit == 1u {
        // TODO This probably should just not be a draw call...
        discard;
    }

    let normal = normalize(in.normal);
    let base_color = material.base_color.rgba * textureSample(base_color_texture, material_sampler, in.uv).rgba;
    
    if base_color.a < 0.5 {
        discard;
    }

    let from_frag_to_view_dir = normalize(scene.camera_transform.w.xyz - in.frag_pos);
    var light_contribution = vec3f(0.0);
    if light.kind == 0u {
        light_contribution = compute_light_blinn_phong(
            base_color.rgb,
            normal,
            from_frag_to_view_dir,
            light.transform.z.xyz,
            light.color.rgb,
            light.color.a,
            8.0,
        );
    } else if light.kind == 1u {
        let distance = distance(in.frag_pos, light.transform.w.xyz);
        if distance > light.radius {
            discard;
        }
        let light_direction = normalize(in.frag_pos - light.transform.w.xyz);
        let attenuation = compute_light_attenuation(distance, light.radius);
        light_contribution = compute_light_blinn_phong(
            base_color.rgb,
            normal,
            from_frag_to_view_dir,
            light_direction,
            light.color.rgb,
            light.color.a * attenuation,
            8.0,
        );
    }

    return vec4f(light_contribution, 1.0);
}

fn compute_light_blinn_phong(
    base_color: vec3f,
    normal: vec3f,
    from_frag_to_view_dir: vec3f,
    light_dir: vec3f,
    light_color: vec3f,
    light_intensity: f32,
    shininess: f32,
) -> vec3f {
    // Diffuse
    let from_frag_to_light_dir = -light_dir;
    let diffuse_intensity = max(dot(normal, from_frag_to_light_dir), 0.0);
    let diffuse = light_color * light_intensity * diffuse_intensity;

    // Specular
    let halfway_dir = normalize(from_frag_to_light_dir + from_frag_to_view_dir);
    let spec_intensity = pow(max(dot(normal, halfway_dir), 0.0), shininess);
    let spec = light_color * light_intensity * spec_intensity;

    return base_color * (diffuse + spec);
}

fn compute_light_attenuation(distance: f32, max_distance: f32) -> f32 {
    let linear_attenuation = clamp((max_distance - distance) / max_distance, 0.0, 1.0);
    return smoothstep(0.0, 1.0, linear_attenuation);
}