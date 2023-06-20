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
    size: vec2u,
};
@group(0) @binding(0)
var<uniform> render: ShowTextureUniform;

@group(0) @binding(1)
var tex_texture: texture_2d<f32>;
@group(0) @binding(2)
var tex_sampler: sampler;


@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4f {
    return hardware_bilinear(in);
}

/// Fast and simple. Only gives good results down to 1/2 downscaling. Below that,
/// aliasing is introduced by the fact that bilinear filtering works on blocks
/// of 2x2 pixels.
/// Expects the sampler to use bilinear filtering.
fn hardware_bilinear(in: VertexOutput) -> vec4f {
    return textureSample(tex_texture, tex_sampler, in.uv);
}

/// Attempt at supporting non integer scaling and the ability to go below
/// 1/2 downscaling with good results. Didn't check for the former and didn't work
/// for the latter: aliasing is introduced when tested at 1/4 downscaling. I need
/// to learn more about signal processing.
/// Expects the sampler to use nearest filtering, but it'll work with bilinear too.
fn fs_main_free_window_sampling(in: VertexOutput) -> vec4f {
    let render_size = vec2f(render.size);
    let texture_size = vec2f(textureDimensions(tex_texture));

    let sampling_window_size = texture_size / render_size;
    let sample_texel_count = vec2i(ceil(sampling_window_size));

    // Top left corner of render pixel
    let pixel_pos = floor(in.uv * render_size);

    var total_coverage = 0.0;
    var weighted_samples_sum = vec4f(0.0);
    for (var i = 0; i < sample_texel_count.x; i++) {
        for (var j = 0; j < sample_texel_count.y; j++) {
            // Top left corner of render pixel
            let sample_pos = pixel_pos * sampling_window_size + vec2f(f32(i), f32(j)) + vec2f(0.5);

            let texel_pos_min = floor(sample_pos);
            let texel_pos_max = floor(sample_pos + vec2f(1.0));
            let texel_pos_center = texel_pos_min + vec2f(0.5);

            var sample_rect_min = texel_pos_min;
            var sample_rect_max = texel_pos_max;

            let xfirst = i == 0;
            let xlast = i == sample_texel_count.x - 1;
            let yfirst = j == 0;
            let ylast = j == sample_texel_count.y - 1;

            if xfirst && !xlast {
                sample_rect_min.x = sample_pos.x;
            }
            if !xfirst && xlast {
                sample_rect_max.x = sample_pos.x;
            }
            if yfirst && !ylast {
                sample_rect_min.y = sample_pos.y;
            }
            if !yfirst && ylast {
                sample_rect_max.y = sample_pos.y;
            }

            if !xfirst && !xlast {
                sample_rect_min.x = texel_pos_min.x;
                sample_rect_max.x = texel_pos_max.x;
            }
            if !yfirst && !ylast {
                sample_rect_min.y = texel_pos_min.y;
                sample_rect_max.y = texel_pos_max.y;
            }

            let sample_rect_size = sample_rect_max - sample_rect_min;
            let sample_area = sample_rect_size.x * sample_rect_size.y;

            let sample_texel_coverage = sample_area;

            total_coverage += sample_texel_coverage;
            let uv = texel_pos_center / texture_size;
            weighted_samples_sum +=
                textureSample(tex_texture, tex_sampler, uv) * sample_texel_coverage;
        }
    }

    if total_coverage == 0.0 {
        // this shouldnt happen but let's be careful
        total_coverage = 1.0;
    }
    let sample_average = weighted_samples_sum / total_coverage;

    let color = sample_average;

    return color;
}
