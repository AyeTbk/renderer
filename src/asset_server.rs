use std::{collections::HashMap, path::Path};

use glam::{Affine3A, Quat, Vec3};
use gltf::{buffer::Source, Gltf, Semantic};

use crate::{
    arena::{Arena, Handle},
    scene::NodeId,
    Material, Mesh, Node, Scene, Submesh, Vertex,
};

#[derive(Default)]
pub struct AssetServer {
    scenes: Arena<Scene>,
    meshes: Arena<Mesh>,
    // images: Arena<Image>,
    materials: Arena<Material>,
}

impl AssetServer {
    pub fn new() -> Self {
        Self {
            scenes: Default::default(),
            meshes: Default::default(),
            // images: Default::default(),
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

        // Preallocate materials
        let mut material_ids_map = HashMap::<Option<usize>, Handle<Material>>::new();
        for gltf_material in gltf.materials() {
            let id = gltf_material.index();
            if !material_ids_map.contains_key(&id) {
                let handle = self.materials.allocate(Material {
                    base_color: gltf_material
                        .pbr_metallic_roughness()
                        .base_color_factor()
                        .into(),
                });
                material_ids_map.insert(id, handle);
            }
        }

        // Preallocate meshes
        let mut meshes_ids_map = HashMap::<usize, Handle<Mesh>>::new();
        for gltf_mesh in gltf.meshes() {
            let id = gltf_mesh.index();
            if !meshes_ids_map.contains_key(&id) {
                let mesh = Self::gltf_mesh_to_mesh(&gltf_mesh, gltf_bin, &mut material_ids_map)?;
                let handle = self.meshes.allocate(mesh);
                meshes_ids_map.insert(id, handle);
            }
        }

        for gltf_scene in gltf.scenes() {
            let mut scene = Scene::new_empty();

            for gltf_node in gltf_scene.nodes() {
                self.load_scene_node_recursive(
                    gltf_node,
                    scene.root,
                    &mut scene,
                    &mut meshes_ids_map,
                );
            }

            let scene_handle = self.scenes.allocate(scene);
            return Ok(scene_handle);
        }

        Err("no scene in file".to_string())
    }

    fn load_scene_node_recursive(
        &mut self,
        gltf_node: gltf::scene::Node,
        parent: NodeId,
        scene: &mut Scene,
        meshes_ids_map: &HashMap<usize, Handle<Mesh>>,
    ) {
        let mut node = if let Some(gltf_mesh) = gltf_node.mesh() {
            let mesh = *meshes_ids_map.get(&gltf_mesh.index()).unwrap();
            Node::new_mesh(mesh)
        } else {
            Node::new_empty()
        };

        node.transform = Self::gltf_transform_to_transform(gltf_node.transform());

        let node_id = scene.add_allocate_child(parent, node);

        // Handle node's children
        for gltf_child in gltf_node.children() {
            self.load_scene_node_recursive(gltf_child, node_id, scene, meshes_ids_map);
        }
    }

    fn gltf_transform_to_transform(transform: gltf::scene::Transform) -> Affine3A {
        // Note: account for GLTF's right handed coords -> renderer's left handed coords conversion
        let (t, r, s) = transform.decomposed();
        let translation = Vec3::new(-t[0], t[1], t[2]);
        let rotation = Quat::from_xyzw(r[0], -r[1], -r[2], r[3]);
        let scale = Vec3::new(s[0], s[1], s[2]);
        Affine3A::from_scale_rotation_translation(scale, rotation, translation)
    }

    fn gltf_mesh_to_mesh(
        gltf_mesh: &gltf::Mesh,
        gltf_bin: &[u8],
        material_ids_map: &mut HashMap<Option<usize>, Handle<Material>>,
    ) -> Result<Mesh, String> {
        let mut submeshes = Vec::new();
        for gltf_primitive in gltf_mesh.primitives() {
            let material = *material_ids_map
                .get(&gltf_primitive.material().index())
                .expect("material should be preallocated");

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
            let positions_bytes = &gltf_bin
                [positions_view.offset()..positions_view.offset() + positions_view.length()];

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
            let indices_bytes =
                &gltf_bin[indices_view.offset()..indices_view.offset() + indices_view.length()];
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

        Ok(Mesh { submeshes })
    }
}
