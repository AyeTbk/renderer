use glam::UVec2;
use pollster::FutureExt;
use wgpu::{util::DeviceExt, BindGroupDescriptor};

use crate::{Color, Vertex};

pub struct Backend {
    render_size: UVec2,
    //
    pub surface: wgpu::Surface,
    pub surface_config: wgpu::SurfaceConfiguration,
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
    //
    show_texture_pipeline: wgpu::RenderPipeline,
    show_texture_bind_group_layout: wgpu::BindGroupLayout,
    //
    material_bind_group_layout: wgpu::BindGroupLayout,
    model_bind_group_layout: wgpu::BindGroupLayout,
}

impl Backend {
    pub const DEPTH_TEXTURE_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Depth32Float;

    pub fn new(window: &winit::window::Window) -> Self {
        let _ = env_logger::try_init();

        let render_size: UVec2 = (window.inner_size().width, window.inner_size().height).into();

        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::default());
        // # Safety
        // The surface must not outlive the window that created it.
        let surface = unsafe { instance.create_surface(window) }.unwrap();

        // An adapter represents an actual GPUxRendererAPI combo.
        let adapter: wgpu::Adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                compatible_surface: Some(&surface),
                ..Default::default()
            })
            .block_on()
            .unwrap();

        // A device represents a logical graphics/compute device.
        // A queue is a handle to a command queue for a device, to which commands can be submitted.
        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor::default(), None)
            .block_on()
            .unwrap();

        let surface_capabilities = surface.get_capabilities(&adapter);
        let surface_format = surface_capabilities
            .formats
            .iter()
            .copied()
            .find(|f| f.is_srgb())
            .unwrap_or(surface_capabilities.formats[0]);

        // A surface config is used to define how to create the surface's SurfaceTexture.
        let surface_config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format: surface_format,
            width: render_size.x,
            height: render_size.y,
            present_mode: wgpu::PresentMode::AutoVsync, // surface_capabilities.present_modes[0],
            alpha_mode: surface_capabilities.alpha_modes[0],
            view_formats: vec![],
        };

        surface.configure(&device, &surface_config);

        // Render pipeline stuff
        let material_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
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
            });
        let model_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
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
            });

        let show_texture_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("show texture bind group layout"),
                entries: &[
                    wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::FRAGMENT,
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
            });

        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("show texture shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shaders/show_texture.wgsl").into()),
        });
        let show_texture_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("show texture pipeline layout"),
                bind_group_layouts: &[&show_texture_bind_group_layout],
                push_constant_ranges: &[],
            });
        let show_texture_pipeline =
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("show texture pipeline"),
                layout: Some(&show_texture_pipeline_layout),
                vertex: wgpu::VertexState {
                    module: &shader,
                    entry_point: "vs_main",
                    buffers: &[],
                },
                fragment: Some(wgpu::FragmentState {
                    module: &shader,
                    entry_point: "fs_main",
                    targets: &[Some(wgpu::ColorTargetState {
                        format: surface_config.format,
                        blend: Some(wgpu::BlendState::REPLACE),
                        write_mask: wgpu::ColorWrites::ALL,
                    })],
                }),
                primitive: wgpu::PrimitiveState {
                    topology: wgpu::PrimitiveTopology::TriangleStrip,
                    strip_index_format: None,
                    front_face: wgpu::FrontFace::Cw,
                    cull_mode: Some(wgpu::Face::Back),
                    ..Default::default()
                },
                depth_stencil: None,
                multisample: wgpu::MultisampleState {
                    count: 1,
                    mask: !0,
                    alpha_to_coverage_enabled: false,
                },
                multiview: None,
            });

        Self {
            render_size,
            surface,
            surface_config,
            device,
            queue,
            show_texture_pipeline,
            show_texture_bind_group_layout,
            material_bind_group_layout,
            model_bind_group_layout,
        }
    }

    pub fn render_size(&self) -> UVec2 {
        self.render_size
    }

    pub fn set_render_size(&mut self, render_size: UVec2) {
        if render_size.x == 0 || render_size.y == 0 {
            return;
        }

        self.render_size = render_size;
        self.surface_config.width = render_size.x;
        self.surface_config.height = render_size.y;
        self.surface.configure(&self.device, &self.surface_config);
    }

    pub fn create_vertex_buffer(&mut self, vertices: &[Vertex]) -> wgpu::Buffer {
        self.device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("vertex buffer"),
                contents: bytemuck::cast_slice(vertices),
                usage: wgpu::BufferUsages::VERTEX,
            })
    }

    pub fn create_index_buffer(&mut self, indices: &[u32]) -> wgpu::Buffer {
        self.device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("vertex buffer"),
                contents: bytemuck::cast_slice(indices),
                usage: wgpu::BufferUsages::INDEX,
            })
    }

    pub fn create_uniform_buffer(&mut self, uniform: impl Uniform) -> wgpu::Buffer {
        self.device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("uniform buffer"),
                contents: bytemuck::cast_slice(&[uniform]),
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            })
    }

    pub fn update_uniform_buffer(&mut self, buffer: &wgpu::Buffer, uniform: impl Uniform) {
        self.queue
            .write_buffer(buffer, 0, bytemuck::cast_slice(&[uniform]));
    }

    pub fn create_material_bind_group(
        &mut self,
        uniform_buffer: &wgpu::Buffer,
        base_color_texture: &wgpu::Texture,
        sampler: &wgpu::Sampler,
    ) -> wgpu::BindGroup {
        let base_color_texture_view = base_color_texture.create_view(&Default::default());
        self.device.create_bind_group(&BindGroupDescriptor {
            label: Some("material bind group"),
            layout: &self.material_bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: uniform_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(&base_color_texture_view),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::Sampler(sampler),
                },
            ],
        })
    }

    pub fn create_model_bind_group(&mut self, uniform_buffer: &wgpu::Buffer) -> wgpu::BindGroup {
        self.device.create_bind_group(&BindGroupDescriptor {
            label: Some("model bind group"),
            layout: &self.model_bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: uniform_buffer.as_entire_binding(),
            }],
        })
    }

    pub fn create_color_texture(
        &mut self,
        width: u32,
        height: u32,
        data: &[u8],
        mip_level_count: u32,
    ) -> wgpu::Texture {
        self.device.create_texture_with_data(
            &self.queue,
            &wgpu::TextureDescriptor {
                label: Some("color texture"),
                size: wgpu::Extent3d {
                    width,
                    height,
                    depth_or_array_layers: 1,
                },
                mip_level_count,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::Rgba8UnormSrgb,
                usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
                view_formats: &[],
            },
            data,
        )
    }

    pub fn create_sampler(&mut self) -> wgpu::Sampler {
        self.device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("sampler"),
            address_mode_u: wgpu::AddressMode::Repeat,
            address_mode_v: wgpu::AddressMode::Repeat,
            address_mode_w: wgpu::AddressMode::Repeat,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::FilterMode::Linear,
            anisotropy_clamp: 16,
            ..Default::default()
        })
    }

    pub fn create_nearest_sampler(&mut self) -> wgpu::Sampler {
        self.device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("nearest sampler"),
            address_mode_u: wgpu::AddressMode::Repeat,
            address_mode_v: wgpu::AddressMode::Repeat,
            address_mode_w: wgpu::AddressMode::Repeat,
            mag_filter: wgpu::FilterMode::Nearest,
            min_filter: wgpu::FilterMode::Nearest,
            mipmap_filter: wgpu::FilterMode::Nearest,
            anisotropy_clamp: 1,
            ..Default::default()
        })
    }

    pub fn render_texture(&mut self, texture: &wgpu::Texture) -> Result<(), wgpu::SurfaceError> {
        let surface_texture = self.surface.get_current_texture()?;
        let surface_view = surface_texture.texture.create_view(&Default::default());

        let texture_view = texture.create_view(&Default::default());
        let sampler = self.create_nearest_sampler();

        let bind_group = self.device.create_bind_group(&BindGroupDescriptor {
            label: Some("show texture bind group"),
            layout: &self.show_texture_bind_group_layout,
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
        });

        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("show texture render encoder"),
            });

        {
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("show texture render pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &surface_view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(Color::RED.to_wgpu()),
                        store: true,
                    },
                })],
                depth_stencil_attachment: None,
            });

            render_pass.set_pipeline(&self.show_texture_pipeline);
            render_pass.set_bind_group(0, &bind_group, &[]);
            render_pass.draw(0..4, 0..1);
        }

        self.queue.submit(Some(encoder.finish()));

        surface_texture.present();

        Ok(())
    }
}

pub trait Uniform: Clone + Copy + bytemuck::Pod + bytemuck::Zeroable {}
impl<T> Uniform for T where T: Clone + Copy + bytemuck::Pod + bytemuck::Zeroable {}

pub struct RenderMeshCommand<'a> {
    pub material_bind_group: &'a wgpu::BindGroup,
    pub model_bind_group: &'a wgpu::BindGroup,
    pub vertex_buffer: &'a wgpu::Buffer,
    pub index_buffer: &'a wgpu::Buffer,
    pub index_count: u32,
}
