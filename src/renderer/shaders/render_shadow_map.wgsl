struct VertexInput {
    @location(0) pos: vec3f,
    @location(1) normal: vec3f,
    @location(2) uv: vec2f,
};

struct VertexOutput {
    @builtin(position) clip_position: vec4f,
};


struct CascadeUniform {
    projection_view: mat4x4f,
};
@group(0) @binding(0)
var<uniform> cascade: CascadeUniform;

struct ModelUniform {
    transform: mat4x4f,
};
@group(1) @binding(0)
var<uniform> model: ModelUniform;


@vertex
fn vs_main(vertex: VertexInput) -> VertexOutput {
    var out: VertexOutput;

    let vertex_pos_in_world_space = model.transform * vec4f(vertex.pos, 1.0);
    out.clip_position = cascade.projection_view * vertex_pos_in_world_space;

    return out;
}
