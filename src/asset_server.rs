use std::path::Path;

use gltf::{buffer::Source, Gltf, Semantic};

use crate::{
    arena::{Arena, Handle},
    Image, Material, Mesh, Node, Scene, Submesh, Vertex,
};

pub struct AssetServer {
    scenes: Arena<Scene>,
    meshes: Arena<Mesh>,
    images: Arena<Image>,
    materials: Arena<Material>,
}

impl AssetServer {
    pub fn new() -> Self {
        Self {
            scenes: Default::default(),
            meshes: Default::default(),
            images: Default::default(),
            materials: Default::default(),
        }
    }

    pub fn get_mesh(&self, handle: Handle<Mesh>) -> &Mesh {
        self.meshes.get(handle)
    }

    pub fn get_material(&self, handle: Handle<Material>) -> &Material {
        self.materials.get(handle)
    }

    pub fn get_scene(&self, handle: Handle<Scene>) -> &Scene {
        self.scenes.get(handle)
    }

    pub fn load_scene(&mut self, path: impl AsRef<Path>) -> Result<Handle<Scene>, String> {
        let gltf = Gltf::open(path).map_err(|e| format!("{:?}", e))?;
        let gltf_bin = gltf
            .blob
            .as_ref()
            .ok_or_else(|| "missing blob".to_string())?;

        // TODO preallocate materials, meshes and images before creating the scene for simplicity

        for gltf_scene in gltf.scenes() {
            let mut scene = Scene::new_empty();

            for gltf_node in gltf_scene.nodes() {
                let node = if let Some(gltf_mesh) = gltf_node.mesh() {
                    let mut submeshes = Vec::new();
                    for gltf_primitive in gltf_mesh.primitives() {
                        // ## Make material
                        // FIXME reuse materials!!
                        let material = self.materials.allocate(Material {
                            base_color: gltf_primitive
                                .material()
                                .pbr_metallic_roughness()
                                .base_color_factor()
                                .into(),
                        });

                        // ## Get vertices data
                        let positions_accessor = gltf_primitive
                            .attributes()
                            .find_map(|(sem, accessor)| {
                                if sem == Semantic::Positions {
                                    Some(accessor)
                                } else {
                                    None
                                }
                            })
                            .ok_or_else(|| format!("missing positions attribute"))?;

                        assert!(positions_accessor.offset() == 0);
                        assert!(positions_accessor.data_type() == gltf::accessor::DataType::F32);
                        assert!(positions_accessor.view().is_some());
                        assert!(matches!(
                            positions_accessor.view().unwrap().buffer().source(),
                            Source::Bin
                        ));
                        assert!(positions_accessor.view().unwrap().stride().is_none());

                        let positions_view = positions_accessor.view().unwrap();
                        let positions_bytes = &gltf_bin[positions_view.offset()
                            ..positions_view.offset() + positions_view.length()];

                        let mut vertices = Vec::new();
                        for i in 0..positions_accessor.count() {
                            let position_idx = i * positions_accessor.size();
                            let read_coord = |j: usize| {
                                let coord_idx = position_idx + j * 4;
                                let coord_bytes = [
                                    positions_bytes[coord_idx + 0],
                                    positions_bytes[coord_idx + 1],
                                    positions_bytes[coord_idx + 2],
                                    positions_bytes[coord_idx + 3],
                                ];
                                f32::from_le_bytes(coord_bytes)
                            };

                            // Note: X coordinate is negated to convert from GLTF's right handed coordinate system to our left handed one.
                            let position = [-read_coord(0), read_coord(1), read_coord(2)];

                            vertices.push(Vertex {
                                position,
                                color: [0.0, 0.0, 0.0, 1.0],
                            });
                        }

                        // ## Get indices data
                        let indices_accessor = gltf_primitive
                            .indices()
                            .ok_or_else(|| "missing primitve indices".to_string())?;
                        assert!(indices_accessor.data_type() == gltf::accessor::DataType::U16);
                        let indices_view = indices_accessor.view().unwrap();
                        let indices_bytes = &gltf_bin
                            [indices_view.offset()..indices_view.offset() + indices_view.length()];
                        let indices = indices_bytes
                            .chunks_exact(2)
                            .map(|bytes| u16::from_le_bytes([bytes[0], bytes[1]]))
                            .collect::<Vec<u16>>();

                        let submesh = Submesh {
                            vertices,
                            indices,
                            material,
                        };
                        submeshes.push(submesh);
                    }

                    let mesh = Mesh { submeshes };
                    let mesh = self.meshes.allocate(mesh);
                    Node::new_mesh(mesh)
                } else {
                    Node::new_empty()
                };

                // Handle node's children
                for _gltf_child in gltf_node.children() {
                    todo!();
                }

                scene.add_allocate_child(scene.root, node);
            }

            let scene_handle = self.scenes.allocate(scene);
            return Ok(scene_handle);
        }

        Err("no scene in file".to_string())
    }
}
