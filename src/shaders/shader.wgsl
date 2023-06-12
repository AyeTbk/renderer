struct SceneUniform {
    projection_view: mat4x4f,
    view_pos: vec4f,
    ambient_light: vec4f,
    sun_color: vec4f,
    sun_dir: vec4f,
};
@group(0) @binding(0)
var<uniform> scene: SceneUniform;

struct ModelUniform {
    transform: mat4x4f,
};
@group(2) @binding(0)
var<uniform> model: ModelUniform;

struct VertexInput {
    @location(0) pos: vec3f,
    @location(1) normal: vec3f,
};

struct VertexOutput {
    @builtin(position) clip_position: vec4f,
    @location(0) normal: vec3f,
    @location(1) frag_pos: vec3f,
};


@vertex
fn vs_main(
    vertex: VertexInput,
) -> VertexOutput {
    var out: VertexOutput;
    let vertex_pos_in_world_space = model.transform * vec4f(vertex.pos, 1.0);
    out.clip_position = scene.projection_view * vertex_pos_in_world_space;
    
    // FIXME: This is incorrect, normals will be wrong with a non-uniform scaling factor (look up 'normal matrix')
    out.normal = normalize((model.transform * vec4f(vertex.normal, 0.0)).xyz);

    out.frag_pos = vertex_pos_in_world_space.xyz;
    return out;
}



struct MaterialUniform {
    base_color: vec4f,
};
@group(1) @binding(0)
var<uniform> material: MaterialUniform;


@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4f {
    let ambient_light = scene.ambient_light.rgb * scene.ambient_light.a;
    
    // Sun light (blinn-phong)
    let from_frag_to_light_dir = -scene.sun_dir.xyz;
    let sun_diffuse_intensity = max(dot(in.normal, from_frag_to_light_dir), 0.0);
    let sun_diffuse = scene.sun_color.rgb * scene.sun_color.a * sun_diffuse_intensity;

    let shininess = 32.0;
    let from_frag_to_view_dir = normalize(scene.view_pos.xyz - in.frag_pos);
    let halfway_dir = normalize(from_frag_to_light_dir + from_frag_to_view_dir);
    let sun_spec_intensity = pow(max(dot(in.normal, halfway_dir), 0.0), shininess);
    let sun_spec = scene.sun_color.rgb * scene.sun_color.a * sun_spec_intensity;
    
    let sun_light = sun_diffuse + sun_spec;
    let color = (ambient_light + sun_light) * material.base_color.rgb;
    
    return vec4f(color, 1.0);
}
