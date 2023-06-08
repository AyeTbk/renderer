use std::collections::HashMap;

use glam::{Affine3A, Mat4, UVec2};

use crate::{
    arena::Handle,
    renderer::RenderMeshCommand,
    scene::{NodeData, NodeId},
    AssetServer, Camera, Material, Mesh, Renderer, Scene,
};

pub struct VisualServer {
    renderer: Renderer,
    render_camera: RenderCamera,
    render_scene: RenderScene,
}

impl VisualServer {
    pub fn new(window: &winit::window::Window) -> Self {
        let mut renderer = Renderer::new(window);
        let camera_uniform = CameraUniform {
            view_projection: Camera::default().projection_matrix().to_cols_array(),
        };
        let render_camera = RenderCamera {
            uniform_buffer: renderer.create_uniform_buffer(camera_uniform),
        };

        Self {
            renderer,
            render_camera,
            render_scene: Default::default(),
        }
    }

    pub fn render_size(&self) -> UVec2 {
        self.renderer.render_size()
    }

    pub fn set_render_size(&mut self, render_size: UVec2) {
        self.renderer.set_render_size(render_size)
    }

    pub fn set_camera(&mut self, transform: &Affine3A, camera: &Camera) {
        let uniform = CameraUniform {
            view_projection: (camera.projection_matrix() * transform.inverse()).to_cols_array(),
        };
        self.renderer
            .update_uniform_buffer(&self.render_camera.uniform_buffer, uniform);
    }

    pub fn render(&mut self) -> Result<(), wgpu::SurfaceError> {
        for mesh_instance in self.render_scene.mesh_instances.values() {
            let mesh = self.render_scene.meshes.get(&mesh_instance.mesh).unwrap();

            let render_mesh_commands = mesh.submeshes.iter().map(|submesh| {
                let material = self.render_scene.materials.get(&submesh.material).unwrap();
                RenderMeshCommand {
                    material_bind_group: &material.bind_group,
                    model_bind_group: &mesh_instance.model_bind_group,
                    vertex_buffer: &submesh.vertex_buffer,
                    index_buffer: &submesh.index_buffer,
                    index_count: submesh.index_count,
                }
            });

            self.renderer
                .render_meshes(&self.render_camera.uniform_buffer, render_mesh_commands)?;
        }

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
            let bind_group = self.renderer.create_material_bind_group(&uniform_buffer);
            let render_material = RenderMaterial {
                bind_group,
                uniform_buffer,
            };

            self.render_scene.materials.insert(handle, render_material);
        }
    }
}

struct RenderCamera {
    uniform_buffer: wgpu::Buffer,
}

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct CameraUniform {
    view_projection: [f32; 16],
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
}

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct MaterialUniform {
    base_color: [f32; 4],
}
