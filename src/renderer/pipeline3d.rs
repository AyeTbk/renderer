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
    pipelines: Pipelines,
    data: Pipeline3dData,
}

pub struct Pipeline3dData {
    scene_bind_group: wgpu::BindGroup,
    render_target_info: RenderTargetInfo,
    pipeline_layouts: PipelineLayouts,
    #[allow(unused)]
    bind_group_layouts: BindGroupLayouts,
    shaders: Shaders,
}

impl Pipeline3d {
    pub fn new(
        scene_uniform_buffer: &wgpu::Buffer,
        render_target_info: RenderTargetInfo,
        backend: &mut Backend,
        asset_server: &mut AssetServer,
    ) -> Self {
        let render_mesh_shader_source_handle =
            asset_server.load::<ShaderSource>("src/renderer/shaders/render_mesh.wgsl");
        let render_mesh_shader_source = asset_server
            .get(render_mesh_shader_source_handle)
            .source()
            .to_string();
        let render_light_shader_source_handle =
            asset_server.load::<ShaderSource>("src/renderer/shaders/render_light.wgsl");
        let render_light_shader_source = asset_server.get(render_light_shader_source_handle);
        let shaders = Shaders {
            render_mesh_source: render_mesh_shader_source_handle,
            render_mesh: backend
                .create_shader_module("render mesh shader", &render_mesh_shader_source),
            render_light_source: render_light_shader_source_handle,
            render_light: backend
                .create_shader_module("render light shader", render_light_shader_source.source()),
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
            light: backend
                .device
                .create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                    label: Some("light bind group layout"),
                    entries: &[wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::FRAGMENT,
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
            ambient_light_depth_prepass: backend.device.create_pipeline_layout(
                &wgpu::PipelineLayoutDescriptor {
                    label: Some("ambient_light_depth_prepass pipeline layout"),
                    bind_group_layouts: &[
                        &bind_group_layouts.scene,
                        &bind_group_layouts.material,
                        &bind_group_layouts.model,
                    ],
                    push_constant_ranges: &[],
                },
            ),
            light: backend
                .device
                .create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                    label: Some("light pipeline layout"),
                    bind_group_layouts: &[
                        &bind_group_layouts.scene,
                        &bind_group_layouts.material,
                        &bind_group_layouts.model,
                        &bind_group_layouts.light,
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

        let pipelines = Self::build_pipelines(&data, backend);

        Self { pipelines, data }
    }

    pub fn update_render_target_info(
        &mut self,
        render_target_info: RenderTargetInfo,
        backend: &mut Backend,
    ) {
        self.data.render_target_info = render_target_info;
        self.rebuild_pipelines(backend);
    }

    pub fn notify_asset_changes(
        &mut self,
        changes: &AssetChanges,
        backend: &mut Backend,
        asset_server: &mut AssetServer,
    ) {
        if changes.contains(self.data.shaders.render_mesh_source) {
            let source = asset_server.get(self.data.shaders.render_mesh_source);
            self.data.shaders.render_mesh =
                backend.create_shader_module("render mesh shader", source.source());

            self.rebuild_pipelines(backend);
        }

        if changes.contains(self.data.shaders.render_light_source) {
            let source = asset_server.get(self.data.shaders.render_light_source);
            self.data.shaders.render_light =
                backend.create_shader_module("render light shader", source.source());

            self.rebuild_pipelines(backend);
        }
    }

    pub fn render(
        &self,
        encoder: &mut CommandEncoder,
        render_commands: &RenderCommands,
        render_target: &RenderTarget,
    ) {
        let (color_attachment, depth_stencil_attachment) = render_target.render_pass_attachments();

        let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("render pass"),
            color_attachments: &[Some(color_attachment)],
            depth_stencil_attachment: Some(depth_stencil_attachment),
        });

        // Ambient and depth
        render_pass.set_pipeline(&self.pipelines.ambient_light_depth_prepass);
        render_pass.set_bind_group(0, &self.data.scene_bind_group, &[]);

        for mesh in render_commands.meshes {
            let RenderCommandMesh {
                material_bind_group,
                model_bind_group,
                vertex_buffer,
                index_buffer,
                index_count,
            } = mesh;

            render_pass.set_bind_group(1, material_bind_group, &[]);
            render_pass.set_bind_group(2, model_bind_group, &[]);
            render_pass.set_vertex_buffer(0, vertex_buffer.slice(..));
            render_pass.set_index_buffer(index_buffer.slice(..), wgpu::IndexFormat::Uint32);
            render_pass.draw_indexed(0..*index_count, 0, 0..1);
        }

        // Lights
        render_pass.set_pipeline(&self.pipelines.light);

        for mesh in render_commands.meshes {
            let RenderCommandMesh {
                material_bind_group,
                model_bind_group,
                vertex_buffer,
                index_buffer,
                index_count,
            } = mesh;

            render_pass.set_bind_group(1, material_bind_group, &[]);
            render_pass.set_bind_group(2, model_bind_group, &[]);
            render_pass.set_vertex_buffer(0, vertex_buffer.slice(..));
            render_pass.set_index_buffer(index_buffer.slice(..), wgpu::IndexFormat::Uint32);

            for light in render_commands.lights {
                render_pass.set_bind_group(3, light.bind_group, &[]);
                render_pass.draw_indexed(0..*index_count, 0, 0..1);
            }
        }
    }

    fn rebuild_pipelines(&mut self, backend: &mut Backend) {
        self.pipelines = Self::build_pipelines(&self.data, backend);
    }

    fn build_pipelines(data: &Pipeline3dData, backend: &mut Backend) -> Pipelines {
        Pipelines {
            ambient_light_depth_prepass: build_pipeline_ambient_light_depth_prepass(data, backend),
            light: build_pipeline_light(data, backend),
        }
    }
}

struct PipelineLayouts {
    pub ambient_light_depth_prepass: wgpu::PipelineLayout,
    pub light: wgpu::PipelineLayout,
}

struct Pipelines {
    pub ambient_light_depth_prepass: wgpu::RenderPipeline,
    pub light: wgpu::RenderPipeline,
}

struct BindGroupLayouts {
    pub scene: wgpu::BindGroupLayout,
    pub material: wgpu::BindGroupLayout,
    pub model: wgpu::BindGroupLayout,
    pub light: wgpu::BindGroupLayout,
}

struct Shaders {
    pub render_mesh_source: Handle<ShaderSource>,
    pub render_mesh: wgpu::ShaderModule,
    pub render_light_source: Handle<ShaderSource>,
    pub render_light: wgpu::ShaderModule,
}

pub struct RenderCommands<'a> {
    pub meshes: &'a [RenderCommandMesh<'a>],
    pub lights: &'a [RenderCommandLight<'a>],
}

pub struct RenderCommandMesh<'a> {
    pub material_bind_group: &'a wgpu::BindGroup,
    pub model_bind_group: &'a wgpu::BindGroup,
    pub vertex_buffer: &'a wgpu::Buffer,
    pub index_buffer: &'a wgpu::Buffer,
    pub index_count: u32,
}

pub struct RenderCommandLight<'a> {
    pub bind_group: &'a wgpu::BindGroup,
}

fn build_pipeline_ambient_light_depth_prepass(
    pipeline_data: &Pipeline3dData,
    backend: &mut Backend,
) -> wgpu::RenderPipeline {
    backend
        .device
        .create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("ambient_light_depth_prepass render pipeline"),
            layout: Some(&pipeline_data.pipeline_layouts.ambient_light_depth_prepass),
            vertex: wgpu::VertexState {
                module: &pipeline_data.shaders.render_mesh,
                entry_point: "vs_main",
                buffers: &[Vertex::buffer_layout()],
            },
            fragment: Some(wgpu::FragmentState {
                module: &pipeline_data.shaders.render_mesh,
                entry_point: "fs_main_ambient_light_depth_prepass",
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
        })
}

fn build_pipeline_light(
    pipeline_data: &Pipeline3dData,
    backend: &mut Backend,
) -> wgpu::RenderPipeline {
    backend
        .device
        .create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("light render pipeline"),
            layout: Some(&pipeline_data.pipeline_layouts.light),
            vertex: wgpu::VertexState {
                module: &pipeline_data.shaders.render_mesh,
                entry_point: "vs_main",
                buffers: &[Vertex::buffer_layout()],
            },
            fragment: Some(wgpu::FragmentState {
                module: &pipeline_data.shaders.render_light,
                entry_point: "fs_main_blinn_phong",
                targets: &[Some(wgpu::ColorTargetState {
                    format: pipeline_data.render_target_info.color_format,
                    blend: Some(ADDITIVE_BLENDING),
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
                depth_write_enabled: false,
                depth_compare: wgpu::CompareFunction::Equal,
                stencil: wgpu::StencilState::default(),
                bias: wgpu::DepthBiasState::default(),
            }),
            multisample: wgpu::MultisampleState {
                count: pipeline_data.render_target_info.sample_count,
                mask: !0,
                alpha_to_coverage_enabled: false,
            },
            multiview: None,
        })
}

const ADDITIVE_BLENDING: wgpu::BlendState = {
    use wgpu::{BlendComponent, BlendFactor, BlendOperation, BlendState};
    BlendState {
        alpha: BlendComponent {
            src_factor: BlendFactor::Zero,
            dst_factor: BlendFactor::One,
            operation: BlendOperation::Add,
        },
        color: BlendComponent {
            src_factor: BlendFactor::One,
            dst_factor: BlendFactor::One,
            operation: BlendOperation::Add,
        },
    }
};
