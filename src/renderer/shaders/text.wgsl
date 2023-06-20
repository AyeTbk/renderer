// struct VertexInput {
// };

struct InstanceInput {
    @location(10) pos: vec2f,
    @location(11) scale: vec2f,
    @location(12) glyph: u32,
};

struct VertexOutput {
    @builtin(position) clip_position: vec4f,
    @location(0) uv: vec2f,
};

struct ViewportUniform {
    size: vec2u,
};
@group(0) @binding(0)
var<uniform> viewport: ViewportUniform;


@group(1) @binding(0)
var font_atlas: texture_2d<f32>;
@group(1) @binding(1)
var tex_sampler: sampler;



@vertex
fn vs_main(
    @builtin(vertex_index) in_vertex_index: u32,
    // vertex: VertexInput,
    instance: InstanceInput,
) -> VertexOutput {
    var out: VertexOutput;
    let viewport_size = vec2f(viewport.size);

    // Expects Topology::TriangleStrips, Ccw winding and 4 vertices
    let x = f32(in_vertex_index / 2u);
    let y = f32(1u - (in_vertex_index & 1u));
    
    let pos = vec2f(x, y);
    let scaled_pos = pos * instance.scale;
    let translation = vec2f(instance.pos.x, viewport_size.y - instance.pos.y - instance.scale.y);
    let translated_pos = scaled_pos + translation;

    let clip_pos = (translated_pos / viewport_size) * 2.0 - 1.0;
    out.clip_position = vec4f(clip_pos, 0.0, 1.0);

    // Expects a wide image of 128 monospaced characters
    let quad_uv = vec2f(
        f32(in_vertex_index / 2u),
        f32(in_vertex_index & 1u),
    );
    let glyph_tile_count = 128.0;
    let glyph_tile_width = 1.0 / glyph_tile_count;
    let glyph_uv = vec2f(
        (quad_uv.x / glyph_tile_count) + f32(instance.glyph) * glyph_tile_width,
        quad_uv.y,
    );

    out.uv = glyph_uv;

    return out;
}


@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4f {
    // let dist = 1.0 - abs(sdf.r - 0.5) * 2.0;
    // let alpha = smoothstep(0.35, 0.4, dist);

    let fill_smoothing_center = 0.1;
    let fill_smooth = 0.1;
    let fill_color = vec3f(0.8);
    let outline_smoothing_center = 0.05;
    let outline_smooth = 0.05;
    let outline_color = vec3f(0.0);

    let sdf = textureSample(font_atlas, tex_sampler, in.uv);
    let dist_in = abs(clamp(sdf.b - 0.5, -0.5, 0.0)) * 2.0;
    let dist_out = 1.0 - abs(sdf.b - 0.5) * 2.0;


    // let fill_alpha = smoothstep(fill_smoothing_center - fill_smooth, fill_smoothing_center + fill_smooth, dist_in);
    // let outline_alpha = smoothstep(outline_smoothing_center - outline_smooth, outline_smoothing_center + outline_smooth, dist_out);
    let fill_alpha = smoothstep(0.01, 0.12, dist_in);
    let outline_alpha = smoothstep(0.7, 0.99, dist_out);

    let alpha = max(fill_alpha, outline_alpha);
    let color = mix(fill_color, outline_color, outline_alpha);

    let fill = vec4f(fill_color, fill_alpha);
    let outline = vec4f(outline_color, outline_alpha);
    let both = fill * fill_alpha + outline * outline_alpha;

    return both;
}
