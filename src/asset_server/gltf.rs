use std::{
    collections::HashMap,
    mem::size_of,
    path::{Path, PathBuf},
};

use glam::{Affine3A, Quat, Vec3};
use gltf::{
    buffer::{self, Source},
    Gltf, Semantic,
};

use crate::{
    arena::Handle, renderer::Vertex, scene::NodeId, AssetServer, Image, Material, Mesh, Node,
    Scene, Submesh,
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
    images_ids_map: HashMap<usize, Handle<Image>>,
}

impl<'a> GtlfLoader<'a> {
    pub fn new(path: impl AsRef<Path>, asset_server: &'a mut AssetServer) -> Result<Self, String> {
        let path = path.as_ref();
        let gltf = Gltf::open(path).map_err(|e| format!("{:?}", e))?;
        let builtin_bin = gltf.blob.clone();

        Ok(Self {
            read: Read {
                base_path: path.parent().unwrap_or(Path::new("")).to_path_buf(),
                gltf,
                builtin_bin,
            },
            write: Write {
                asset_server,
                external_bins: Default::default(),
                material_ids_map: Default::default(),
                meshes_ids_map: Default::default(),
                images_ids_map: Default::default(),
            },
        })
    }

    pub fn load(&'a mut self) -> Result<Handle<Scene>, String> {
        self.write.load(&self.read)
    }
}

impl<'a> Write<'a> {
    pub fn load(&mut self, read: &'a Read) -> Result<Handle<Scene>, String> {
        // Preallocate textures/images
        for gltf_texture in read.gltf.textures() {
            let id = gltf_texture.index();
            let handle = match gltf_texture.source().source() {
                gltf::image::Source::Uri { uri, .. } => {
                    let full_path = Self::make_full_path(uri, read);
                    self.asset_server
                        .load(&full_path.to_string_lossy().to_string())
                }
                gltf::image::Source::View { view, .. } => {
                    if let Source::Uri(path) = view.buffer().source() {
                        self.load_external_bin(path, read)?;
                    }
                    let bytes = self.get_bytes_from_view(&view, read)?;
                    let image = Image::load_from_memory(bytes)?;
                    self.asset_server.add(image)
                }
            };

            self.images_ids_map.insert(id, handle);
        }

        // Preallocate materials
        self.material_ids_map
            .insert(None, self.asset_server.add(Material::default()));
        for gltf_material in read.gltf.materials() {
            let id = gltf_material.index();
            let pbr = gltf_material.pbr_metallic_roughness();
            let handle = self.asset_server.add(Material {
                base_color: pbr.base_color_factor().into(),
                base_color_image: pbr.base_color_texture().and_then(|info| {
                    let id = info.texture().index();
                    self.images_ids_map.get(&id).copied()
                }),
            });
            self.material_ids_map.insert(id, handle);
        }

        // Preallocate meshes
        for gltf_mesh in read.gltf.meshes() {
            let id = gltf_mesh.index();
            let mesh = self.gltf_mesh_to_mesh(&gltf_mesh, read)?;
            let handle = self.asset_server.add(mesh);
            self.meshes_ids_map.insert(id, handle);
        }

        // Load scene
        if let Some(gltf_scene) = read.gltf.scenes().next() {
            let mut scene = Scene::new_empty();

            for gltf_node in gltf_scene.nodes() {
                self.load_node_recursive(gltf_node, scene.root, &mut scene);
            }

            let scene_handle = self.asset_server.add(scene);
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
            assert!(matches!(gltf_primitive.mode(), gltf::mesh::Mode::Triangles));

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
                .ok_or_else(|| "missing positions attribute".to_string())?;

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
                .ok_or_else(|| "missing normals attribute".to_string())?;

            assert!(normals_accessor.data_type() == gltf::accessor::DataType::F32);
            assert!(normals_accessor.view().is_some());
            let normals_stride = normals_accessor
                .view()
                .unwrap()
                .stride()
                .unwrap_or(normals_accessor.size());

            let normals_view = normals_accessor.view().unwrap();
            if let buffer::Source::Uri(path) = normals_view.buffer().source() {
                self.load_external_bin(path, read)?;
            }

            // ### uv attribute
            let uvs_accessor = gltf_primitive
                .attributes()
                .find_map(|(sem, accessor)| {
                    if sem == Semantic::TexCoords(0) {
                        Some(accessor)
                    } else {
                        None
                    }
                })
                .ok_or_else(|| "missing uvs attribute".to_string())?;

            assert!(uvs_accessor.data_type() == gltf::accessor::DataType::F32);
            assert!(uvs_accessor.view().is_some());
            let uvs_stride = uvs_accessor
                .view()
                .unwrap()
                .stride()
                .unwrap_or(uvs_accessor.size());

            let uvs_view = uvs_accessor.view().unwrap();
            if let buffer::Source::Uri(path) = uvs_view.buffer().source() {
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

            let uvs_bin = self.get_bin_from_buffer_source(uvs_view.buffer().source(), read)?;
            let uvs_bytes = &uvs_bin[uvs_view.offset()..uvs_view.offset() + uvs_view.length()];

            let mut vertices = Vec::new();
            for i in 0..positions_accessor.count() {
                let position_idx = i * positions_stride + positions_accessor.offset();
                let read_pos_coord = |j: usize| {
                    let coord_idx = position_idx + j * size_of::<f32>();
                    let coord_bytes = [
                        positions_bytes[coord_idx],
                        positions_bytes[coord_idx + 1],
                        positions_bytes[coord_idx + 2],
                        positions_bytes[coord_idx + 3],
                    ];
                    f32::from_le_bytes(coord_bytes)
                };
                let normal_idx = i * normals_stride + normals_accessor.offset();
                let read_n_coord = |j: usize| {
                    let coord_idx = normal_idx + j * size_of::<f32>();
                    let coord_bytes = [
                        normals_bytes[coord_idx],
                        normals_bytes[coord_idx + 1],
                        normals_bytes[coord_idx + 2],
                        normals_bytes[coord_idx + 3],
                    ];
                    f32::from_le_bytes(coord_bytes)
                };
                let uv_idx = i * uvs_stride + uvs_accessor.offset();
                let read_uv_coord = |j: usize| {
                    let coord_idx = uv_idx + j * size_of::<f32>();
                    let coord_bytes = [
                        uvs_bytes[coord_idx],
                        uvs_bytes[coord_idx + 1],
                        uvs_bytes[coord_idx + 2],
                        uvs_bytes[coord_idx + 3],
                    ];
                    f32::from_le_bytes(coord_bytes)
                };

                // Note: X coordinate is negated to convert from GLTF's right handed coordinate system to our left handed one.
                let position = [-read_pos_coord(0), read_pos_coord(1), read_pos_coord(2)];
                let normal = [-read_n_coord(0), read_n_coord(1), read_n_coord(2)];
                let uv = [read_uv_coord(0), read_uv_coord(1)];

                vertices.push(Vertex {
                    position,
                    normal,
                    uv,
                });
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
            let indices_view_bytes =
                &indices_bin[indices_view.offset()..indices_view.offset() + indices_view.length()];
            let indices_bytes = &indices_view_bytes[indices_accessor.offset()
                ..indices_accessor.offset() + indices_accessor.count() * indices_accessor.size()];

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

    fn get_bytes_from_view(
        &self,
        view: &buffer::View<'a>,
        read: &'a Read,
    ) -> Result<&[u8], String> {
        let bin = self.get_bin_from_buffer_source(view.buffer().source(), read)?;
        let bytes = &bin[view.offset()..view.offset() + view.length()];
        Ok(bytes)
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
                let full_path = Self::make_full_path(path, read);
                Ok(self
                    .external_bins
                    .get(&full_path)
                    .expect("call load_external_bin before to make sure it's loaded"))
            }
        }
    }

    fn load_external_bin(&mut self, path: &str, read: &'a Read) -> Result<&[u8], String> {
        let full_path = Self::make_full_path(path, read);

        if !self.external_bins.contains_key(&full_path) {
            let bin = std::fs::read(&full_path).map_err(|e| format!("{:?}: {:?}", e, full_path))?;
            self.external_bins.insert(full_path.clone(), bin);
        }
        Ok(self.external_bins.get(&full_path).unwrap())
    }

    fn make_full_path(path: &str, read: &'a Read) -> PathBuf {
        // TODO remove PathBuf and use String instead
        let mut full_path = PathBuf::new();
        full_path.push(&read.base_path);
        full_path.push(path);
        full_path
    }
}
