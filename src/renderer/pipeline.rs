use glam::UVec2;

use crate::{renderer::step::build_render_meshes_step, Color};

use super::{
    backend::{Backend, RenderMeshCommand},
    step::Step,
};

pub struct Pipeline {
    steps: Vec<Step>,
    data: PipelineData,
}

pub struct PipelineData {
    pub render_target: RenderTarget,
    pub pipeline_layouts: PipelineLayouts,
    pub bind_group_layouts: BindGroupLayouts,
    pub shaders: Shaders,
}

impl Pipeline {
    pub fn new(backend: &mut Backend) -> Self {
        let shaders = Shaders {
            render_meshes: backend
                .device
                .create_shader_module(wgpu::ShaderModuleDescriptor {
                    label: Some("render meshes shader"),
                    source: wgpu::ShaderSource::Wgsl(include_str!("shaders/shader.wgsl").into()),
                }),
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

        let render_target = Self::create_render_target(
            (1, 1).into(),
            1,
            wgpu::TextureFormat::Rgba8UnormSrgb,
            Backend::DEPTH_TEXTURE_FORMAT,
            backend,
        );

        let data = PipelineData {
            render_target,
            pipeline_layouts,
            bind_group_layouts,
            shaders,
        };
        let steps = Self::build_steps(&data, backend);

        Self { steps, data }
    }

    pub fn set_render_target_size(&mut self, size: UVec2, backend: &mut Backend) {
        self.data.render_target.size = size;

        self.recreate_render_target(backend);
    }

    pub fn set_render_target_sample_count(&mut self, sample_count: u32, backend: &mut Backend) {
        self.data.render_target.sample_count = sample_count;

        self.recreate_render_target(backend);
        self.rebuild_steps(backend);
    }

    pub fn render_target_texture(&self) -> &wgpu::Texture {
        match &self.data.render_target.texture {
            RenderTargetTexture::Simple { color, .. } => color,
            RenderTargetTexture::Multisampled { resolve, .. } => resolve,
        }
    }

    pub fn render<'a>(
        &self,
        scene_uniform_buffer: &wgpu::Buffer,
        render_commands: &[RenderMeshCommand],
        backend: &mut Backend,
    ) {
        let scene_bind_group = backend
            .device
            .create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("scene bind group"),
                layout: &self.data.bind_group_layouts.scene,
                entries: &[wgpu::BindGroupEntry {
                    binding: 0,
                    resource: scene_uniform_buffer.as_entire_binding(),
                }],
            });

        let (color_view, depth_view, resolve_view) = match &self.data.render_target.texture {
            RenderTargetTexture::Simple { color, depth } => (
                color.create_view(&Default::default()),
                depth.create_view(&Default::default()),
                None,
            ),
            RenderTargetTexture::Multisampled {
                color,
                depth,
                resolve,
            } => (
                color.create_view(&Default::default()),
                depth.create_view(&Default::default()),
                Some(resolve.create_view(&Default::default())),
            ),
        };

        let render_pass_color_attachment = wgpu::RenderPassColorAttachment {
            view: &color_view,
            resolve_target: resolve_view.as_ref(),
            ops: wgpu::Operations {
                load: wgpu::LoadOp::Clear(Color::GRUE.to_wgpu()),
                store: true,
            },
        };

        let render_pass_depth_stencil_attachment = wgpu::RenderPassDepthStencilAttachment {
            view: &depth_view,
            depth_ops: Some(wgpu::Operations {
                load: wgpu::LoadOp::Clear(1.0),
                store: true,
            }),
            stencil_ops: None,
        };

        let mut encoder = backend
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("render encoder"),
            });

        {
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("render pass"),
                color_attachments: &[Some(render_pass_color_attachment)],
                depth_stencil_attachment: Some(render_pass_depth_stencil_attachment),
            });

            for step in &self.steps {
                render_pass.set_pipeline(&step.pipeline);
                render_pass.set_bind_group(0, &scene_bind_group, &[]);

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

        backend.queue.submit(Some(encoder.finish()));
    }

    fn rebuild_steps(&mut self, backend: &mut Backend) {
        self.steps = Self::build_steps(&self.data, backend);
    }

    fn build_steps(data: &PipelineData, backend: &mut Backend) -> Vec<Step> {
        vec![build_render_meshes_step(data, backend)]
    }

    fn recreate_render_target(&mut self, backend: &mut Backend) {
        self.data.render_target = Self::create_render_target(
            self.data.render_target.size,
            self.data.render_target.sample_count,
            self.data.render_target.color_format,
            self.data.render_target.depth_format,
            backend,
        );
    }

    fn create_render_target(
        size: UVec2,
        sample_count: u32,
        color_format: wgpu::TextureFormat,
        depth_format: wgpu::TextureFormat,
        backend: &mut Backend,
    ) -> RenderTarget {
        let texture_size = wgpu::Extent3d {
            width: size.x,
            height: size.y,
            depth_or_array_layers: 1,
        };

        let color = backend.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("color texture"),
            size: texture_size,
            mip_level_count: 1,
            sample_count,
            dimension: wgpu::TextureDimension::D2,
            format: color_format,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });
        let depth = backend.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("depth texture"),
            size: texture_size,
            mip_level_count: 1,
            sample_count,
            dimension: wgpu::TextureDimension::D2,
            format: depth_format,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });

        let is_multisampled = sample_count > 1;
        let texture = if is_multisampled {
            let resolve = backend.device.create_texture(&wgpu::TextureDescriptor {
                label: Some("resolve texture"),
                size: texture_size,
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: color_format,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                    | wgpu::TextureUsages::TEXTURE_BINDING,
                view_formats: &[],
            });

            RenderTargetTexture::Multisampled {
                color,
                depth,
                resolve,
            }
        } else {
            RenderTargetTexture::Simple { color, depth }
        };

        RenderTarget {
            size,
            color_format,
            depth_format,
            sample_count,
            texture,
        }
    }
}

pub struct RenderTarget {
    pub size: UVec2,
    pub color_format: wgpu::TextureFormat,
    pub depth_format: wgpu::TextureFormat,
    pub sample_count: u32,
    pub texture: RenderTargetTexture,
}

pub enum RenderTargetTexture {
    Simple {
        color: wgpu::Texture,
        depth: wgpu::Texture,
    },
    Multisampled {
        color: wgpu::Texture,
        depth: wgpu::Texture,
        resolve: wgpu::Texture,
    },
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
    pub render_meshes: wgpu::ShaderModule,
}
