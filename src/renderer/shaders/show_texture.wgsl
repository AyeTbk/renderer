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


struct ShowTextureUniform {
    tone_mapping: u32,
};
@group(0) @binding(0)
var<uniform> render: ShowTextureUniform;

@group(0) @binding(1)
var tex_texture: texture_2d<f32>;
@group(0) @binding(2)
var tex_sampler: sampler;

const TONE_MAPPING_NONE: u32 = 0u;
const TONE_MAPPING_REINHARD: u32 = 1u;


@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4f {
    var color = textureSample(tex_texture, tex_sampler, in.uv);
    
    switch render.tone_mapping {
        case TONE_MAPPING_REINHARD: {
            let tone_mapped = color.rgb / (luminance(color.rgb) + 1.0);
            color.r = tone_mapped.r;
            color.g = tone_mapped.g;
            color.b = tone_mapped.b;
        }
        default: {
            // Don't.
        }
    }

    return color;
}

fn luminance(v: vec3f) -> f32 {
    return 0.2126 * v.r + 0.7152 * v.g + 0.0722 * v.b;
}