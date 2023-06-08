use glam::{Affine3A, UVec2, Vec3};
use winit::{event::VirtualKeyCode, window::Window};

use crate::{
    scene::{NodeData, NodeId},
    AssetServer, Input, Scene, VisualServer,
};

pub struct Engine {
    pub asset_server: AssetServer,
    pub visual_server: VisualServer,
    pub input: Input,
    pub display: Display,
    pub scene: Scene,
}

impl Engine {
    pub fn new(window: &Window) -> Self {
        Self {
            asset_server: AssetServer::new(),
            visual_server: VisualServer::new(window),
            input: Default::default(),
            display: Default::default(),
            scene: Scene::new_empty(),
        }
    }

    pub fn set_window_inner_size(&mut self, size: UVec2) {
        self.display.window_inner_size = size;
        self.visual_server.set_render_size(size);
    }

    pub fn update(&mut self) {
        self.update_input();

        self.update_node_recursive(self.scene.root, Affine3A::IDENTITY);
    }

    fn update_input(&mut self) {
        if let Some(pointer_pos) = self.input.pointer_pos {
            if let Some(previous_pointer_pos) = self.input.previous_pointer_pos {
                let delta_pointer = pointer_pos - previous_pointer_pos;
                let delta_view = delta_pointer / self.display.window_inner_size.y as f32;
                self.input.delta_view = delta_view;
            }
            self.input.previous_pointer_pos = Some(pointer_pos);
        }

        self.input.movement = Vec3::new(
            self.input
                .axis_strength(VirtualKeyCode::D, VirtualKeyCode::A),
            0.0,
            self.input
                .axis_strength(VirtualKeyCode::W, VirtualKeyCode::S),
        );

        self.input.fast = self.input.mod_shift;
    }

    fn update_node_recursive(&mut self, node_id: NodeId, parent_global_transform: Affine3A) {
        let node = self.scene.nodes.get_mut(node_id);

        if let Some(update_fn) = node.update_fn.take() {
            update_fn(
                node,
                node_id,
                Context {
                    asset_server: &mut self.asset_server,
                    visual_server: &mut self.visual_server,
                    input: &self.input,
                    time: &Time { delta: 1.0 / 60.0 },
                },
            );
            node.update_fn = Some(update_fn);
        }

        let node_global_transform = parent_global_transform * node.transform;

        match &mut node.data {
            NodeData::Mesh(mesh_handle) => {
                self.visual_server.set_mesh_instance(
                    node_id,
                    node_global_transform,
                    *mesh_handle,
                    &self.asset_server,
                );
            }
            NodeData::Camera(camera) => {
                camera.aspect_ratio = self.display.window_aspect_ratio();
                self.visual_server
                    .set_camera(&node_global_transform, camera);
            }
            _ => (),
        }

        for child_id in self.scene.children_of(node_id).to_vec() {
            self.update_node_recursive(child_id, node_global_transform);
        }
    }
}

pub struct Context<'a> {
    pub asset_server: &'a mut AssetServer,
    pub visual_server: &'a mut VisualServer,
    pub input: &'a Input,
    pub time: &'a Time,
}

pub struct Time {
    pub delta: f32,
}

#[derive(Debug, Default)]
pub struct Display {
    pub window_inner_size: UVec2,
}

impl Display {
    pub fn window_aspect_ratio(&self) -> f32 {
        self.window_inner_size.x as f32 / self.window_inner_size.y as f32
    }
}
