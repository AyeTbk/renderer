use glam::{UVec2, Vec3};
use pollster::FutureExt;
use wgpu::{util::DeviceExt, BindGroupDescriptor};

use crate::{Color, Vertex};

const VERTICES: &[Vertex] = &[
    Vertex::new(Vec3::new(-1.0, 1.0, 0.0), Color::new_rgb(0.5, 0.0, 0.5)),
    Vertex::new(Vec3::new(-1.0, -1.0, 0.0), Color::new_rgb(0.5, 0.0, 0.0)),
    Vertex::new(Vec3::new(1.0, 1.0, 0.0), Color::new_rgb(0.0, 0.0, 0.5)),
    //Vertex::new(Vec3::new(1.0, -1.0, 0.0), Color::BLACK),
];

const INDICES: &[u16] = &[
    0, 1, 2, //
      // 1, 3, 2, //
];

pub struct Renderer {
    render_size: UVec2,
    clear_color: Color,
    //
    surface: wgpu::Surface,
    surface_config: wgpu::SurfaceConfiguration,
    device: wgpu::Device,
    queue: wgpu::Queue,
    //
    render_pipeline: wgpu::RenderPipeline,
    //
    camera_bind_group_layout: wgpu::BindGroupLayout,
    material_bind_group_layout: wgpu::BindGroupLayout,
}

impl Renderer {
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
        let camera_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("camera bind group layout"),
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
        let material_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("material bind group layout"),
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
            });

        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shaders/shader.wgsl").into()),
        });
        let render_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("render pipeline layout"),
                bind_group_layouts: &[&camera_bind_group_layout, &material_bind_group_layout],
                push_constant_ranges: &[],
            });
        let render_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("render pipeline"),
            layout: Some(&render_pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: "vs_main",
                buffers: &[Vertex::buffer_layout()],
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
                topology: wgpu::PrimitiveTopology::TriangleList,
                strip_index_format: None,
                front_face: wgpu::FrontFace::Ccw,
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
            clear_color: Color::GRUE,
            surface,
            surface_config,
            device,
            queue,
            render_pipeline,
            camera_bind_group_layout,
            material_bind_group_layout,
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

    pub fn clear_color(&mut self) -> Color {
        self.clear_color
    }

    pub fn set_clear_color(&mut self, clear_color: Color) {
        self.clear_color = clear_color;
    }

    pub fn create_vertex_buffer(&mut self, vertices: &[Vertex]) -> wgpu::Buffer {
        self.device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("vertex buffer"),
                contents: bytemuck::cast_slice(vertices),
                usage: wgpu::BufferUsages::VERTEX,
            })
    }

    pub fn create_index_buffer(&mut self, indices: &[u16]) -> wgpu::Buffer {
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

    pub fn create_material_bind_group(&mut self, uniform_buffer: &wgpu::Buffer) -> wgpu::BindGroup {
        self.device.create_bind_group(&BindGroupDescriptor {
            label: Some("material bind group"),
            layout: &self.material_bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: uniform_buffer.as_entire_binding(),
            }],
        })
    }

    pub fn render_meshes<'a>(
        &mut self,
        camera_uniform_buffer: &wgpu::Buffer,
        render_commands: impl Iterator<Item = RenderMeshCommand<'a>>,
        // material_uniform_buffer: &wgpu::Buffer,
        // vertex_buffer: &wgpu::Buffer,
        // index_buffer: &wgpu::Buffer,
        // index_count: u32,
    ) -> Result<(), wgpu::SurfaceError> {
        let surface_texture = self.surface.get_current_texture()?;
        let view = surface_texture
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());

        let camera_bind_group = self.device.create_bind_group(&BindGroupDescriptor {
            label: Some("camera bind group"),
            layout: &self.camera_bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: camera_uniform_buffer.as_entire_binding(),
            }],
        });

        // The encoder helps build a command buffer to be sent to the gpu.
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("render encoder"),
            });

        {
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("render pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(self.clear_color.to_wgpu()),
                        store: true,
                    },
                })],
                depth_stencil_attachment: None,
            });

            render_pass.set_pipeline(&self.render_pipeline);
            render_pass.set_bind_group(0, &camera_bind_group, &[]);

            for render_command in render_commands {
                let RenderMeshCommand {
                    material_bind_group,
                    vertex_buffer,
                    index_buffer,
                    index_count,
                } = render_command;

                render_pass.set_bind_group(1, material_bind_group, &[]);
                render_pass.set_vertex_buffer(0, vertex_buffer.slice(..));
                render_pass.set_index_buffer(index_buffer.slice(..), wgpu::IndexFormat::Uint16);
                render_pass.draw_indexed(0..index_count, 0, 0..1);
            }
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
    pub vertex_buffer: &'a wgpu::Buffer,
    pub index_buffer: &'a wgpu::Buffer,
    pub index_count: u32,
}
