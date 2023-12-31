use std::collections::{hash_map::Entry, HashMap};

use glam::{Affine3A, Mat4, UVec2, Vec2, Vec3Swizzles};

// TODO Find ways to reduce coupling between the renderer and the rest of the engine, to
// eventually make it easy to extract in a separate crate (mostly in hopes of getting
// better compile times). This goes for AssetServer too, I guess.
use crate::{
    arena::Handle, asset_server::AssetChanges, image::Image, light::LightKind,
    material::BillboardMode, scene::UniqueNodeId, AssetServer, Camera, Color, Light, Material,
    Mesh,
};

use super::{
    backend::Backend,
    pipeline2d::{
        self, glyph_instance::GlyphInstance, Pipeline2d, RenderFullscreenTextureCommand,
        RenderTextCommand,
    },
    pipeline3d::{Pipeline3d, RenderCommandLight, RenderCommandMesh, RenderCommands},
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
    default_material: Option<Handle<Material>>,
    quad_mesh: Option<Handle<Mesh>>,
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
            projection: Camera::default().projection_matrix().to_cols_array(),
            view: Mat4::IDENTITY.to_cols_array(),
            camera_transform: Mat4::IDENTITY.to_cols_array(),
            ambient_light: Color::new(0.3, 0.5, 0.9, 0.04).to_array(),
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

        let mut this = Self {
            backend,
            render_size_factor: 1.0,
            //
            viewport_uniform_buffer,
            render_scene: Default::default(),
            render_scene_data,
            white_texture,
            font_texture,
            font_handle: None,
            quad_mesh: None,
            default_material: None,
            //
            render_target,
            pipeline3d,
            pipeline2d,
        };

        this.initialize_default_resources(asset_server);

        this
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
        self.render_scene_data.uniform.projection = camera.projection_matrix().to_cols_array();
        self.render_scene_data.uniform.view = Mat4::from(transform.inverse()).to_cols_array();
        self.render_scene_data.uniform.camera_transform = Mat4::from(*transform).to_cols_array();

        self.backend.update_uniform_buffer(
            &self.render_scene_data.uniform_buffer,
            self.render_scene_data.uniform,
        );
    }

    pub fn render(&mut self) -> Result<(), wgpu::SurfaceError> {
        let mut render_commands_meshes = Vec::new();

        for mesh_instance in self.render_scene.mesh_instances.values() {
            let mesh = self.render_scene.meshes.get(&mesh_instance.mesh).unwrap();

            for submesh in &mesh.submeshes {
                let material_handle = mesh_instance
                    .material_override
                    .as_ref()
                    .unwrap_or(&submesh.material);
                let material = self.render_scene.materials.get(material_handle).unwrap();
                render_commands_meshes.push(RenderCommandMesh {
                    material_bind_group: &material.bind_group,
                    model_bind_group: &mesh_instance.model_bind_group,
                    vertex_buffer: &submesh.vertex_buffer,
                    index_buffer: &submesh.index_buffer,
                    index_count: submesh.index_count,
                });
            }
        }

        let mut render_commands_lights = Vec::new();
        for light in self.render_scene.lights.values() {
            render_commands_lights.push(RenderCommandLight {
                bind_group: &light.bind_group,
                shadow_map_scene_bind_group: &light.shadow_map_scene_bind_group,
                shadow_map: &light.shadow_map,
            });
        }

        let commands = RenderCommands {
            meshes: &render_commands_meshes,
            lights: &render_commands_lights,
        };

        let mut encoder =
            self.backend
                .device
                .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                    label: Some("render encoder"),
                });

        self.pipeline3d
            .render(&mut encoder, &commands, &self.render_target);

        let mut render_text_commands = Vec::new();
        for text in self.render_scene.texts.values() {
            render_text_commands.push(RenderTextCommand {
                instance_buffer: &text.instance_buffer,
                instance_count: text.instance_count,
            });
        }

        let maybe_texture_command =
            if let Some(render_texture) = &self.render_scene.fullscreen_texture {
                Some(RenderFullscreenTextureCommand {
                    fullscreen_texture_bind_group: &render_texture.bind_group,
                })
            } else {
                None
            };
        let commands_2d = pipeline2d::RenderCommands {
            texts: &render_text_commands,
            texture: maybe_texture_command.as_ref(),
        };
        self.pipeline2d
            .render(&mut encoder, &commands_2d, &self.render_target);

        // FIXME: Strive to minimise the amount of submits across the board / submit as much work as possible
        // to reduce overhead / wasted GPU cycles. Right now there is two submits, one here and one in backend,
        // where there could be only one.
        self.backend.queue.submit(Some(encoder.finish()));

        self.backend
            .render_texture(&self.render_target.output_color_texture())?;

        Ok(())
    }

    pub fn set_depth_fullscreen_texture(&mut self) {
        // let Some(light) = self.render_scene.lights.get(&light_id) else {
        //     eprintln!("warning: {}:{}: no such light registered", file!(), line!());
        //     return;
        // };
        // let texture = &light.shadow_map;
        let texture = &self.render_target.texture.depth();
        let sampler = self.backend.create_non_filtering_sampler();
        let bind_group = self.pipeline2d.build_fullscreen_texture_bind_group(
            texture,
            &sampler,
            &mut self.backend,
        );
        self.render_scene.fullscreen_texture = Some(RenderFullscreenTexture {
            bind_group,
            sampler,
        });
    }

    pub fn set_shadow_map_fullscreen_texture(&mut self, light_id: UniqueNodeId) {
        let Some(light) = self.render_scene.lights.get(&light_id) else {
            eprintln!("warning: {}:{}: no such light registered", file!(), line!());
            return;
        };
        let texture = &light.shadow_map;
        let sampler = self.backend.create_non_filtering_sampler();
        let bind_group = self.pipeline2d.build_fullscreen_texture_bind_group(
            texture,
            &sampler,
            &mut self.backend,
        );
        self.render_scene.fullscreen_texture = Some(RenderFullscreenTexture {
            bind_group,
            sampler,
        });
    }

    pub fn unset_fullscreen_texture(&mut self) {
        self.render_scene.fullscreen_texture = None;
    }

    pub fn set_light(&mut self, id: UniqueNodeId, transform: Affine3A, light: &Light) {
        let kind = match &light.kind {
            LightKind::Directional { .. } => 0,
            LightKind::Point { .. } => 1,
        };

        let shadow_map = self
            .backend
            .device
            .create_texture(&wgpu::TextureDescriptor {
                label: Some("shadow map texture"),
                size: wgpu::Extent3d {
                    width: 1024,
                    height: 1024,
                    ..Default::default()
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: Backend::DEPTH_TEXTURE_FORMAT,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                    | wgpu::TextureUsages::TEXTURE_BINDING,
                view_formats: &[],
            });

        let ortho_size = 16.0;
        let shadow_map_projection = Mat4::orthographic_lh(
            -ortho_size,
            ortho_size,
            -ortho_size,
            ortho_size,
            0.05,
            100.0,
        );
        let shadow_map_view = Mat4::from(transform.inverse());
        let shadow_map_scene_uniform = SceneUniform {
            projection: shadow_map_projection.to_cols_array(),
            view: shadow_map_view.to_cols_array(),
            camera_transform: Mat4::from(transform).to_cols_array(),
            ambient_light: [0.0, 0.0, 0.0, 0.0],
        };
        let shadow_map_scene_uniform_buffer =
            self.backend.create_uniform_buffer(shadow_map_scene_uniform);
        let shadow_map_scene_bind_group =
            self.backend
                .device
                .create_bind_group(&wgpu::BindGroupDescriptor {
                    label: Some("scene bind group"),
                    layout: &self.pipeline3d.data.bind_group_layouts.scene,
                    entries: &[wgpu::BindGroupEntry {
                        binding: 0,
                        resource: shadow_map_scene_uniform_buffer.as_entire_binding(),
                    }],
                });

        let uniform = LightUniform {
            transform: Mat4::from(transform).to_cols_array(),
            world_to_light: Mat4::from(shadow_map_projection * shadow_map_view).to_cols_array(),
            color: light.color.to_array(),
            radius: light.radius().unwrap_or_default(),
            kind,
            _padding: Default::default(),
        };
        let uniform_buffer = self.backend.create_uniform_buffer(uniform);

        let sampler = self.backend.create_non_filtering_sampler();

        let bind_group = self.backend.create_light_bind_group(
            &uniform_buffer,
            &shadow_map,
            &sampler,
            &self.pipeline3d.data.bind_group_layouts.light,
        );

        self.render_scene.lights.insert(
            id,
            RenderLight {
                bind_group,
                uniform_buffer,
                sampler,
                shadow_map_scene_bind_group,
                shadow_map_scene_uniform_buffer,
                shadow_map,
            },
        );
    }

    pub fn set_mesh_instance(
        &mut self,
        id: UniqueNodeId,
        transform: Affine3A,
        mesh_handle: Handle<Mesh>,
        asset_server: &AssetServer,
    ) {
        self.register_mesh(mesh_handle, asset_server);

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
                mesh: mesh_handle,
                material_override: None,
            },
        );
    }

    pub fn set_sprite(
        &mut self,
        id: UniqueNodeId,
        transform: Affine3A,
        image_handle: Handle<Image>,
        base_color: Color,
        asset_server: &mut AssetServer,
    ) {
        let model_uniform = ModelUniform {
            transform: Mat4::from(transform).to_cols_array(),
        };
        let model_uniform_buffer = self.backend.create_uniform_buffer(model_uniform);
        let model_bind_group = self.backend.create_model_bind_group(&model_uniform_buffer);

        let material = asset_server.add(Material {
            base_color,
            base_color_image: Some(image_handle),
            billboard_mode: BillboardMode::On,
            unlit: true,
        });
        self.register_material(material, asset_server);

        self.render_scene.mesh_instances.insert(
            id,
            RenderMeshInstance {
                model_uniform_buffer,
                model_bind_group,
                mesh: self.quad_mesh.unwrap(),
                material_override: Some(material),
            },
        );
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

    fn register_mesh(&mut self, handle: Handle<Mesh>, asset_server: &AssetServer) {
        let mut materials_to_register = Vec::new();

        if let Entry::Vacant(e) = self.render_scene.meshes.entry(handle) {
            let mesh = asset_server.get(handle);

            let mut render_submeshes = Vec::new();
            for submesh in &mesh.submeshes {
                let material = if let Some(material) = submesh.material {
                    materials_to_register.push(material);
                    material
                } else {
                    self.default_material.unwrap()
                };

                render_submeshes.push(RenderSubmesh {
                    vertex_buffer: self.backend.create_vertex_buffer(&submesh.vertices),
                    index_buffer: self.backend.create_index_buffer(&submesh.indices),
                    index_count: submesh.indices.len() as u32,
                    material,
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
        let billboard_mode = match &material.billboard_mode {
            BillboardMode::Off => 0,
            BillboardMode::On => 1,
            BillboardMode::FixedSize => 2,
        };
        let material_uniform = MaterialUniform {
            base_color: material.base_color.into(),
            billboard_mode,
            unlit: material.unlit as u8 as u32,
            _padding: Default::default(),
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

    fn initialize_default_resources(&mut self, asset_server: &mut AssetServer) {
        let material = asset_server.add(Material::default());
        self.register_material(material, asset_server);
        self.default_material = Some(material);

        let mesh = asset_server.add(Mesh::quad());
        self.register_mesh(mesh, asset_server);
        self.quad_mesh = Some(mesh);
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
    projection: [f32; 16],
    view: [f32; 16],
    camera_transform: [f32; 16],
    ambient_light: [f32; 4],
}

#[derive(Default)]
struct RenderScene {
    meshes: HashMap<Handle<Mesh>, RenderMesh>,
    materials: HashMap<Handle<Material>, RenderMaterial>,
    textures: HashMap<Handle<Image>, wgpu::Texture>,
    lights: HashMap<UniqueNodeId, RenderLight>,
    mesh_instances: HashMap<UniqueNodeId, RenderMeshInstance>,
    texts: HashMap<UniqueNodeId, RenderText>,
    fullscreen_texture: Option<RenderFullscreenTexture>,
}

struct RenderFullscreenTexture {
    bind_group: wgpu::BindGroup,
    #[allow(unused)]
    sampler: wgpu::Sampler,
}

struct RenderText {
    instance_buffer: wgpu::Buffer,
    instance_count: u32,
}

struct RenderLight {
    bind_group: wgpu::BindGroup,
    #[allow(unused)]
    uniform_buffer: wgpu::Buffer,
    #[allow(unused)]
    sampler: wgpu::Sampler,
    shadow_map_scene_bind_group: wgpu::BindGroup,
    #[allow(unused)]
    shadow_map_scene_uniform_buffer: wgpu::Buffer,
    shadow_map: wgpu::Texture,
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
    material_override: Option<Handle<Material>>,
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
    billboard_mode: u32,
    unlit: u32,
    _padding: [u32; 2],
}

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct LightUniform {
    transform: [f32; 16],
    world_to_light: [f32; 16],
    color: [f32; 4],
    radius: f32,
    kind: u32, // Directional=0, Point=1
    _padding: [f32; 2],
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

impl RenderTargetTexture {
    pub fn depth(&self) -> &wgpu::Texture {
        match self {
            Self::Simple { depth, .. } | Self::Multisampled { depth, .. } => depth,
        }
    }
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
                store: wgpu::StoreOp::Store,
            },
        };

        let depth_stencil_attachment = wgpu::RenderPassDepthStencilAttachment {
            view: depth_view,
            depth_ops: Some(wgpu::Operations {
                load: wgpu::LoadOp::Clear(1.0),
                store: wgpu::StoreOp::Store,
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
