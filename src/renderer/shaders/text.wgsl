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
    return fs_signed_distance_field(in);
}

fn fs_signed_distance_field(in: VertexOutput) -> vec4f {
    let fill_color = vec3f(0.9);
    let outline_color = vec3f(0.0);
    
    // TODO: potential improvement: have these values automatically calculated from data about the sdffont
    // instead of hand picking them.
    let fill_smoothcenter = 0.3;
    let outline_smoothcenter = 0.7;
    let smoothing = 0.15;

    let sd = textureSample(font_atlas, tex_sampler, in.uv);
    
    let fill_dist = smoothstep(fill_smoothcenter - smoothing, fill_smoothcenter + smoothing, sd.b);
    let outline_dist = smoothstep(outline_smoothcenter - smoothing, outline_smoothcenter + smoothing, sd.b);

    let fill_alpha = 1.0 - fill_dist;
    let outline_alpha = 1.0 - outline_dist;

    let alpha = outline_alpha;
    let color = mix(outline_color, fill_color, fill_alpha);

    return vec4f(color, alpha);
}
