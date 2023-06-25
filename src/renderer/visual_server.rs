use std::collections::{hash_map::Entry, HashMap};

use glam::{Affine3A, Mat4, UVec2, Vec2, Vec3, Vec3Swizzles, Vec4};

// TODO Find ways to reduce coupling between the renderer and the rest of the engine, to
// eventually make it easy to extract in a separate crate (mostly in hopes of getting
// better compile times). This goes for AssetServer too, I guess.
use crate::{
    arena::Handle, asset_server::AssetChanges, image::Image, scene::UniqueNodeId, AssetServer,
    Camera, Color, Material, Mesh,
};

use super::{
    backend::Backend,
    pipeline2d::{glyph_instance::GlyphInstance, Pipeline2d, RenderTextCommand},
    pipeline3d::{Pipeline3d, RenderMeshCommand},
};

pub struct VisualServer {
    backend: Backend,
    render_size_factor: f32,
    //
    viewport_uniform_buffer: wgpu::Buffer,
    render_scene: RenderScene,
    render_scene_data: RenderSceneData,
    white_texture: wgpu::Texture,
    font_texture: wgpu::Texture,
    font_handle: Option<Handle<Image>>,
    //
    render_target: RenderTarget,
    pipeline3d: Pipeline3d,
    pipeline2d: Pipeline2d,
}

impl VisualServer {
    pub fn new(window: &winit::window::Window, asset_server: &mut AssetServer) -> Self {
        let mut backend = Backend::new(window);

        let viewport_uniform = ViewportUniform {
            size: backend.render_size().to_array(),
        };
        let viewport_uniform_buffer = backend.create_uniform_buffer(viewport_uniform);

        let scene_uniform = SceneUniform {
            projection_view: Camera::default().projection_matrix().to_cols_array(),
            view_pos: Vec4::default().to_array(),
            ambient_light: Color::new(0.3, 0.5, 0.9, 0.05).to_array(),
            sun_color: Color::new(1.0, 0.9, 0.8, 1.0).to_array(),
            sun_direction: Vec3::new(0.1, -1.0, 0.4)
                .normalize_or_zero()
                .xyzz()
                .to_array(),
        };
        let render_scene_data = RenderSceneData {
            uniform: scene_uniform,
            uniform_buffer: backend.create_uniform_buffer(scene_uniform),
        };

        let white_texture = backend.create_color_texture(1, 1, &[255, 255, 255, 255], 1);
        let font_texture = backend.create_color_texture(1, 1, &[255, 255, 0, 255], 1);

        let render_target = create_render_target(
            backend.render_size(),
            1,
            wgpu::TextureFormat::Rgba8UnormSrgb,
            Backend::DEPTH_TEXTURE_FORMAT,
            &mut backend,
        );

        let pipeline3d = Pipeline3d::new(
            &render_scene_data.uniform_buffer,
            render_target.info(),
            &mut backend,
            asset_server,
        );

        let pipeline2d = Pipeline2d::new(
            &viewport_uniform_buffer,
            &font_texture,
            render_target.info(),
            &mut backend,
            asset_server,
        );

        Self {
            backend,
            render_size_factor: 1.0,
            //
            viewport_uniform_buffer,
            render_scene: Default::default(),
            render_scene_data,
            white_texture,
            font_texture,
            font_handle: None,
            //
            render_target,
            pipeline3d,
            pipeline2d,
        }
    }

    pub fn render_size(&self) -> UVec2 {
        self.backend.render_size()
    }

    pub fn set_render_size(&mut self, render_size: UVec2) {
        self.backend.set_render_size(render_size);

        self.recreate_render_target();
    }

    pub fn set_render_size_factor(&mut self, factor: f32) {
        self.render_size_factor = factor;

        self.recreate_render_target();
    }

    pub fn set_msaa(&mut self, sample_count: u32) {
        self.render_target.sample_count = sample_count;

        self.recreate_render_target();
    }

    pub fn set_font_image(&mut self, handle: Handle<Image>, asset_server: &AssetServer) {
        self.font_handle = Some(handle);
        let image = asset_server.get(handle);
        self.font_texture = self.backend.create_color_texture_linear(
            image.width(),
            image.height(),
            image.data(),
            1,
        );

        self.pipeline2d
            .update_font_texture(&self.font_texture, &mut self.backend);
    }

    pub fn set_camera(&mut self, transform: &Affine3A, camera: &Camera) {
        self.render_scene_data.uniform.projection_view =
            (camera.projection_matrix() * transform.inverse()).to_cols_array();
        self.render_scene_data.uniform.view_pos = transform.translation.xyzz().to_array();

        self.backend.update_uniform_buffer(
            &self.render_scene_data.uniform_buffer,
            self.render_scene_data.uniform,
        );
    }

    pub fn render(&mut self) -> Result<(), wgpu::SurfaceError> {
        let mut render_mesh_commands = Vec::new();

        for mesh_instance in self.render_scene.mesh_instances.values() {
            let mesh = self.render_scene.meshes.get(&mesh_instance.mesh).unwrap();

            for submesh in &mesh.submeshes {
                let material = self.render_scene.materials.get(&submesh.material).unwrap();
                render_mesh_commands.push(RenderMeshCommand {
                    material_bind_group: &material.bind_group,
                    model_bind_group: &mesh_instance.model_bind_group,
                    vertex_buffer: &submesh.vertex_buffer,
                    index_buffer: &submesh.index_buffer,
                    index_count: submesh.index_count,
                });
            }
        }

        let mut encoder =
            self.backend
                .device
                .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                    label: Some("render encoder"),
                });

        self.pipeline3d
            .render(&mut encoder, &render_mesh_commands, &self.render_target);

        let mut render_text_commands = Vec::new();
        for text in self.render_scene.texts.values() {
            render_text_commands.push(RenderTextCommand {
                instance_buffer: &text.instance_buffer,
                instance_count: text.instance_count,
            });
        }
        self.pipeline2d
            .render(&mut encoder, &render_text_commands, &self.render_target);

        // FIXME: Strive to minimise the amount of submits across the board / submit as much work as possible
        // to reduce overhead / wasted GPU cycles. Right now there is two submits, one here and one in backend,
        // where there could be only one.
        self.backend.queue.submit(Some(encoder.finish()));

        self.backend
            .render_texture(&self.render_target.output_color_texture())?;

        Ok(())
    }

    pub fn set_mesh_instance(
        &mut self,
        id: UniqueNodeId,
        transform: Affine3A,
        mesh_handle: Handle<Mesh>,
        asset_server: &AssetServer,
    ) {
        self.register_mesh_instance(id, transform, mesh_handle, asset_server);
    }

    pub fn set_text(&mut self, id: UniqueNodeId, transform: &Affine3A, text: &[u8], size: f32) {
        let offset = transform.translation.xy();
        let glyphs = text
            .iter()
            .enumerate()
            .map(|(i, &id)| {
                let id = id.min(127);
                GlyphInstance::new(
                    offset + Vec2::new(i as f32 * size * 0.5, 0.0),
                    Vec2::new(size * 1.1667 * 0.5, size),
                    id,
                )
            })
            .collect::<Vec<_>>();
        let instance_buffer = self.backend.create_vertex_buffer(&glyphs);

        self.render_scene.texts.insert(
            id,
            RenderText {
                instance_buffer,
                instance_count: glyphs.len() as u32,
            },
        );
    }

    pub fn reset_scene(&mut self) {
        self.render_scene = Default::default();
    }

    pub fn notify_asset_changes(&mut self, changes: &AssetChanges, asset_server: &mut AssetServer) {
        let mut textures_to_update = Vec::new();
        let mut materials_to_update = Vec::new();

        for changed_image_handle in changes.iter::<Image>() {
            if self
                .render_scene
                .textures
                .contains_key(&changed_image_handle)
            {
                textures_to_update.push(changed_image_handle);
            }

            for (&material_handle, material) in self.render_scene.materials.iter() {
                if material.used_textures.contains(&changed_image_handle) {
                    materials_to_update.push(material_handle);
                }
            }

            if self.font_handle == Some(changed_image_handle) {
                self.set_font_image(changed_image_handle, asset_server);
            }
        }

        for texture_handle in textures_to_update {
            self.update_texture(texture_handle, asset_server);
        }
        for material_handle in materials_to_update {
            self.update_render_material_data(material_handle, asset_server);
        }

        self.pipeline3d
            .notify_asset_changes(changes, &mut self.backend, asset_server);

        self.pipeline2d
            .notify_asset_changes(changes, &mut self.backend, asset_server);
    }

    fn recreate_render_target(&mut self) {
        let scaled_render_size =
            (self.render_size().as_vec2() * self.render_size_factor).as_uvec2();

        let info = self.render_target.info();
        self.render_target = create_render_target(
            scaled_render_size,
            info.sample_count,
            info.color_format,
            info.depth_format,
            &mut self.backend,
        );

        let viewport_uniform = ViewportUniform {
            size: self.backend.render_size().to_array(),
        };
        self.backend
            .update_uniform_buffer(&self.viewport_uniform_buffer, viewport_uniform);

        self.pipeline3d
            .update_render_target_info(self.render_target.info(), &mut self.backend);
        self.pipeline2d
            .update_render_target_info(self.render_target.info(), &mut self.backend);
    }

    fn register_mesh_instance(
        &mut self,
        id: UniqueNodeId,
        transform: Affine3A,
        handle: Handle<Mesh>,
        asset_server: &AssetServer,
    ) {
        self.register_mesh(handle, asset_server);

        let model_uniform = ModelUniform {
            transform: Mat4::from(transform).to_cols_array(),
        };
        let model_uniform_buffer = self.backend.create_uniform_buffer(model_uniform);
        let model_bind_group = self.backend.create_model_bind_group(&model_uniform_buffer);

        self.render_scene.mesh_instances.insert(
            id,
            RenderMeshInstance {
                model_uniform_buffer,
                model_bind_group,
                mesh: handle,
            },
        );
    }

    fn register_mesh(&mut self, handle: Handle<Mesh>, asset_server: &AssetServer) {
        let mut materials_to_register = Vec::new();

        if let Entry::Vacant(e) = self.render_scene.meshes.entry(handle) {
            let mesh = asset_server.get(handle);

            let mut render_submeshes = Vec::new();
            for submesh in &mesh.submeshes {
                materials_to_register.push(submesh.material);

                render_submeshes.push(RenderSubmesh {
                    vertex_buffer: self.backend.create_vertex_buffer(&submesh.vertices),
                    index_buffer: self.backend.create_index_buffer(&submesh.indices),
                    index_count: submesh.indices.len() as u32,
                    material: submesh.material,
                })
            }
            let render_mesh = RenderMesh {
                submeshes: render_submeshes,
            };
            e.insert(render_mesh);
        }

        for material_handle in materials_to_register {
            self.register_material(material_handle, asset_server);
        }
    }

    fn register_material(&mut self, handle: Handle<Material>, asset_server: &AssetServer) {
        if self.render_scene.materials.contains_key(&handle) {
            return;
        }
        let material = asset_server.get(handle);

        if let Some(image) = material.base_color_image {
            self.register_texture(image, asset_server);
        }

        self.update_render_material_data(handle, asset_server);
    }

    fn update_render_material_data(
        &mut self,
        handle: Handle<Material>,
        asset_server: &AssetServer,
    ) {
        let material = asset_server.get(handle);
        let material_uniform = MaterialUniform {
            base_color: material.base_color.into(),
        };

        let uniform_buffer = self.backend.create_uniform_buffer(material_uniform);
        let base_color_texture = if let Some(image) = material.base_color_image {
            let texture = self.render_scene.textures.get(&image).unwrap();
            Some(texture)
        } else {
            None
        };
        let sampler = self.backend.create_sampler();

        let base_color_texture_ref = base_color_texture.unwrap_or(&self.white_texture);

        let bind_group = self.backend.create_material_bind_group(
            &uniform_buffer,
            base_color_texture_ref,
            &sampler,
        );
        let render_material = RenderMaterial {
            bind_group,
            uniform_buffer,
            used_textures: material.base_color_image.into_iter().collect(),
            sampler,
        };

        self.render_scene.materials.insert(handle, render_material);
    }

    fn register_texture(&mut self, handle: Handle<Image>, asset_server: &AssetServer) {
        if self.render_scene.textures.contains_key(&handle) {
            return;
        }

        self.update_texture(handle, asset_server);
    }

    fn update_texture(&mut self, handle: Handle<Image>, asset_server: &AssetServer) {
        let image = asset_server.get(handle);
        let texture = self.backend.create_color_texture(
            image.width(),
            image.height(),
            image.data(),
            image.mip_level_count(),
        );
        self.render_scene.textures.insert(handle, texture);
    }
}

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct ViewportUniform {
    size: [u32; 2],
}

struct RenderSceneData {
    uniform: SceneUniform,
    uniform_buffer: wgpu::Buffer,
}

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct SceneUniform {
    projection_view: [f32; 16],
    view_pos: [f32; 4],
    ambient_light: [f32; 4],
    sun_color: [f32; 4],
    sun_direction: [f32; 4],
}

#[derive(Default)]
struct RenderScene {
    meshes: HashMap<Handle<Mesh>, RenderMesh>,
    materials: HashMap<Handle<Material>, RenderMaterial>,
    textures: HashMap<Handle<Image>, wgpu::Texture>,
    mesh_instances: HashMap<UniqueNodeId, RenderMeshInstance>,
    texts: HashMap<UniqueNodeId, RenderText>,
}

struct RenderText {
    instance_buffer: wgpu::Buffer,
    instance_count: u32,
}

struct RenderMesh {
    submeshes: Vec<RenderSubmesh>,
}

struct RenderSubmesh {
    vertex_buffer: wgpu::Buffer,
    index_buffer: wgpu::Buffer,
    index_count: u32,
    material: Handle<Material>,
}

struct RenderMeshInstance {
    model_bind_group: wgpu::BindGroup,
    #[allow(unused)]
    model_uniform_buffer: wgpu::Buffer,
    mesh: Handle<Mesh>,
}

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct ModelUniform {
    transform: [f32; 16],
}

struct RenderMaterial {
    bind_group: wgpu::BindGroup,
    #[allow(unused)]
    uniform_buffer: wgpu::Buffer,
    #[allow(unused)]
    used_textures: Vec<Handle<Image>>,
    #[allow(unused)]
    sampler: wgpu::Sampler,
}

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct MaterialUniform {
    base_color: [f32; 4],
}

pub struct RenderTarget {
    pub size: UVec2,
    pub sample_count: u32,
    pub color_format: wgpu::TextureFormat,
    pub depth_format: wgpu::TextureFormat,
    pub texture: RenderTargetTexture,
}

pub enum RenderTargetTexture {
    Simple {
        color: wgpu::Texture,
        color_view: wgpu::TextureView,
        depth: wgpu::Texture,
        depth_view: wgpu::TextureView,
    },
    Multisampled {
        color: wgpu::Texture,
        color_view: wgpu::TextureView,
        depth: wgpu::Texture,
        depth_view: wgpu::TextureView,
        resolve: wgpu::Texture,
        resolve_view: wgpu::TextureView,
    },
}

pub struct RenderTargetInfo {
    pub sample_count: u32,
    pub color_format: wgpu::TextureFormat,
    pub depth_format: wgpu::TextureFormat,
}

impl RenderTarget {
    pub fn info(&self) -> RenderTargetInfo {
        RenderTargetInfo {
            sample_count: self.sample_count,
            color_format: self.color_format,
            depth_format: self.depth_format,
        }
    }

    pub fn output_color_texture(&self) -> &wgpu::Texture {
        match &self.texture {
            RenderTargetTexture::Simple { color, .. } => color,
            RenderTargetTexture::Multisampled { resolve, .. } => resolve,
        }
    }

    pub fn render_pass_attachments(
        &self,
    ) -> (
        wgpu::RenderPassColorAttachment,
        wgpu::RenderPassDepthStencilAttachment,
    ) {
        let (color_view, depth_view, resolve_view) = match &self.texture {
            RenderTargetTexture::Simple {
                color_view,
                depth_view,
                ..
            } => (color_view, depth_view, None),
            RenderTargetTexture::Multisampled {
                color_view,
                depth_view,
                resolve_view,
                ..
            } => (color_view, depth_view, Some(resolve_view)),
        };

        let color_attachment = wgpu::RenderPassColorAttachment {
            view: color_view,
            resolve_target: resolve_view,
            ops: wgpu::Operations {
                load: wgpu::LoadOp::Clear(Color::GRUE.to_wgpu()),
                store: true,
            },
        };

        let depth_stencil_attachment = wgpu::RenderPassDepthStencilAttachment {
            view: depth_view,
            depth_ops: Some(wgpu::Operations {
                load: wgpu::LoadOp::Clear(1.0),
                store: true,
            }),
            stencil_ops: None,
        };

        (color_attachment, depth_stencil_attachment)
    }
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
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });

        RenderTargetTexture::Multisampled {
            color_view: color.create_view(&Default::default()),
            color,
            depth_view: depth.create_view(&Default::default()),
            depth,
            resolve_view: resolve.create_view(&Default::default()),
            resolve,
        }
    } else {
        RenderTargetTexture::Simple {
            color_view: color.create_view(&Default::default()),
            color,
            depth_view: depth.create_view(&Default::default()),
            depth,
        }
    };

    RenderTarget {
        size,
        color_format,
        depth_format,
        sample_count,
        texture,
    }
}
