struct CameraUniform {
    view_projection: mat4x4f,
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
    @location(1) color: vec4f,
};

struct VertexOutput {
    @builtin(position) clip_position: vec4f,
    @location(0) color: vec3f,
};



struct MaterialUniform {
    base_color: vec4f,
};
@group(1) @binding(0)
var<uniform> material: MaterialUniform;

@vertex
fn vs_main(
    vertex: VertexInput,
) -> VertexOutput {
    var out: VertexOutput;
    out.clip_position = camera.view_projection * model.transform * vec4f(vertex.pos, 1.0);
    out.color = vertex.color.xyz;
    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4f {
    return vec4f(material.base_color.xyz, 1.0);
}
