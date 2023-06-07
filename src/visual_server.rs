use std::collections::HashMap;

use glam::{Affine3A, Mat4, UVec2};

use crate::{
    arena::Handle,
    asset_server,
    scene::{NodeData, NodeId},
    AssetServer, Camera, Color, Material, Mesh, Renderer, Scene,
};

pub struct VisualServer {
    renderer: Renderer,
    camera: RenderCamera,
    render_scene: RenderScene,
}

impl VisualServer {
    pub fn new(window: &winit::window::Window) -> Self {
        Self {
            renderer: Renderer::new(window),
            camera: Default::default(),
            render_scene: Default::default(),
        }
    }

    pub fn render_size(&self) -> UVec2 {
        self.renderer.render_size()
    }

    pub fn set_render_size(&mut self, render_size: UVec2) {
        self.renderer.set_render_size(render_size)
    }

    pub fn render(&mut self) -> Result<(), wgpu::SurfaceError> {
        self.renderer.render()
    }

    pub fn set_scene(&mut self, scene: Handle<Scene>, asset_server: &AssetServer) {
        self.render_scene = Default::default();

        let scene = asset_server.get_scene(scene);
        self.register_node(Affine3A::IDENTITY, scene.root, scene, asset_server);
    }

    fn register_node(
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
                self.register_mesh_instance(node_transform, mesh_handle, asset_server);
            }
            NodeData::Camera(_) => (),
        }

        for &child_id in scene.children_of(node_id) {
            self.register_node(node_transform, child_id, scene, asset_server);
        }
    }

    fn register_mesh_instance(
        &mut self,
        transform: Affine3A,
        handle: Handle<Mesh>,
        asset_server: &AssetServer,
    ) {
        self.register_mesh(handle, asset_server);

        self.render_scene.mesh_instances.push(RenderMeshInstance {
            transform: transform.into(),
            mesh: handle,
        });
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
            self.render_scene.materials.insert(
                handle,
                RenderMaterial {
                    base_color: material.base_color.into(),
                },
            );
        }
    }
}

#[derive(Debug, Default)]
struct RenderCamera {
    transform: Mat4,
    projection: Mat4,
}

#[derive(Default)]
struct RenderScene {
    meshes: HashMap<Handle<Mesh>, RenderMesh>,
    materials: HashMap<Handle<Material>, RenderMaterial>,
    mesh_instances: Vec<RenderMeshInstance>,
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
    transform: Mat4,
    mesh: Handle<Mesh>,
}

struct RenderMaterial {
    base_color: [f32; 4],
}
