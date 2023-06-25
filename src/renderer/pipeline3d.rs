use wgpu::CommandEncoder;

use crate::{
    arena::Handle,
    asset_server::{shader_source::ShaderSource, AssetChanges},
    AssetServer,
};

use super::{
    backend::Backend,
    visual_server::{RenderTarget, RenderTargetInfo},
    Vertex,
};

pub struct Pipeline3d {
    steps: Vec<Step>,
    data: Pipeline3dData,
}

pub struct Pipeline3dData {
    pub scene_bind_group: wgpu::BindGroup,
    pub render_target_info: RenderTargetInfo,
    pub pipeline_layouts: PipelineLayouts,
    pub bind_group_layouts: BindGroupLayouts,
    pub shaders: Shaders,
}

pub struct Step {
    pub pipeline: wgpu::RenderPipeline,
}

impl Pipeline3d {
    pub fn new(
        scene_uniform_buffer: &wgpu::Buffer,
        render_target_info: RenderTargetInfo,
        backend: &mut Backend,
        asset_server: &mut AssetServer,
    ) -> Self {
        let shader_source_handle =
            asset_server.load::<ShaderSource>("src/renderer/shaders/shader.wgsl");
        let shader_source = asset_server.get(shader_source_handle);
        let shaders = Shaders {
            render_meshes_source: shader_source_handle,
            render_meshes: backend
                .create_shader_module("render meshes shader", shader_source.source()),
        };

        let bind_group_layouts = BindGroupLayouts {
            scene: backend
                .device
                .create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                    label: Some("scene bind group layout"),
                    entries: &[wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Uniform,
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    }],
                }),
            material: backend
                .device
                .create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                    label: Some("material bind group layout"),
                    entries: &[
                        wgpu::BindGroupLayoutEntry {
                            binding: 0,
                            visibility: wgpu::ShaderStages::FRAGMENT,
                            ty: wgpu::BindingType::Buffer {
                                ty: wgpu::BufferBindingType::Uniform,
                                has_dynamic_offset: false,
                                min_binding_size: None,
                            },
                            count: None,
                        },
                        wgpu::BindGroupLayoutEntry {
                            binding: 1,
                            visibility: wgpu::ShaderStages::FRAGMENT,
                            ty: wgpu::BindingType::Texture {
                                sample_type: wgpu::TextureSampleType::Float { filterable: true },
                                view_dimension: wgpu::TextureViewDimension::D2,
                                multisampled: false,
                            },
                            count: None,
                        },
                        wgpu::BindGroupLayoutEntry {
                            binding: 2,
                            visibility: wgpu::ShaderStages::FRAGMENT,
                            ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                            count: None,
                        },
                    ],
                }),
            model: backend
                .device
                .create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                    label: Some("model bind group layout"),
                    entries: &[wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::VERTEX,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Uniform,
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    }],
                }),
        };

        let pipeline_layouts = PipelineLayouts {
            render_meshes: backend
                .device
                .create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                    label: Some("render meshes pipeline layout"),
                    bind_group_layouts: &[
                        &bind_group_layouts.scene,
                        &bind_group_layouts.material,
                        &bind_group_layouts.model,
                    ],
                    push_constant_ranges: &[],
                }),
        };

        let scene_bind_group = backend
            .device
            .create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("scene bind group"),
                layout: &bind_group_layouts.scene,
                entries: &[wgpu::BindGroupEntry {
                    binding: 0,
                    resource: scene_uniform_buffer.as_entire_binding(),
                }],
            });

        let data = Pipeline3dData {
            scene_bind_group,
            render_target_info,
            pipeline_layouts,
            bind_group_layouts,
            shaders,
        };

        let steps = Self::build_steps(&data, backend);

        Self { steps, data }
    }

    pub fn update_render_target_info(
        &mut self,
        render_target_info: RenderTargetInfo,
        backend: &mut Backend,
    ) {
        self.data.render_target_info = render_target_info;
        self.rebuild_steps(backend);
    }

    pub fn notify_asset_changes(
        &mut self,
        changes: &AssetChanges,
        backend: &mut Backend,
        asset_server: &mut AssetServer,
    ) {
        if changes.contains(self.data.shaders.render_meshes_source) {
            let source = asset_server.get(self.data.shaders.render_meshes_source);
            self.data.shaders.render_meshes =
                backend.create_shader_module("render meshes shader", source.source());

            self.rebuild_steps(backend);
        }
    }

    pub fn render(
        &self,
        encoder: &mut CommandEncoder,
        render_commands: &[RenderMeshCommand],
        render_target: &RenderTarget,
    ) {
        let (color_attachment, depth_stencil_attachment) = render_target.render_pass_attachments();

        let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("render pass"),
            color_attachments: &[Some(color_attachment)],
            depth_stencil_attachment: Some(depth_stencil_attachment),
        });

        for step in &self.steps {
            render_pass.set_pipeline(&step.pipeline);
            render_pass.set_bind_group(0, &self.data.scene_bind_group, &[]);

            for render_command in render_commands {
                let RenderMeshCommand {
                    material_bind_group,
                    model_bind_group,
                    vertex_buffer,
                    index_buffer,
                    index_count,
                } = render_command;

                render_pass.set_bind_group(1, material_bind_group, &[]);
                render_pass.set_bind_group(2, model_bind_group, &[]);
                render_pass.set_vertex_buffer(0, vertex_buffer.slice(..));
                render_pass.set_index_buffer(index_buffer.slice(..), wgpu::IndexFormat::Uint32);
                render_pass.draw_indexed(0..*index_count, 0, 0..1);
            }
        }
    }

    fn rebuild_steps(&mut self, backend: &mut Backend) {
        self.steps = Self::build_steps(&self.data, backend);
    }

    fn build_steps(data: &Pipeline3dData, backend: &mut Backend) -> Vec<Step> {
        vec![build_render_meshes_step(data, backend)]
    }
}

pub struct PipelineLayouts {
    pub render_meshes: wgpu::PipelineLayout,
}

pub struct BindGroupLayouts {
    pub scene: wgpu::BindGroupLayout,
    pub material: wgpu::BindGroupLayout,
    pub model: wgpu::BindGroupLayout,
}

pub struct Shaders {
    pub render_meshes_source: Handle<ShaderSource>,
    pub render_meshes: wgpu::ShaderModule,
}

pub struct RenderMeshCommand<'a> {
    pub material_bind_group: &'a wgpu::BindGroup,
    pub model_bind_group: &'a wgpu::BindGroup,
    pub vertex_buffer: &'a wgpu::Buffer,
    pub index_buffer: &'a wgpu::Buffer,
    pub index_count: u32,
}

fn build_render_meshes_step(pipeline_data: &Pipeline3dData, backend: &mut Backend) -> Step {
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
                        format: pipeline_data.render_target_info.color_format,
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
                    format: pipeline_data.render_target_info.depth_format,
                    depth_write_enabled: true,
                    depth_compare: wgpu::CompareFunction::Less,
                    stencil: wgpu::StencilState::default(),
                    bias: wgpu::DepthBiasState::default(),
                }),
                multisample: wgpu::MultisampleState {
                    count: pipeline_data.render_target_info.sample_count,
                    mask: !0,
                    alpha_to_coverage_enabled: false,
                },
                multiview: None,
            }),
    }
}
