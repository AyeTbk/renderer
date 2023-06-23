use std::{
    any::{Any, TypeId},
    collections::{HashMap, HashSet},
    path::{Path, PathBuf},
};

use crate::{
    arena::{Arena, Handle, UntypedHandle},
    Image, Material, Mesh, Scene, Timestamp,
};

use self::shader_source::ShaderSource;

mod gltf;
pub mod shader_source;

const FILES_CHECK_POLL_INTERVAL: f64 = 0.25;

#[derive(Default)]
pub struct AssetServer {
    arenas: HashMap<TypeId, Arena<Box<dyn Asset>>>,
    metadata: HashMap<UntypedHandle, Metadata>,
    changes: AssetChanges,
    last_changes_check: Timestamp,
}

impl AssetServer {
    pub fn new() -> Self {
        Default::default()
    }

    pub fn get<A: Asset>(&self, handle: Handle<A>) -> &A {
        let generic_handle = unsafe { handle.transmute() };
        self.get_arena::<A>()
            .get(generic_handle)
            .as_any()
            .downcast_ref()
            .expect("handle type and asset type should match such that this doesnt blow up")
    }

    pub fn get_mut<A: Asset>(&mut self, handle: Handle<A>) -> &mut A {
        // TODO notify changes
        let generic_handle = unsafe { handle.transmute() };
        self.get_arena_mut::<A>()
            .get_mut(generic_handle)
            .as_any_mut()
            .downcast_mut()
            .expect("handle type and asset type should match such that this doesnt blow up")
    }

    pub fn add<A: Asset>(&mut self, asset: A) -> Handle<A> {
        let generic_handle = self
            .get_or_create_arena_mut::<A>()
            .allocate(Box::new(asset));
        let typed_handle: Handle<A> = unsafe { generic_handle.transmute() };
        self.metadata
            .insert(typed_handle.to_untyped(), Metadata::new());
        typed_handle
    }

    pub fn load_scene(&mut self, path: &str) -> Result<Handle<Scene>, String> {
        gltf::GtlfLoader::new(path, self)?.load()
    }

    pub fn load_image(&mut self, path: &str) -> Result<Handle<Image>, String> {
        let mut image = Image::load_from_path(path)?;
        let _ = image.make_mips();
        let handle = self.add(image);
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
            .ok_or_else(|| "cannot reload a pathless image".to_string())?;

        let mut image = Image::load_from_path(path)?;
        self.set_asset_timestamp(handle, new_timestamp);

        let _ = image.make_mips();
        *self.get_mut(handle) = image;
        Ok(())
    }

    pub fn load_shader_source(&mut self, path: &str) -> Result<Handle<ShaderSource>, String> {
        let shader_source = ShaderSource::load_from_path(path)?;
        let handle = self.add(shader_source);
        self.set_asset_path(handle, path);
        Ok(handle)
    }

    pub fn reload_shader_source(
        &mut self,
        handle: Handle<ShaderSource>,
        new_timestamp: Timestamp,
    ) -> Result<(), String> {
        let path = self
            .asset_path(handle)
            .ok_or_else(|| "cannot reload a pathless shader source".to_string())?;

        let shader_source = ShaderSource::load_from_path(path)?;
        self.set_asset_timestamp(handle, new_timestamp);

        shader_source.validate()?;

        *self.get_mut(handle) = shader_source;
        Ok(())
    }

    pub(crate) fn take_asset_changes(&mut self) -> AssetChanges {
        if self.last_changes_check.seconds_since() > FILES_CHECK_POLL_INTERVAL {
            self.check_for_file_changes();

            self.last_changes_check = Timestamp::now();
        }

        std::mem::take(&mut self.changes)
    }

    fn get_arena<A: Asset>(&self) -> &Arena<Box<dyn Asset>> {
        self.arenas.get(&TypeId::of::<A>()).expect(&format!(
            "no asset of type added yet (how did you get a handle?): {}",
            std::any::type_name::<A>()
        ))
    }

    fn get_arena_mut<A: Asset>(&mut self) -> &mut Arena<Box<dyn Asset>> {
        self.arenas.get_mut(&TypeId::of::<A>()).expect(&format!(
            "no asset of type added yet (how did you get a handle?): {}",
            std::any::type_name::<A>()
        ))
    }

    fn get_or_create_arena_mut<A: Asset>(&mut self) -> &mut Arena<Box<dyn Asset>> {
        self.arenas
            .entry(TypeId::of::<A>())
            .or_insert_with(|| Arena::new())
    }

    fn _set_asset(&mut self, handle: UntypedHandle, asset: Box<dyn Asset>) {
        // TODO notify changes
        let generic_handle = unsafe { handle.transmute() };
        let slot = self
            .arenas
            .get_mut(&handle.erased_type_id())
            .expect("no arena registered for the handle's type id")
            .get_mut(generic_handle);
        *slot = asset;
    }

    fn get_metadata<A: Asset>(&self, handle: Handle<A>) -> &Metadata {
        self.metadata
            .get(&handle.to_untyped())
            .expect("asset metadata should exist is a handle to it exists")
    }

    fn get_metadata_mut<A: Asset>(&mut self, handle: Handle<A>) -> &mut Metadata {
        self.metadata
            .get_mut(&handle.to_untyped())
            .expect("asset metadata should exist is a handle to it exists")
    }

    fn check_for_file_changes(&mut self) {
        // TODO this

        // let mut images_to_reload = Vec::new();
        // for (handle, asset) in self.images.elements() {
        //     let Some(path) = &asset.path else { continue };
        //     let Ok(file_metadata) = std::fs::metadata(path) else { continue };
        //     let Ok(modified_time) = file_metadata.modified() else { continue };
        //     let modified_timestamp = Timestamp::from(modified_time);
        //     if asset.timestamp < modified_timestamp {
        //         images_to_reload.push((handle, modified_timestamp));
        //     }
        // }
        // for (handle, new_timestamp) in images_to_reload {
        //     let _ = self.reload_image(handle, new_timestamp);
        // }

        // let mut shader_sources_to_reload = Vec::new();
        // for (handle, asset) in self.shader_sources.elements() {
        //     let Some(path) = &asset.path else { continue };
        //     let Ok(file_metadata) = std::fs::metadata(path) else { continue };
        //     let Ok(modified_time) = file_metadata.modified() else { continue };
        //     let modified_timestamp = Timestamp::from(modified_time);
        //     if asset.timestamp < modified_timestamp {
        //         shader_sources_to_reload.push((handle, modified_timestamp));
        //     }
        // }
        // for (handle, new_timestamp) in shader_sources_to_reload {
        //     let _ = self.reload_shader_source(handle, new_timestamp);
        // }
    }

    pub(crate) fn asset_path<A: Asset>(&self, handle: Handle<A>) -> Option<&Path> {
        self.get_metadata(handle).path.as_ref().map(|p| p.as_path())
    }

    pub(crate) fn set_asset_path<A: Asset>(&mut self, handle: Handle<A>, path: impl Into<PathBuf>) {
        self.get_metadata_mut(handle).path = Some(path.into());
    }

    pub(crate) fn set_asset_timestamp<A: Asset>(
        &mut self,
        handle: Handle<A>,
        timestamp: Timestamp,
    ) {
        self.get_metadata_mut(handle).timestamp = timestamp;
    }
}

pub trait Asset: Any {
    fn as_any(&self) -> &dyn Any;
    fn as_any_mut(&mut self) -> &mut dyn Any;
}
impl<T: IsAsset + Any> Asset for T {
    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}

trait IsAsset {}
impl IsAsset for Scene {}
impl IsAsset for Mesh {}
impl IsAsset for Material {}
impl IsAsset for Image {}
impl IsAsset for ShaderSource {}

struct Metadata {
    path: Option<PathBuf>,
    timestamp: Timestamp,
}

impl Metadata {
    pub fn new() -> Self {
        Self {
            path: None,
            timestamp: Timestamp::now(),
        }
    }
}

#[derive(Default)]
pub struct AssetChanges {
    pub images: HashSet<Handle<Image>>,
    pub shader_sources: HashSet<Handle<ShaderSource>>,
}
