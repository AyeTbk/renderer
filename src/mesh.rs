use crate::{arena::Handle, renderer::Vertex, Material};

pub struct Mesh {
    pub submeshes: Vec<Submesh>,
}

pub struct Submesh {
    pub vertices: Vec<Vertex>,
    pub indices: Vec<u32>,
    pub material: Handle<Material>,
}
