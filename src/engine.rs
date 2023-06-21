use glam::{Affine3A, UVec2, Vec2, Vec3};
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
        self.notify_asset_changes();

        self.update_input();

        self.update_node_recursive(self.scene.root, Affine3A::IDENTITY);
    }

    fn notify_asset_changes(&mut self) {
        let changes = self.asset_server.take_asset_changes();

        self.visual_server
            .notify_asset_changes(&changes, &mut self.asset_server);
    }

    fn update_input(&mut self) {
        if self.display.window_inner_size.y > 0 {
            let delta_view = self.input.pointer_delta / self.display.window_inner_size.y as f32;
            self.input.delta_view = delta_view;
        }

        self.input.movement = Vec3::new(
            self.input
                .axis_strength(VirtualKeyCode::D, VirtualKeyCode::A),
            self.input
                .axis_strength(VirtualKeyCode::Q, VirtualKeyCode::Z),
            self.input
                .axis_strength(VirtualKeyCode::W, VirtualKeyCode::S),
        );

        self.input.fast = self.input.mod_shift;

        // With how the mousemove event works, the delta has to be accumulated, and here I reset it.
        self.input.pointer_delta = Vec2::ZERO;
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

        let children = self.scene.children_of(node_id).to_vec();
        for child_id in children {
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
