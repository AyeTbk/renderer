use glam::Vec2;
use wgpu::CommandEncoder;

pub mod glyph_instance;
use crate::{arena::Handle, asset_server::AssetChanges, shader_source::ShaderSource, AssetServer};

use self::glyph_instance::GlyphInstance;

use super::{
    backend::Backend,
    visual_server::{RenderTarget, RenderTargetInfo},
};

pub struct Pipeline2d {
    render_text_pipeline: wgpu::RenderPipeline,
    render_fullscreen_texture_pipeline: wgpu::RenderPipeline,
    data: Pipeline2dData,
}

pub struct Pipeline2dData {
    pub viewport_bind_group: wgpu::BindGroup,
    pub instance_buffer: wgpu::Buffer,
    pub render_target_info: RenderTargetInfo,
    pub pipeline_layouts: PipelineLayouts,
    pub bind_group_layouts: BindGroupLayouts,
    pub shaders: Shaders,
    //
    pub font_texture_bind_group: wgpu::BindGroup,
    pub sampler_bilinear: wgpu::Sampler,
}

impl Pipeline2d {
    pub fn new(
        viewport_uniform_buffer: &wgpu::Buffer,
        font_texture: &wgpu::Texture,
        render_target_info: RenderTargetInfo,
        backend: &mut Backend,
        asset_server: &mut AssetServer,
    ) -> Self {
        let shader_source_handle =
            asset_server.load::<ShaderSource>("src/renderer/shaders/text.wgsl");
        let shader_source = asset_server.get(shader_source_handle).source().to_string();

        let render_fullscreen_texture_handle =
            asset_server.load::<ShaderSource>("src/renderer/shaders/fullscreen_texture.wgsl");
        let render_fullscreen_texture = asset_server
            .get(render_fullscreen_texture_handle)
            .source()
            .to_string();

        let shaders = Shaders {
            render_text_source: shader_source_handle,
            render_text: backend.create_shader_module("render text shader", &shader_source),
            render_fullscreen_texture_source: render_fullscreen_texture_handle,
            render_fullscreen_texture: backend.create_shader_module(
                "render fullscreen texture shader",
                &render_fullscreen_texture,
            ),
        };

        let bind_group_layouts = BindGroupLayouts {
            viewport: backend
                .device
                .create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                    label: Some("viewport bind group layout"),
                    entries: &[wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Uniform,
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    }],
                }),
            text_font: backend
                .device
                .create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                    label: Some("text font bind group layout"),
                    entries: &[
                        wgpu::BindGroupLayoutEntry {
                            binding: 0,
                            visibility: wgpu::ShaderStages::VERTEX_FRAGMENT, // FIXME fragment only plz
                            ty: wgpu::BindingType::Texture {
                                sample_type: wgpu::TextureSampleType::Float { filterable: true },
                                view_dimension: wgpu::TextureViewDimension::D2,
                                multisampled: false,
                            },
                            count: None,
                        },
                        wgpu::BindGroupLayoutEntry {
                            binding: 1,
                            visibility: wgpu::ShaderStages::FRAGMENT,
                            ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                            count: None,
                        },
                    ],
                }),
            fullscreen_texture: backend.device.create_bind_group_layout(
                &wgpu::BindGroupLayoutDescriptor {
                    label: Some("fullscreen texture bind group layout"),
                    entries: &[
                        wgpu::BindGroupLayoutEntry {
                            binding: 0,
                            visibility: wgpu::ShaderStages::FRAGMENT,
                            ty: wgpu::BindingType::Texture {
                                sample_type: wgpu::TextureSampleType::Float { filterable: false },
                                view_dimension: wgpu::TextureViewDimension::D2,
                                multisampled: false,
                            },
                            count: None,
                        },
                        wgpu::BindGroupLayoutEntry {
                            binding: 1,
                            visibility: wgpu::ShaderStages::FRAGMENT,
                            ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::NonFiltering),
                            count: None,
                        },
                    ],
                },
            ),
        };

        let pipeline_layouts = PipelineLayouts {
            text: backend
                .device
                .create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                    label: Some("2d pipeline layout"),
                    bind_group_layouts: &[
                        &bind_group_layouts.viewport,
                        &bind_group_layouts.text_font,
                    ],
                    push_constant_ranges: &[],
                }),
            fullscreen_texture: backend.device.create_pipeline_layout(
                &wgpu::PipelineLayoutDescriptor {
                    label: Some("fullscreen texture pipeline layout"),
                    bind_group_layouts: &[&bind_group_layouts.fullscreen_texture],
                    push_constant_ranges: &[],
                },
            ),
        };

        let viewport_bind_group = backend
            .device
            .create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("viewport bind group"),
                layout: &bind_group_layouts.viewport,
                entries: &[wgpu::BindGroupEntry {
                    binding: 0,
                    resource: viewport_uniform_buffer.as_entire_binding(),
                }],
            });

        let sampler_bilinear = backend.create_sampler();
        let font_texture_bind_group = Self::build_font_texture_bind_group(
            &bind_group_layouts,
            font_texture,
            &sampler_bilinear,
            backend,
        );

        let glyphs = b"Hello text!"
            .iter()
            .enumerate()
            .map(|(i, &id)| {
                GlyphInstance::new(
                    Vec2::new(i as f32 * 12.0 + 20.0, 20.0),
                    Vec2::new(14.0, 24.0),
                    id,
                )
            })
            .collect::<Vec<_>>();
        let instance_buffer = backend.create_vertex_buffer(&glyphs);

        let data = Pipeline2dData {
            viewport_bind_group,
            instance_buffer,
            render_target_info,
            pipeline_layouts,
            bind_group_layouts,
            shaders,
            //
            font_texture_bind_group,
            sampler_bilinear,
        };

        Self {
            render_text_pipeline: build_render_text_pipeline(&data, backend),
            render_fullscreen_texture_pipeline: build_render_fullscreen_texture_pipeline(
                &data, backend,
            ),
            data,
        }
    }

    pub fn update_render_target_info(
        &mut self,
        render_target_info: RenderTargetInfo,
        backend: &mut Backend,
    ) {
        self.data.render_target_info = render_target_info;

        self.rebuild_pipelines(backend);
    }

    pub fn update_font_texture(&mut self, font_texture: &wgpu::Texture, backend: &mut Backend) {
        self.data.font_texture_bind_group = Self::build_font_texture_bind_group(
            &self.data.bind_group_layouts,
            font_texture,
            &self.data.sampler_bilinear,
            backend,
        );
    }

    pub fn notify_asset_changes(
        &mut self,
        changes: &AssetChanges,
        backend: &mut Backend,
        asset_server: &mut AssetServer,
    ) {
        if changes.contains(self.data.shaders.render_text_source) {
            let source = asset_server.get(self.data.shaders.render_text_source);
            self.data.shaders.render_text =
                backend.create_shader_module("render text shader", source.source());

            self.rebuild_pipelines(backend);
        }
        if changes.contains(self.data.shaders.render_fullscreen_texture_source) {
            let source = asset_server.get(self.data.shaders.render_fullscreen_texture_source);
            self.data.shaders.render_fullscreen_texture =
                backend.create_shader_module("render fullscreen texture shader", source.source());

            self.rebuild_pipelines(backend);
        }
    }

    pub fn render(
        &self,
        encoder: &mut CommandEncoder,
        render_commands: &RenderCommands,
        render_target: &RenderTarget,
    ) {
        let (color_attachment, _depth_stencil_attachment) = render_target.render_pass_attachments();

        let color_attachment = wgpu::RenderPassColorAttachment {
            ops: wgpu::Operations {
                load: wgpu::LoadOp::Load,
                store: true,
            },
            ..color_attachment
        };

        let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("2d render pass"),
            color_attachments: &[Some(color_attachment)],
            depth_stencil_attachment: None,
        });

        if let Some(render_command) = render_commands.texture {
            render_pass.set_pipeline(&self.render_fullscreen_texture_pipeline);
            render_pass.set_bind_group(0, render_command.fullscreen_texture_bind_group, &[]);
            render_pass.draw(0..4, 0..1);
        }

        for render_command in render_commands.texts {
            render_pass.set_pipeline(&self.render_text_pipeline);
            render_pass.set_bind_group(0, &self.data.viewport_bind_group, &[]);
            render_pass.set_bind_group(1, &self.data.font_texture_bind_group, &[]);
            render_pass.set_vertex_buffer(0, render_command.instance_buffer.slice(..));
            render_pass.draw(0..4, 0..render_command.instance_count);
        }
    }

    fn rebuild_pipelines(&mut self, backend: &mut Backend) {
        self.render_text_pipeline = build_render_text_pipeline(&self.data, backend);
        self.render_fullscreen_texture_pipeline =
            build_render_fullscreen_texture_pipeline(&self.data, backend);
    }

    fn build_font_texture_bind_group(
        bind_group_layouts: &BindGroupLayouts,
        font_texture: &wgpu::Texture,
        sampler: &wgpu::Sampler,
        backend: &mut Backend,
    ) -> wgpu::BindGroup {
        let font_texture_view = font_texture.create_view(&Default::default());
        backend
            .device
            .create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("font texture bind group"),
                layout: &bind_group_layouts.text_font,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: wgpu::BindingResource::TextureView(&font_texture_view),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::Sampler(&sampler),
                    },
                ],
            })
    }

    pub fn build_fullscreen_texture_bind_group(
        &self,
        texture: &wgpu::Texture,
        sampler: &wgpu::Sampler,
        backend: &mut Backend,
    ) -> wgpu::BindGroup {
        let texture_view = texture.create_view(&Default::default());
        backend
            .device
            .create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("fullscreen texture bind group"),
                layout: &self.data.bind_group_layouts.fullscreen_texture,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: wgpu::BindingResource::TextureView(&texture_view),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::Sampler(&sampler),
                    },
                ],
            })
    }
}

pub struct PipelineLayouts {
    pub text: wgpu::PipelineLayout,
    pub fullscreen_texture: wgpu::PipelineLayout,
}

pub struct BindGroupLayouts {
    pub viewport: wgpu::BindGroupLayout,
    pub text_font: wgpu::BindGroupLayout,
    pub fullscreen_texture: wgpu::BindGroupLayout,
}

pub struct Shaders {
    pub render_text_source: Handle<ShaderSource>,
    pub render_text: wgpu::ShaderModule,
    pub render_fullscreen_texture_source: Handle<ShaderSource>,
    pub render_fullscreen_texture: wgpu::ShaderModule,
}

pub struct RenderCommands<'a> {
    pub texts: &'a [RenderTextCommand<'a>],
    pub texture: Option<&'a RenderFullscreenTextureCommand<'a>>,
}

pub struct RenderTextCommand<'a> {
    pub instance_buffer: &'a wgpu::Buffer,
    pub instance_count: u32,
}

pub struct RenderFullscreenTextureCommand<'a> {
    pub fullscreen_texture_bind_group: &'a wgpu::BindGroup,
}

fn build_render_text_pipeline(
    pipeline_data: &Pipeline2dData,
    backend: &mut Backend,
) -> wgpu::RenderPipeline {
    backend
        .device
        .create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("render text pipeline"),
            layout: Some(&pipeline_data.pipeline_layouts.text),
            vertex: wgpu::VertexState {
                module: &pipeline_data.shaders.render_text,
                entry_point: "vs_main",
                buffers: &[GlyphInstance::buffer_layout()],
            },
            fragment: Some(wgpu::FragmentState {
                module: &pipeline_data.shaders.render_text,
                entry_point: "fs_main",
                targets: &[Some(wgpu::ColorTargetState {
                    format: pipeline_data.render_target_info.color_format,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleStrip,
                strip_index_format: None,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: Some(wgpu::Face::Back),
                ..Default::default()
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState {
                count: pipeline_data.render_target_info.sample_count,
                mask: !0,
                alpha_to_coverage_enabled: false,
            },
            multiview: None,
        })
}

fn build_render_fullscreen_texture_pipeline(
    pipeline_data: &Pipeline2dData,
    backend: &mut Backend,
) -> wgpu::RenderPipeline {
    backend
        .device
        .create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("render fullscreen texture pipeline"),
            layout: Some(&pipeline_data.pipeline_layouts.fullscreen_texture),
            vertex: wgpu::VertexState {
                module: &pipeline_data.shaders.render_fullscreen_texture,
                entry_point: "vs_main",
                buffers: &[],
            },
            fragment: Some(wgpu::FragmentState {
                module: &pipeline_data.shaders.render_fullscreen_texture,
                entry_point: "fs_main",
                targets: &[Some(wgpu::ColorTargetState {
                    format: pipeline_data.render_target_info.color_format,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleStrip,
                strip_index_format: None,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: Some(wgpu::Face::Back),
                ..Default::default()
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState {
                count: pipeline_data.render_target_info.sample_count,
                mask: !0,
                alpha_to_coverage_enabled: false,
            },
            multiview: None,
        })
}
