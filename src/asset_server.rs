use std::path::{Path, PathBuf};

use crate::{
    arena::{Arena, Handle},
    Image, Material, Mesh, Scene, Timestamp,
};

mod gltf;

#[derive(Default)]
pub struct AssetServer {
    scenes: Arena<Asset<Scene>, Handle<Scene>>,
    meshes: Arena<Asset<Mesh>, Handle<Mesh>>,
    images: Arena<Asset<Image>, Handle<Image>>,
    materials: Arena<Asset<Material>, Handle<Material>>,
    changes: AssetChanges,
    last_changes_check: Timestamp,
}

impl AssetServer {
    pub fn new() -> Self {
        Default::default()
    }

    pub fn get_mesh(&self, handle: Handle<Mesh>) -> &Mesh {
        &self.meshes.get(handle).asset
    }

    pub fn get_material(&self, handle: Handle<Material>) -> &Material {
        &self.materials.get(handle).asset
    }

    pub fn get_image(&self, handle: Handle<Image>) -> &Image {
        &self.images.get(handle).asset
    }

    pub fn get_image_mut(&mut self, handle: Handle<Image>) -> &mut Image {
        self.changes.images.push(handle);
        &mut self.images.get_mut(handle).asset
    }

    pub fn get_scene(&self, handle: Handle<Scene>) -> &Scene {
        &self.scenes.get(handle).asset
    }

    pub fn add_scene(&mut self, scene: Scene) -> Handle<Scene> {
        self.scenes.allocate(Asset::new(scene))
    }

    pub fn add_mesh(&mut self, mesh: Mesh) -> Handle<Mesh> {
        self.meshes.allocate(Asset::new(mesh))
    }

    pub fn add_image(&mut self, mut image: Image) -> Handle<Image> {
        let _ = image.make_mips();
        self.images.allocate(Asset::new(image))
    }

    pub fn add_material(&mut self, material: Material) -> Handle<Material> {
        self.materials.allocate(Asset::new(material))
    }

    pub fn load_scene(&mut self, path: &str) -> Result<Handle<Scene>, String> {
        gltf::GtlfLoader::new(path, self)?.load()
    }

    pub fn load_image(&mut self, path: &str) -> Result<Handle<Image>, String> {
        let mut image = Image::load_from_path(path)?;
        let _ = image.make_mips();
        let handle = self.add_image(image);
        self.set_asset_path(handle, path);
        Ok(handle)
    }

    pub fn reload_image(
        &mut self,
        handle: Handle<Image>,
        new_timestamp: Timestamp,
    ) -> Result<(), String> {
        let path = self
            .asset_path(handle)
            .ok_or_else(|| "cannot reload a pathless asset".to_string())?;
        let mut image = Image::load_from_path(path)?;
        let _ = image.make_mips();
        *self.get_image_mut(handle) = image;
        self.set_asset_timestamp(handle, new_timestamp);
        Ok(())
    }

    pub(crate) fn take_asset_changes(&mut self) -> AssetChanges {
        if self.last_changes_check.seconds_since() > 0.25 {
            self.check_for_file_changes();

            self.last_changes_check = Timestamp::now();
        }

        std::mem::take(&mut self.changes)
    }

    fn check_for_file_changes(&mut self) {
        let mut images_to_reload = Vec::new();
        for (handle, asset) in self.images.elements() {
            let Some(path) = &asset.path else { continue };
            let Ok(file_metadata) = std::fs::metadata(path) else { continue };
            let Ok(modified_time) = file_metadata.modified() else { continue };
            let modified_timestamp = Timestamp::from(modified_time);
            if asset.timestamp < modified_timestamp {
                images_to_reload.push((handle, modified_timestamp));
            }
        }

        for (handle, new_timestamp) in images_to_reload {
            let _ = self.reload_image(handle, new_timestamp);
        }
    }
}

macro_rules! asset_dispatch {
    ($self:expr, let $name:ident = $handle:expr => $toks:block) => {{
        let handle = $handle.to_type_erased();
        if let Ok(handle) = handle.downcast::<Scene>() {
            let $name = $self.scenes.get_mut(handle);
            $toks
        } else if let Ok(handle) = handle.downcast::<Mesh>() {
            let $name = $self.meshes.get_mut(handle);
            $toks
        } else if let Ok(handle) = handle.downcast::<Image>() {
            let $name = $self.images.get_mut(handle);
            $toks
        } else if let Ok(handle) = handle.downcast::<Material>() {
            let $name = $self.materials.get_mut(handle);
            $toks
        } else {
            panic!(
                "invalid asset handle type: {:?}",
                std::any::type_name::<Handle<T>>()
            );
        }
    }};
}

impl AssetServer {
    pub(crate) fn asset_path<T: 'static>(&mut self, handle: Handle<T>) -> Option<&Path> {
        asset_dispatch!(self, let asset = handle => {
            asset.path.as_ref().map(|v| v.as_path())
        })
    }

    pub(crate) fn set_asset_path<T: 'static>(
        &mut self,
        handle: Handle<T>,
        path: impl Into<PathBuf>,
    ) {
        asset_dispatch!(self, let asset = handle => {
            asset.path = Some(path.into());
        });
    }

    // pub(crate) fn asset_timestamp<T: 'static>(&mut self, handle: Handle<T>) -> Timestamp {
    //     asset_dispatch!(self, let asset = handle => {
    //         asset.timestamp
    //     })
    // }

    pub(crate) fn set_asset_timestamp<T: 'static>(
        &mut self,
        handle: Handle<T>,
        timestamp: Timestamp,
    ) {
        asset_dispatch!(self, let asset = handle => {
            asset.timestamp = timestamp;
        });
    }
}

struct Asset<T> {
    asset: T,
    path: Option<PathBuf>,
    timestamp: Timestamp,
}

impl<T> Asset<T> {
    pub fn new(asset: T) -> Self {
        Self {
            asset,
            path: None,
            timestamp: Timestamp::now(),
        }
    }
}

#[derive(Default)]
pub struct AssetChanges {
    pub images: Vec<Handle<Image>>,
}
