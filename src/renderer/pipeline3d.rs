use wgpu::CommandEncoder;

use crate::{arena::Handle, asset_server::AssetChanges, shader_source::ShaderSource, AssetServer};

use super::{
    backend::Backend,
    visual_server::{RenderTarget, RenderTargetInfo},
    Vertex,
};

pub struct Pipeline3d {
    pipelines: Pipelines,
    pub data: Pipeline3dData,
}

pub struct Pipeline3dData {
    scene_bind_group: wgpu::BindGroup,
    render_target_info: RenderTargetInfo,
    pipeline_layouts: PipelineLayouts,
    #[allow(unused)]
    pub bind_group_layouts: BindGroupLayouts,
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

        let render_light_shader_source_handle = asset_server
            .load_with_options::<ShaderSource>("src/renderer/shaders/render_mesh.wgsl", "LIGHTS");
        let render_light_shader_source = asset_server
            .get(render_light_shader_source_handle)
            .source()
            .to_string();

        let render_shadow_map_shader_source_handle =
            asset_server.load::<ShaderSource>("src/renderer/shaders/render_shadow_map.wgsl");
        let render_shadow_map_shader_source =
            asset_server.get(render_shadow_map_shader_source_handle);

        let shaders = Shaders {
            render_mesh_source: render_mesh_shader_source_handle,
            render_mesh: backend
                .create_shader_module("render mesh shader", &render_mesh_shader_source),
            render_light_source: render_light_shader_source_handle,
            render_light: backend
                .create_shader_module("render light shader", &render_light_shader_source),
            render_shadow_map_source: render_shadow_map_shader_source_handle,
            render_shadow_map: backend.create_shader_module(
                "render shadow map shader",
                render_shadow_map_shader_source.source(),
            ),
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
                            visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
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
            directional_shadow_map: backend.device.create_pipeline_layout(
                &wgpu::PipelineLayoutDescriptor {
                    label: Some("directional shadow map pipeline layout"),
                    bind_group_layouts: &[&bind_group_layouts.scene, &bind_group_layouts.model],
                    push_constant_ranges: &[],
                },
            ),
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

        if changes.contains(self.data.shaders.render_shadow_map_source) {
            let source = asset_server.get(self.data.shaders.render_shadow_map_source);
            self.data.shaders.render_shadow_map =
                backend.create_shader_module("render shadow map shader", source.source());

            self.rebuild_pipelines(backend);
        }
    }

    pub fn render(
        &self,
        encoder: &mut CommandEncoder,
        render_commands: &RenderCommands,
        render_target: &RenderTarget,
    ) {
        // Shadow maps
        for light in render_commands.lights {
            let depth_view = light.shadow_map.create_view(&Default::default());
            let depth_stencil_attachment = wgpu::RenderPassDepthStencilAttachment {
                view: &depth_view,
                depth_ops: Some(wgpu::Operations {
                    load: wgpu::LoadOp::Clear(1.0),
                    store: true,
                }),
                stencil_ops: None,
            };
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("shadow map render pass"),
                color_attachments: &[],
                depth_stencil_attachment: Some(depth_stencil_attachment),
            });

            render_pass.set_pipeline(&self.pipelines.directional_shadow_map);
            render_pass.set_bind_group(0, &light.shadow_map_scene_bind_group, &[]);

            for mesh in render_commands.meshes {
                let RenderCommandMesh {
                    model_bind_group,
                    vertex_buffer,
                    index_buffer,
                    index_count,
                    ..
                } = mesh;

                render_pass.set_bind_group(1, model_bind_group, &[]);
                render_pass.set_vertex_buffer(0, vertex_buffer.slice(..));
                render_pass.set_index_buffer(index_buffer.slice(..), wgpu::IndexFormat::Uint32);
                render_pass.draw_indexed(0..*index_count, 0, 0..1);
            }
        }

        //## ACTUAL RENDERING DOWN HERE
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
            directional_shadow_map: build_pipeline_directional_shadow_map(data, backend),
        }
    }
}

struct PipelineLayouts {
    pub ambient_light_depth_prepass: wgpu::PipelineLayout,
    pub light: wgpu::PipelineLayout,
    pub directional_shadow_map: wgpu::PipelineLayout,
}

struct Pipelines {
    pub ambient_light_depth_prepass: wgpu::RenderPipeline,
    pub light: wgpu::RenderPipeline,
    pub directional_shadow_map: wgpu::RenderPipeline,
}

pub struct BindGroupLayouts {
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
    pub render_shadow_map_source: Handle<ShaderSource>,
    pub render_shadow_map: wgpu::ShaderModule,
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
    pub shadow_map_scene_bind_group: &'a wgpu::BindGroup,
    pub shadow_map: &'a wgpu::Texture,
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

fn build_pipeline_directional_shadow_map(
    pipeline_data: &Pipeline3dData,
    backend: &mut Backend,
) -> wgpu::RenderPipeline {
    backend
        .device
        .create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("directional shadow map render pipeline"),
            layout: Some(&pipeline_data.pipeline_layouts.directional_shadow_map),
            vertex: wgpu::VertexState {
                module: &pipeline_data.shaders.render_shadow_map,
                entry_point: "vs_main",
                buffers: &[Vertex::buffer_layout()],
            },
            fragment: None,
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                strip_index_format: None,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: Some(wgpu::Face::Back),
                ..Default::default()
            },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: Backend::DEPTH_TEXTURE_FORMAT,
                depth_write_enabled: true,
                depth_compare: wgpu::CompareFunction::Less,
                stencil: wgpu::StencilState::default(),
                bias: wgpu::DepthBiasState::default(),
            }),
            multisample: wgpu::MultisampleState {
                count: 1,
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
