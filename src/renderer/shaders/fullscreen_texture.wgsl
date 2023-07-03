struct VertexOutput {
    @builtin(position) clip_position: vec4f,
    @location(0) uv: vec2f,
};


@vertex
fn vs_main(
    @builtin(vertex_index) in_vertex_index: u32,
) -> VertexOutput {
    var out: VertexOutput;

    // Expects Topology::TriangleStrips, Ccw winding and 4 vertices
    let x = f32(in_vertex_index / 2u) * 2.0 - 1.0;
    let y = f32(1u - (in_vertex_index & 1u)) * 2.0 - 1.0;
    out.clip_position = vec4f(x, y, 0.0, 1.0);

    out.uv.x = f32(in_vertex_index / 2u);
    out.uv.y = f32(in_vertex_index & 1u);

    return out;
}

@group(0) @binding(0)
var tex_texture: texture_2d<f32>;
@group(0) @binding(1)
var tex_sampler: sampler;

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4f {
    let d = textureSample(tex_texture, tex_sampler, in.uv).r;
    let dd = linearize_depth(d);
    let color = vec4f(vec3f(dd), 1.0);
    return color;
}

fn linearize_depth(depth: f32) -> f32 {
    let near = 0.05;
    let far = 100.0;

    return (2.0 * near) / (far + near - depth * (far - near));
}