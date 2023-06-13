use std::collections::HashMap;

use glam::{Affine3A, Mat4, UVec2, Vec3, Vec3Swizzles, Vec4};

use crate::{
    arena::Handle,
    scene::{NodeData, NodeId},
    AssetServer, Camera, Color, Material, Mesh, Scene,
};

pub mod renderer;
use self::renderer::*;

pub struct VisualServer {
    renderer: Renderer,
    render_scene: RenderScene,
    render_scene_data: RenderSceneData,
    white_texture: wgpu::Texture,
}

impl VisualServer {
    pub fn new(window: &winit::window::Window) -> Self {
        let mut renderer = Renderer::new(window);
        let scene_uniform = SceneUniform {
            projection_view: Camera::default().projection_matrix().to_cols_array(),
            view_pos: Vec4::default().to_array(),
            ambient_light: Color::new(0.3, 0.5, 0.9, 0.05).to_array(),
            sun_color: Color::new(1.0, 0.78, 0.7, 1.0).to_array(),
            sun_direction: Vec3::new(0.1, -1.0, 0.4)
                .normalize_or_zero()
                .xyzz()
                .to_array(),
        };
        let render_scene_data = RenderSceneData {
            uniform: scene_uniform,
            uniform_buffer: renderer.create_uniform_buffer(scene_uniform),
            depth_texture: renderer
                .create_depth_texture(window.inner_size().width, window.inner_size().height),
        };
        let white_texture = renderer.create_color_texture(1, 1, &[1, 1, 1, 1]);

        Self {
            renderer,
            render_scene: Default::default(),
            render_scene_data,
            white_texture,
        }
    }

    pub fn render_size(&self) -> UVec2 {
        self.renderer.render_size()
    }

    pub fn set_render_size(&mut self, render_size: UVec2) {
        self.renderer.set_render_size(render_size);

        self.render_scene_data.depth_texture = self
            .renderer
            .create_depth_texture(render_size.x, render_size.y);
    }

    pub fn set_camera(&mut self, transform: &Affine3A, camera: &Camera) {
        self.render_scene_data.uniform.projection_view =
            (camera.projection_matrix() * transform.inverse()).to_cols_array();
        self.render_scene_data.uniform.view_pos = transform.translation.xyzz().to_array();

        self.renderer.update_uniform_buffer(
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

        self.renderer.render_meshes(
            &self.render_scene_data.depth_texture,
            &self.render_scene_data.uniform_buffer,
            render_mesh_commands.into_iter(),
        )?;

        Ok(())
    }

    pub fn set_mesh_instance(
        &mut self,
        id: NodeId,
        transform: Affine3A,
        mesh_handle: Handle<Mesh>,
        asset_server: &AssetServer,
    ) {
        self.register_mesh_instance(id, transform, mesh_handle, asset_server);
    }

    pub fn set_scene(&mut self, scene: Handle<Scene>, asset_server: &AssetServer) {
        self.reset_scene();

        let scene = asset_server.get_scene(scene);
        self.register_node_recursive(Affine3A::IDENTITY, scene.root, scene, asset_server);
    }

    pub fn reset_scene(&mut self) {
        self.render_scene = Default::default();
    }

    fn register_node_recursive(
        &mut self,
        parent_transform: Affine3A,
        node_id: NodeId,
        scene: &Scene,
        asset_server: &AssetServer,
    ) {
        let node = scene.nodes.get(node_id);
        let node_transform = parent_transform * node.transform;

        match node.data {
            NodeData::Empty => (),
            NodeData::Mesh(mesh_handle) => {
                self.register_mesh_instance(node_id, node_transform, mesh_handle, asset_server);
            }
            NodeData::Camera(_) => (),
        }

        for &child_id in scene.children_of(node_id) {
            self.register_node_recursive(node_transform, child_id, scene, asset_server);
        }
    }

    fn register_mesh_instance(
        &mut self,
        id: NodeId,
        transform: Affine3A,
        handle: Handle<Mesh>,
        asset_server: &AssetServer,
    ) {
        self.register_mesh(handle, asset_server);

        let model_uniform = ModelUniform {
            transform: Mat4::from(transform).to_cols_array(),
        };
        let model_uniform_buffer = self.renderer.create_uniform_buffer(model_uniform);
        let model_bind_group = self.renderer.create_model_bind_group(&model_uniform_buffer);

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
        if !self.render_scene.meshes.contains_key(&handle) {
            let mesh = asset_server.get_mesh(handle);

            let mut render_submeshes = Vec::new();
            for submesh in &mesh.submeshes {
                self.register_material(submesh.material, asset_server);

                render_submeshes.push(RenderSubmesh {
                    vertex_buffer: self.renderer.create_vertex_buffer(&submesh.vertices),
                    index_buffer: self.renderer.create_index_buffer(&submesh.indices),
                    index_count: submesh.indices.len() as u32,
                    material: submesh.material,
                })
            }
            let render_mesh = RenderMesh {
                submeshes: render_submeshes,
            };
            self.render_scene.meshes.insert(handle, render_mesh);
        }
    }

    fn register_material(&mut self, handle: Handle<Material>, asset_server: &AssetServer) {
        if !self.render_scene.materials.contains_key(&handle) {
            let material = asset_server.get_material(handle);
            let material_uniform = MaterialUniform {
                base_color: material.base_color.into(),
            };

            let uniform_buffer = self.renderer.create_uniform_buffer(material_uniform);
            let base_color_texture = if let Some(image) = material.base_color_image {
                let image = asset_server.get_image(image);
                let texture =
                    self.renderer
                        .create_color_texture(image.width(), image.height(), image.data());
                Some(texture)
            } else {
                None
            };
            let sampler = self.renderer.create_sampler();

            let base_color_texture_ref = base_color_texture.as_ref().unwrap_or(&self.white_texture);

            let bind_group = self.renderer.create_material_bind_group(
                &uniform_buffer,
                base_color_texture_ref,
                &sampler,
            );
            let render_material = RenderMaterial {
                bind_group,
                uniform_buffer,
                base_color_texture,
                sampler,
            };

            self.render_scene.materials.insert(handle, render_material);
        }
    }
}

struct RenderSceneData {
    uniform: SceneUniform,
    uniform_buffer: wgpu::Buffer,
    depth_texture: wgpu::Texture,
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
    mesh_instances: HashMap<NodeId, RenderMeshInstance>,
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
    base_color_texture: Option<wgpu::Texture>,
    #[allow(unused)]
    sampler: wgpu::Sampler,
}

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct MaterialUniform {
    base_color: [f32; 4],
}
