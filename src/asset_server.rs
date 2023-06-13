use crate::{
    arena::{Arena, Handle},
    Image, Material, Mesh, Scene,
};

mod gltf;

#[derive(Default)]
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

    pub fn get_image(&self, handle: Handle<Image>) -> &Image {
        self.images.get(handle)
    }

    pub fn get_scene(&self, handle: Handle<Scene>) -> &Scene {
        self.scenes.get(handle)
    }

    pub fn load_scene(&mut self, path: &str) -> Result<Handle<Scene>, String> {
        gltf::GtlfLoader::new(path, self)?.load()
    }
}
