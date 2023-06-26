use glam::{Vec2, Vec3};

use crate::{arena::Handle, renderer::Vertex, Material};

pub struct Mesh {
    pub submeshes: Vec<Submesh>,
}

pub struct Submesh {
    pub vertices: Vec<Vertex>,
    pub indices: Vec<u32>,
    pub material: Option<Handle<Material>>,
}

impl Mesh {
    pub fn quad() -> Self {
        // Verts   x/y          uv
        // 0---1        1       0-------1
        // |  /|        |       |
        // | / |   -1 ----- 1   |
        // |/  |        |       |
        // 2---3       -1       1

        let normal = Vec3::NEG_Z;
        Self {
            submeshes: vec![Submesh {
                vertices: vec![
                    Vertex::new((-0.5, 0.5, 0.0).into(), normal, Vec2::new(0.0, 0.0)),
                    Vertex::new((0.5, 0.5, 0.0).into(), normal, Vec2::new(1.0, 0.0)),
                    Vertex::new((-0.5, -0.5, 0.0).into(), normal, Vec2::new(0.0, 1.0)),
                    Vertex::new((0.5, -0.5, 0.0).into(), normal, Vec2::new(1.0, 1.0)),
                ],
                indices: vec![0, 2, 1, 1, 2, 3],
                material: None,
            }],
        }
    }
}
