use crate::{arena::Handle, Material, Vertex};

pub struct Mesh {
    pub submeshes: Vec<Submesh>,
}

pub struct Submesh {
    pub vertices: Vec<Vertex>,
    pub indices: Vec<u32>,
    pub material: Handle<Material>,
}
