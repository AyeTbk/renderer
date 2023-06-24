use std::{
    any::{Any, TypeId},
    collections::{HashMap, HashSet},
    sync::mpsc,
    thread,
};

use crate::{
    arena::{Arena, Handle, TypeErasedHandle},
    Image, Material, Mesh, Scene, Timestamp,
};

use self::shader_source::ShaderSource;

mod gltf;
pub mod shader_source;

const FILES_CHECK_POLL_INTERVAL: f64 = 0.25;

pub struct AssetServer {
    arenas: HashMap<TypeId, Arena<Box<dyn Asset>>>,
    metadata: HashMap<TypeErasedHandle, Metadata>,
    changes: AssetChanges,
    last_changes_check: Timestamp,
    //
    work_sender: mpsc::Sender<Work>,
    work_result_receiver: mpsc::Receiver<WorkResult>,
}

impl Default for AssetServer {
    fn default() -> Self {
        Self::new()
    }
}

impl AssetServer {
    pub fn new() -> Self {
        let (work_sender, work_receiver) = mpsc::channel();
        let (work_result_sender, work_result_receiver) = mpsc::channel();
        let _ = Self::make_work_thread(work_receiver, work_result_sender);

        Self {
            arenas: Default::default(),
            metadata: Default::default(),
            changes: Default::default(),
            last_changes_check: Default::default(),
            //
            work_sender,
            work_result_receiver,
        }
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
            .insert(typed_handle.to_type_erased(), Metadata::new());
        typed_handle
    }

    pub fn load<A: Asset + Loadable>(&mut self, path: &str) -> Handle<A> {
        let handle = self.add(A::new_placeholder());
        self.set_asset_path(handle, path);
        self.work_sender
            .send(Work::LoadFromPath {
                handle: handle.to_type_erased(),
                loader: A::new_loader(),
                path: path.to_owned(),
            })
            .unwrap();
        handle
    }

    pub fn load_scene(&mut self, path: &str) -> Result<Handle<Scene>, String> {
        gltf::GtlfLoader::new(path, self)?.load()
    }

    pub fn load_shader_source(&mut self, path: &str) -> Result<Handle<ShaderSource>, String> {
        let shader_source = ShaderSource::load_from_path(path)?;
        let handle = self.add(shader_source);
        self.set_asset_path(handle, path);
        Ok(handle)
    }

    pub fn take_asset_changes(&mut self) -> AssetChanges {
        std::mem::take(&mut self.changes)
    }

    pub fn update(&mut self) {
        while let Ok((handle, result)) = self.work_result_receiver.try_recv() {
            let asset = result.unwrap();
            self.set_asset(handle, asset);

            if let Ok(handle) = handle.downcast::<Image>() {
                self.changes.images.insert(handle);
            }
        }

        if self.last_changes_check.seconds_since() > FILES_CHECK_POLL_INTERVAL {
            self.check_for_file_changes();

            self.last_changes_check = Timestamp::now();
        }
    }

    pub fn iter_assets<A: Asset>(&self) -> impl Iterator<Item = (Handle<A>, &A)> {
        self.get_arena::<A>()
            .elements()
            .map(|(generic_handle, boxed_asset)| {
                let typed_handle = unsafe { generic_handle.transmute() };
                let asset = boxed_asset
                    .as_any()
                    .downcast_ref::<A>()
                    .expect("all boxed asset of used arena should be of type A");
                (typed_handle, asset)
            })
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

    fn set_asset(&mut self, handle: TypeErasedHandle, asset: Box<dyn Asset>) {
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
            .get(&handle.to_type_erased())
            .expect("asset metadata should exist is a handle to it exists")
    }

    fn get_metadata_mut<A: Asset>(&mut self, handle: Handle<A>) -> &mut Metadata {
        self.metadata
            .get_mut(&handle.to_type_erased())
            .expect("asset metadata should exist is a handle to it exists")
    }

    fn check_for_file_changes(&mut self) {
        // TODO this

        let mut images_to_reload = Vec::new();
        for (handle, _) in self.iter_assets::<Image>() {
            let Some(path) = self.asset_path(handle) else { continue };
            let Ok(file_metadata) = std::fs::metadata(path) else { continue };
            let Ok(modified_time) = file_metadata.modified() else { continue };
            let modified_timestamp = Timestamp::from(modified_time);
            if self.asset_timestamp(handle) < modified_timestamp {
                images_to_reload.push((handle, modified_timestamp));
            }
        }
        for (handle, new_timestamp) in images_to_reload {
            let path = self.asset_path(handle).unwrap();
            self.send_load_from_path_work(handle, path);
            self.set_asset_timestamp(handle, new_timestamp);
        }

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

    pub(crate) fn asset_path<A: Asset>(&self, handle: Handle<A>) -> Option<&str> {
        self.get_metadata(handle).path.as_ref().map(|p| p.as_str())
    }

    pub(crate) fn set_asset_path<A: Asset>(&mut self, handle: Handle<A>, path: impl Into<String>) {
        self.get_metadata_mut(handle).path = Some(path.into());
    }

    pub(crate) fn asset_timestamp<A: Asset>(&self, handle: Handle<A>) -> Timestamp {
        self.get_metadata(handle).timestamp
    }

    pub(crate) fn set_asset_timestamp<A: Asset>(
        &mut self,
        handle: Handle<A>,
        timestamp: Timestamp,
    ) {
        self.get_metadata_mut(handle).timestamp = timestamp;
    }

    fn send_load_from_path_work<A: Asset + Loadable>(&self, handle: Handle<A>, path: &str) {
        self.work_sender
            .send(Work::LoadFromPath {
                handle: handle.to_type_erased(),
                loader: A::new_loader(),
                path: path.to_owned(),
            })
            .unwrap();
    }

    fn make_work_thread(
        work_receiver: mpsc::Receiver<Work>,
        result_sender: mpsc::Sender<WorkResult>,
    ) -> thread::JoinHandle<()> {
        thread::spawn(move || loop {
            match work_receiver.recv().unwrap() {
                Work::Terminate => break,
                Work::LoadFromPath {
                    handle,
                    loader,
                    path,
                } => {
                    let result = loader.load_from_path(&path);
                    result_sender.send((handle, result)).unwrap();
                }
            }
        })
    }
}

impl Drop for AssetServer {
    fn drop(&mut self) {
        let _ = self.work_sender.send(Work::Terminate);
    }
}

pub trait Asset: Any + Send {
    fn as_any(&self) -> &dyn Any;
    fn as_any_mut(&mut self) -> &mut dyn Any;
}

pub trait Loadable {
    fn new_placeholder() -> Self;
    fn new_loader() -> Box<dyn Loader>;
}

trait IsAsset: Send {}
impl IsAsset for Scene {}
impl IsAsset for Mesh {}
impl IsAsset for Material {}
impl IsAsset for Image {}
impl IsAsset for ShaderSource {}

impl<T: IsAsset + Any> Asset for T {
    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}

struct Metadata {
    path: Option<String>,
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

pub trait Loader: Send {
    fn load_from_path(&self, path: &str) -> Result<Box<dyn Asset>, String>;
}

#[derive(Default)]
pub struct AssetChanges {
    pub images: HashSet<Handle<Image>>,
    pub shader_sources: HashSet<Handle<ShaderSource>>,
}

enum Work {
    Terminate,
    LoadFromPath {
        handle: TypeErasedHandle,
        loader: Box<dyn Loader>,
        path: String,
    },
}

type WorkResult = (TypeErasedHandle, Result<Box<dyn Asset>, String>);
