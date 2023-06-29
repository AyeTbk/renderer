struct SceneUniform {
    projection: mat4x4f,
    view: mat4x4f,
    camera_transform: mat4x4f,
    ambient_light: vec4f,
};
@group(0) @binding(0)
var<uniform> scene: SceneUniform;

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

struct ModelUniform {
    transform: mat4x4f,
};
@group(2) @binding(0)
var<uniform> model: ModelUniform;

struct VertexInput {
    @location(0) pos: vec3f,
    @location(1) normal: vec3f,
    @location(2) uv: vec2f,
};

struct VertexOutput {
    @builtin(position) @invariant clip_position: vec4f, // @invariant is necessary because lighting pipelines want to compare if depth is equal.
    @location(0) frag_pos: vec3f,
    @location(1) normal: vec3f,
    @location(2) uv: vec2f,
};


@vertex
fn vs_main(
    vertex: VertexInput,
) -> VertexOutput {
    var out: VertexOutput;
    
    let projection_view = scene.projection * scene.view;

    let vertex_pos_in_world_space = model.transform * vec4f(vertex.pos, 1.0);
    out.clip_position = projection_view * vertex_pos_in_world_space;
    out.frag_pos = vertex_pos_in_world_space.xyz;
    
    // FIXME: This is incorrect, normals will be wrong with a non-uniform scaling factor (look up 'normal matrix')
    out.normal = (model.transform * vec4f(vertex.normal, 0.0)).xyz;
    out.uv = vertex.uv;

    if material.billboard_mode == 1u {
        let transform = mat4x4f(
            scene.camera_transform.x * 0.4,
            scene.camera_transform.y * 0.4,
            scene.camera_transform.z * 0.4,
            model.transform.w,
        );
        let vertex_pos_in_world_space = transform * vec4f(vertex.pos, 1.0);
        out.clip_position = projection_view * vertex_pos_in_world_space;
        out.frag_pos = vertex_pos_in_world_space.xyz;
    } else if material.billboard_mode == 2u {
        let vp_model_pos = projection_view * vec4f(model.transform.w.xyz, 1.0);
        out.clip_position = vp_model_pos;
        out.clip_position /= out.clip_position.w;
        out.clip_position = vec4f(
            out.clip_position.xy + vertex.pos.xy * vec2f(0.4, 0.4),
            out.clip_position.z,
            out.clip_position.w,
        );
    
        out.normal = vertex.normal.xyz;
    }

    return out;
}



@fragment
fn fs_main_ambient_light_depth_prepass(in: VertexOutput) -> @location(0) vec4f {
    let normal = normalize(in.normal);
    let base_color = material.base_color.rgba * textureSample(base_color_texture, material_sampler, in.uv).rgba;
    
    if base_color.a < 0.5 {
        discard;
    }

    var ambient_light = base_color.rgb;
    if material.unlit == 0u {
        ambient_light = compute_ambient_light(
            base_color.rgb,
            scene.ambient_light.rgb,
            scene.ambient_light.a,
        );
    }

    return vec4f(ambient_light, base_color.a);
}

fn compute_ambient_light(base_color: vec3f, light_color: vec3f, light_intensity: f32) -> vec3f {
    return base_color * (light_color * light_intensity);
}


#ifdef LIGHTS

struct LightUniform {
    transform: mat4x4f,
    color: vec4f,
    radius: f32,
    kind: u32, // Directional=0, Point=1
};
@group(3) @binding(0)
var<uniform> light: LightUniform;

const LIGHT_KIND_DIRECTIONAL = 0u;
const LIGHT_KIND_POINT = 1u;


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
    if light.kind == LIGHT_KIND_DIRECTIONAL {
        light_contribution = compute_light_blinn_phong(
            base_color.rgb,
            normal,
            from_frag_to_view_dir,
            light.transform.z.xyz,
            light.color.rgb,
            light.color.a,
            8.0,
        );
    } else if light.kind == LIGHT_KIND_POINT {
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

#endif

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