use std::collections::HashMap;

use glam::Affine3A;

use crate::{
    arena::{Arena, Handle},
    Camera, Mesh,
};

pub type NodeId = Handle<Node>;

pub struct Scene {
    pub nodes: Arena<Node>,
    pub root: NodeId,
    pub children: HashMap<NodeId, Vec<NodeId>>,
}

impl Scene {
    pub fn new_empty() -> Self {
        let mut nodes = Arena::default();
        let root = nodes.allocate(Node::with_data(NodeData::Empty));
        Self {
            nodes,
            root,
            children: Default::default(),
        }
    }

    pub fn add_allocate_child(&mut self, parent: NodeId, child: Node) {
        let child = self.nodes.allocate(child);
        self.children.entry(parent).or_default().push(child);
    }

    pub fn children_of(&self, node_id: NodeId) -> &[NodeId] {
        self.children
            .get(&node_id)
            .map(|v| v.as_ref())
            .unwrap_or(&[])
    }
}

pub struct Node {
    pub transform: Affine3A,
    pub data: NodeData,
}

impl Node {
    pub fn new_empty() -> Self {
        Self::with_data(NodeData::Empty)
    }

    pub fn new_mesh(mesh: Handle<Mesh>) -> Self {
        Self::with_data(NodeData::Mesh(mesh))
    }

    pub fn with_data(data: NodeData) -> Self {
        Self {
            transform: Default::default(),
            data,
        }
    }
}

pub enum NodeData {
    Empty,
    Camera(Camera),
    Mesh(Handle<Mesh>),
}
