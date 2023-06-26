use std::collections::HashMap;

use glam::Affine3A;

use crate::{
    arena::{Arena, Handle},
    engine::Context,
    Camera, Light, Mesh,
};

pub type NodeId = Handle<Node>;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct UniqueNodeId(Handle<Scene>, NodeId);

#[derive(Clone)]
pub struct Scene {
    pub handle: Option<Handle<Scene>>,
    pub nodes: Arena<Node>,
    pub root: NodeId,
    pub children: HashMap<NodeId, Vec<NodeId>>,
}

impl Scene {
    pub fn new_empty() -> Self {
        let mut nodes = Arena::default();
        let root = nodes.allocate(Node::with_data(NodeData::Empty));
        Self {
            handle: None,
            nodes,
            root,
            children: Default::default(),
        }
    }

    pub fn add_allocate_child(&mut self, parent: NodeId, child: Node) -> NodeId {
        let child = self.nodes.allocate(child);
        self.children.entry(parent).or_default().push(child);
        child
    }

    pub fn children_of(&self, node_id: NodeId) -> &[NodeId] {
        self.children
            .get(&node_id)
            .map(|v| v.as_ref())
            .unwrap_or(&[])
    }

    pub fn make_unique_node_id(&self, node_id: NodeId) -> UniqueNodeId {
        UniqueNodeId(self.handle.expect("dont call this if it crashes"), node_id)
    }
}

#[derive(Clone)]
pub struct Node {
    pub transform: Affine3A,
    pub data: NodeData,
    pub update_fn: Option<fn(&mut Node, &mut Context)>,
}

impl Node {
    pub fn new_empty() -> Self {
        Self::with_data(NodeData::Empty)
    }

    pub fn new_camera(camera: Camera) -> Self {
        Self::with_data(NodeData::Camera(camera))
    }

    pub fn new_light(light: Light) -> Self {
        Self::with_data(NodeData::Light(light))
    }

    pub fn new_mesh(mesh: Handle<Mesh>) -> Self {
        Self::with_data(NodeData::Mesh(mesh))
    }

    pub fn new_scene(scene: Scene) -> Self {
        Self::with_data(NodeData::Scene(Box::new(scene)))
    }

    pub fn new_text(text: impl Into<Vec<u8>>, size: f32) -> Self {
        Self::with_data(NodeData::Text(text.into(), size))
    }

    pub fn with_data(data: NodeData) -> Self {
        Self {
            transform: Default::default(),
            data,
            update_fn: None,
        }
    }

    pub fn with_transform(mut self, transform: Affine3A) -> Self {
        self.transform = transform;
        self
    }

    pub fn with_update(mut self, update_fn: fn(&mut Node, &mut Context)) -> Self {
        self.update_fn = Some(update_fn);
        self
    }
}

#[derive(Clone)]
pub enum NodeData {
    Empty,
    Camera(Camera),
    Light(Light),
    Mesh(Handle<Mesh>),
    Scene(Box<Scene>),
    Text(Vec<u8>, f32),
}
