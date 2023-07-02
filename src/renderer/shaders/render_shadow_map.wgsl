struct VertexInput {
    @location(0) pos: vec3f,
    @location(1) normal: vec3f,
    @location(2) uv: vec2f,
};

struct VertexOutput {
    @builtin(position) clip_position: vec4f,
};


struct SceneUniform {
    projection: mat4x4f,
    view: mat4x4f,
    camera_transform: mat4x4f,
    ambient_light: vec4f,
};
@group(0) @binding(0)
var<uniform> scene: SceneUniform;

struct ModelUniform {
    transform: mat4x4f,
};
@group(1) @binding(0)
var<uniform> model: ModelUniform;


@vertex
fn vs_main(vertex: VertexInput) -> VertexOutput {
    var out: VertexOutput;

    let projection_view = scene.projection * scene.view;
    let vertex_pos_in_world_space = model.transform * vec4f(vertex.pos, 1.0);
    out.clip_position = projection_view * vertex_pos_in_world_space;

    return out;
}
