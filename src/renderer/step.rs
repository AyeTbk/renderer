use crate::Vertex;

use super::{backend::Backend, pipeline::PipelineData};

pub struct Step {
    pub pipeline: wgpu::RenderPipeline,
}

impl Step {}

pub fn build_render_meshes_step(pipeline_data: &PipelineData, backend: &mut Backend) -> Step {
    Step {
        pipeline: backend
            .device
            .create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("render meshes pipeline"),
                layout: Some(&pipeline_data.pipeline_layouts.render_meshes),
                vertex: wgpu::VertexState {
                    module: &pipeline_data.shaders.render_meshes,
                    entry_point: "vs_main",
                    buffers: &[Vertex::buffer_layout()],
                },
                fragment: Some(wgpu::FragmentState {
                    module: &pipeline_data.shaders.render_meshes,
                    entry_point: "fs_main",
                    targets: &[Some(wgpu::ColorTargetState {
                        format: pipeline_data.render_target.color_format,
                        blend: Some(wgpu::BlendState::REPLACE),
                        write_mask: wgpu::ColorWrites::ALL,
                    })],
                }),
                primitive: wgpu::PrimitiveState {
                    topology: wgpu::PrimitiveTopology::TriangleList,
                    strip_index_format: None,
                    front_face: wgpu::FrontFace::Ccw,
                    cull_mode: Some(wgpu::Face::Back),
                    ..Default::default()
                },
                depth_stencil: Some(wgpu::DepthStencilState {
                    format: pipeline_data.render_target.depth_format,
                    depth_write_enabled: true,
                    depth_compare: wgpu::CompareFunction::Less,
                    stencil: wgpu::StencilState::default(),
                    bias: wgpu::DepthBiasState::default(),
                }),
                multisample: wgpu::MultisampleState {
                    count: pipeline_data.render_target.sample_count,
                    mask: !0,
                    alpha_to_coverage_enabled: false,
                },
                multiview: None,
            }),
    }
}
