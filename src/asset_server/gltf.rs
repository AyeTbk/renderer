use std::{
    collections::HashMap,
    path::{Path, PathBuf},
};

use glam::{Affine3A, Quat, Vec3};
use gltf::{
    buffer::{self, Source},
    Gltf, Semantic,
};

use crate::{
    arena::Handle, scene::NodeId, AssetServer, Material, Mesh, Node, Scene, Submesh, Vertex,
};

pub struct GtlfLoader<'a> {
    read: Read,
    write: Write<'a>,
}

struct Read {
    base_path: PathBuf,
    gltf: Gltf,
    builtin_bin: Option<Vec<u8>>,
}

struct Write<'a> {
    asset_server: &'a mut AssetServer,
    external_bins: HashMap<PathBuf, Vec<u8>>,
    material_ids_map: HashMap<Option<usize>, Handle<Material>>,
    meshes_ids_map: HashMap<usize, Handle<Mesh>>,
}

impl<'a> GtlfLoader<'a> {
    pub fn new(path: impl AsRef<Path>, asset_server: &'a mut AssetServer) -> Result<Self, String> {
        let path = path.as_ref();
        let gltf = Gltf::open(path).map_err(|e| format!("{:?}", e))?;
        let builtin_bin = gltf.blob.clone();

        Ok(Self {
            read: Read {
                base_path: path.parent().unwrap_or(&Path::new("")).to_path_buf(),
                gltf,
                builtin_bin,
            },
            write: Write {
                asset_server,
                external_bins: Default::default(),
                material_ids_map: Default::default(),
                meshes_ids_map: Default::default(),
            },
        })
    }

    pub fn load(&'a mut self) -> Result<Handle<Scene>, String> {
        self.write.load(&self.read)
    }
}

impl<'a> Write<'a> {
    pub fn load(&mut self, read: &'a Read) -> Result<Handle<Scene>, String> {
        // Preallocate materials
        self.material_ids_map.insert(
            None,
            self.asset_server.materials.allocate(Material::default()),
        );
        for gltf_material in read.gltf.materials() {
            let id = gltf_material.index();
            if !self.material_ids_map.contains_key(&id) {
                let handle = self.asset_server.materials.allocate(Material {
                    base_color: gltf_material
                        .pbr_metallic_roughness()
                        .base_color_factor()
                        .into(),
                });
                self.material_ids_map.insert(id, handle);
            }
        }

        // Preallocate meshes
        for gltf_mesh in read.gltf.meshes() {
            let id = gltf_mesh.index();
            if !self.meshes_ids_map.contains_key(&id) {
                let mesh = self.gltf_mesh_to_mesh(&gltf_mesh, read)?;
                let handle = self.asset_server.meshes.allocate(mesh);
                self.meshes_ids_map.insert(id, handle);
            }
        }

        for gltf_scene in read.gltf.scenes() {
            let mut scene = Scene::new_empty();

            for gltf_node in gltf_scene.nodes() {
                self.load_node_recursive(gltf_node, scene.root, &mut scene);
            }

            let scene_handle = self.asset_server.scenes.allocate(scene);
            return Ok(scene_handle);
        }

        Err("no scene in file".to_string())
    }

    fn load_node_recursive(
        &mut self,
        gltf_node: gltf::scene::Node,
        parent: NodeId,
        scene: &mut Scene,
    ) {
        let mut node = if let Some(gltf_mesh) = gltf_node.mesh() {
            let mesh = *self.meshes_ids_map.get(&gltf_mesh.index()).unwrap();
            Node::new_mesh(mesh)
        } else {
            Node::new_empty()
        };

        node.transform = Self::gltf_transform_to_transform(gltf_node.transform());

        let node_id = scene.add_allocate_child(parent, node);

        // Handle node's children
        for gltf_child in gltf_node.children() {
            self.load_node_recursive(gltf_child, node_id, scene);
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

    fn gltf_mesh_to_mesh<'b>(
        &mut self,
        gltf_mesh: &'b gltf::Mesh,
        read: &'a Read,
    ) -> Result<Mesh, String>
    where
        'a: 'b,
    {
        let mut submeshes = Vec::new();
        for gltf_primitive in gltf_mesh.primitives() {
            let material = *self
                .material_ids_map
                .get(&gltf_primitive.material().index())
                .expect("material should be preallocated");

            // ## Get vertices data
            // ### position attribute
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

            assert!(positions_accessor.data_type() == gltf::accessor::DataType::F32);
            assert!(positions_accessor.view().is_some());
            let positions_stride = positions_accessor
                .view()
                .unwrap()
                .stride()
                .unwrap_or(positions_accessor.size());

            let positions_view = positions_accessor.view().unwrap();
            if let buffer::Source::Uri(path) = positions_view.buffer().source() {
                self.load_external_bin(path, read)?;
            }

            // ### normal attribute
            let normals_accessor = gltf_primitive
                .attributes()
                .find_map(|(sem, accessor)| {
                    if sem == Semantic::Normals {
                        Some(accessor)
                    } else {
                        None
                    }
                })
                .ok_or_else(|| format!("missing normals attribute"))?;

            assert!(normals_accessor.data_type() == gltf::accessor::DataType::F32);
            assert!(normals_accessor.view().is_some());
            let normals_stride = normals_accessor
                .view()
                .unwrap()
                .stride()
                .unwrap_or(positions_accessor.size());

            let normals_view = normals_accessor.view().unwrap();
            if let buffer::Source::Uri(path) = normals_view.buffer().source() {
                self.load_external_bin(path, read)?;
            }

            let positions_bin =
                self.get_bin_from_buffer_source(positions_view.buffer().source(), read)?;
            let positions_bytes = &positions_bin
                [positions_view.offset()..positions_view.offset() + positions_view.length()];

            let normals_bin =
                self.get_bin_from_buffer_source(normals_view.buffer().source(), read)?;
            let normals_bytes =
                &normals_bin[normals_view.offset()..normals_view.offset() + normals_view.length()];

            let mut vertices = Vec::new();
            for i in 0..positions_accessor.count() {
                let position_idx = i * positions_stride + positions_accessor.offset();
                let read_pos_coord = |j: usize| {
                    let coord_idx = position_idx + j * 4;
                    let coord_bytes = [
                        positions_bytes[coord_idx + 0],
                        positions_bytes[coord_idx + 1],
                        positions_bytes[coord_idx + 2],
                        positions_bytes[coord_idx + 3],
                    ];
                    f32::from_le_bytes(coord_bytes)
                };
                let normal_idx = i * normals_stride + normals_accessor.offset();
                let read_n_coord = |j: usize| {
                    let coord_idx = normal_idx + j * 4;
                    let coord_bytes = [
                        normals_bytes[coord_idx + 0],
                        normals_bytes[coord_idx + 1],
                        normals_bytes[coord_idx + 2],
                        normals_bytes[coord_idx + 3],
                    ];
                    f32::from_le_bytes(coord_bytes)
                };

                // Note: X coordinate is negated to convert from GLTF's right handed coordinate system to our left handed one.
                let position = [-read_pos_coord(0), read_pos_coord(1), read_pos_coord(2)];
                let normal = [-read_n_coord(0), read_n_coord(1), read_n_coord(2)];

                vertices.push(Vertex { position, normal });
            }

            // ## Get indices data
            let indices_accessor = gltf_primitive
                .indices()
                .ok_or_else(|| "missing primitve indices".to_string())?;
            let indices_view = indices_accessor.view().unwrap();
            if let buffer::Source::Uri(path) = indices_view.buffer().source() {
                self.load_external_bin(path, read)?;
            }
            let indices_bin =
                self.get_bin_from_buffer_source(indices_view.buffer().source(), read)?;
            let indices_bytes =
                &indices_bin[indices_view.offset()..indices_view.offset() + indices_view.length()];

            let indices = match indices_accessor.data_type() {
                gltf::accessor::DataType::U16 => indices_bytes
                    .chunks_exact(2)
                    .map(|bytes| u16::from_le_bytes([bytes[0], bytes[1]]))
                    .map(|short| short as u32)
                    .collect::<Vec<u32>>(),
                gltf::accessor::DataType::U32 => indices_bytes
                    .chunks_exact(4)
                    .map(|bytes| u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
                    .collect::<Vec<u32>>(),
                t => return Err(format!("unsuported index type: {:?}", t)),
            };

            let submesh = Submesh {
                vertices,
                indices,
                material,
            };
            submeshes.push(submesh);
        }

        Ok(Mesh { submeshes })
    }

    fn get_bin_from_buffer_source(
        &self,
        source: buffer::Source<'a>,
        read: &'a Read,
    ) -> Result<&[u8], String> {
        match source {
            Source::Bin => read
                .builtin_bin
                .as_ref()
                .map(|v| &v[..])
                .ok_or("expected builtin bin but it's missing".to_string()),
            Source::Uri(path) => {
                let mut full_path = PathBuf::new();
                full_path.push(&read.base_path);
                full_path.push(path);
                Ok(self
                    .external_bins
                    .get(&full_path)
                    .expect("call load_external_bin before to make sure it's loaded"))
            }
        }
    }

    fn load_external_bin(&mut self, path: &str, read: &'a Read) -> Result<&[u8], String> {
        let mut full_path = PathBuf::new();
        full_path.push(&read.base_path);
        full_path.push(path);

        if !self.external_bins.contains_key(&full_path) {
            let bin = std::fs::read(&full_path).map_err(|e| format!("{:?}: {:?}", e, full_path))?;
            self.external_bins.insert(full_path.clone(), bin);
        }
        Ok(self.external_bins.get(&full_path).unwrap())
    }
}
