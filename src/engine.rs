use asset_image::Image;
use glam::{Affine3A, UVec2, Vec2, Vec3};
use winit::{event::VirtualKeyCode, window::Window};

use crate::{
    arena::Handle,
    scene::{NodeData, NodeId},
    AssetServer, Input, Scene, VisualServer,
};

pub struct Engine {
    pub asset_server: AssetServer,
    pub visual_server: VisualServer,
    pub input: Input,
    pub display: Display,
    pub scene: Scene,
    gizmo_image: Handle<Image>,
}

impl Engine {
    pub fn new(window: &Window) -> Self {
        let mut asset_server = AssetServer::new();
        let gizmo_image = asset_server.load("data/gizmo_dummy.png");
        Self {
            visual_server: VisualServer::new(window, &mut asset_server),
            asset_server,
            input: Default::default(),
            display: Default::default(),
            scene: Scene::new_empty(),
            gizmo_image,
        }
    }

    pub fn set_window_inner_size(&mut self, size: UVec2) {
        self.display.window_inner_size = size;
        self.visual_server.set_render_size(size);
    }

    pub fn update(&mut self) {
        self.asset_server.update();

        self.notify_asset_changes();

        self.update_input();

        Self::update_node_recursive(
            self.scene.root,
            &mut self.scene,
            Affine3A::IDENTITY,
            &mut Context {
                asset_server: &mut self.asset_server,
                visual_server: &mut self.visual_server,
                display: &self.display,
                input: &self.input,
                time: &Time { delta: 1.0 / 60.0 },
                gizmo_image: self.gizmo_image,
            },
        );
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

    fn update_node_recursive(
        node_id: NodeId,
        scene: &mut Scene,
        parent_global_transform: Affine3A,
        context: &mut Context,
    ) {
        let unique_node_id = scene.make_unique_node_id(node_id);
        let node = scene.nodes.get_mut(node_id);

        if let Some(update_fn) = node.update_fn.take() {
            update_fn(node, context);
            node.update_fn = Some(update_fn);
        }

        let node_global_transform = parent_global_transform * node.transform;

        match &mut node.data {
            NodeData::Empty => (),
            NodeData::Camera(camera) => {
                camera.aspect_ratio = context.display.window_aspect_ratio();
                context
                    .visual_server
                    .set_camera(&node_global_transform, camera);
            }
            NodeData::Light(light) => {
                context
                    .visual_server
                    .set_light(unique_node_id, node_global_transform, light);
                context.visual_server.set_sprite(
                    unique_node_id,
                    node_global_transform,
                    context.gizmo_image,
                    light.color,
                    context.asset_server,
                );
            }
            NodeData::Mesh(mesh_handle) => {
                context.visual_server.set_mesh_instance(
                    unique_node_id,
                    node_global_transform,
                    *mesh_handle,
                    context.asset_server,
                );
            }
            NodeData::Scene(subscene) => {
                Self::update_node_recursive(
                    subscene.root,
                    subscene,
                    node_global_transform,
                    context,
                );
            }
            NodeData::Text(text, size) => {
                context.visual_server.set_text(
                    unique_node_id,
                    &node.transform, // Not global!! Intended (for now)
                    text,
                    *size,
                );
            }
        }

        let children = scene.children_of(node_id).to_vec();
        for child_id in children {
            Self::update_node_recursive(child_id, scene, node_global_transform, context);
        }
    }
}

pub struct Context<'a> {
    pub asset_server: &'a mut AssetServer,
    pub visual_server: &'a mut VisualServer,
    pub display: &'a Display,
    pub input: &'a Input,
    pub time: &'a Time,
    pub gizmo_image: Handle<Image>,
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
