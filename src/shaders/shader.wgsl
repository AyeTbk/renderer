struct CameraUniform {
    projection_view: mat4x4f,
};
@group(0) @binding(0)
var<uniform> camera: CameraUniform;

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
};


@vertex
fn vs_main(
    vertex: VertexInput,
) -> VertexOutput {
    var out: VertexOutput;
    out.clip_position = camera.projection_view * model.transform * vec4f(vertex.pos, 1.0);
    out.normal = (model.transform * vec4f(vertex.normal, 0.0)).xyz;
    return out;
}



struct MaterialUniform {
    base_color: vec4f,
};
@group(1) @binding(0)
var<uniform> material: MaterialUniform;


@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4f {
    let col = material.base_color.xyz * 0.2 + in.normal * 0.8;
    return vec4f(col, 1.0);
}
