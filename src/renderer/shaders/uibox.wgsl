struct InstanceInput {
    @location(10) pos: vec2f,
    @location(11) size: vec2f,
    @location(12) color: vec4f,
};

struct VertexOutput {
    @builtin(position) clip_position: vec4f,
    @location(0) color: vec4f,
};

struct ViewportUniform {
    size: vec2u,
};
@group(0) @binding(0)
var<uniform> viewport: ViewportUniform;



@vertex
fn vs_main(
    @builtin(vertex_index) in_vertex_index: u32,
    instance: InstanceInput,
) -> VertexOutput {
    var out: VertexOutput;
    out.color = instance.color;

    let viewport_size = vec2f(viewport.size);

    // Expects Topology::TriangleStrips, Ccw winding and 4 vertices
    let x = f32(in_vertex_index / 2u);
    let y = f32(1u - (in_vertex_index & 1u));
    
    let pos = vec2f(x, y);
    let sized_pos = pos * instance.size;
    let translation = vec2f(instance.pos.x, viewport_size.y - instance.pos.y - instance.size.y);
    let translated_pos = sized_pos + translation;

    let clip_pos = (translated_pos / viewport_size) * 2.0 - 1.0;
    out.clip_position = vec4f(clip_pos, 0.0, 1.0);

    return out;
}


@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4f {
    return in.color;
}
