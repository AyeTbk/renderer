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
