use std::{
    any::{Any, TypeId},
    collections::{HashMap, HashSet, VecDeque},
    sync::{mpsc, Mutex, RwLock},
    thread,
    time::Duration,
};

use crate::{
    arena::{Arena, Handle, TypeErasedHandle},
    Image, Material, Mesh, Scene, ShaderSource, Timestamp,
};

mod gltf;

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
        let _ = Self::make_work_threads(work_receiver, work_result_sender);

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
        self.load_with_options(path, "")
    }

    pub fn load_with_options<A: Asset + Loadable>(
        &mut self,
        path: &str,
        options: &str,
    ) -> Handle<A> {
        let handle = self.add(A::new_placeholder());
        self.set_asset_path(handle, path);
        self.set_asset_load_options(handle, options);
        self.reload(handle);

        handle
    }

    pub fn reload<A: Asset + Loadable>(&mut self, handle: Handle<A>) {
        let path = self
            .asset_path(handle)
            .expect("assets without path cannot be reloaded");
        let load_options = self.asset_load_options(handle);
        let mut loader = A::new_loader(load_options);
        if loader.only_sync() {
            if let Ok(boxed_asset) = loader.load_from_path(path) {
                self.set_asset(handle.to_type_erased(), boxed_asset);
                self.finish_asset_reload(handle);
            } else {
                eprintln!("AssetServer::reload(): asset failed to load: {}", path);
            }
        } else {
            self.work_sender
                .send(Work::LoadFromPath {
                    handle: handle.to_type_erased(),
                    loader,
                    path: path.to_owned(),
                })
                .unwrap();
        }
        self.set_asset_timestamp(handle, Timestamp::now());
    }

    pub fn load_scene(&mut self, path: &str) -> Result<Handle<Scene>, String> {
        gltf::GtlfLoader::new(path, self)?.load()
    }

    pub fn take_asset_changes(&mut self) -> AssetChanges {
        std::mem::take(&mut self.changes)
    }

    pub fn update(&mut self) {
        while let Ok((handle, result)) = self.work_result_receiver.try_recv() {
            let asset = result.unwrap();
            self.set_asset(handle, asset);

            if let Ok(handle) = handle.downcast::<Image>() {
                self.finish_asset_reload(handle);
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
            .expect("asset metadata should exist if a handle to it exists")
    }

    fn get_metadata_mut<A: Asset>(&mut self, handle: Handle<A>) -> &mut Metadata {
        self.metadata
            .get_mut(&handle.to_type_erased())
            .expect("asset metadata should exist if a handle to it exists")
    }

    fn check_for_file_changes(&mut self) {
        let mut images_to_reload = Vec::new();
        for (handle, _) in self.iter_assets::<Image>() {
            let Some(path) = self.asset_path(handle) else { continue };
            let Ok(file_metadata) = std::fs::metadata(path) else { continue };
            let Ok(modified_time) = file_metadata.modified() else { continue };
            let modified_timestamp = Timestamp::from(modified_time);
            if self.asset_timestamp(handle) < modified_timestamp {
                images_to_reload.push(handle);
            }
        }
        for handle in images_to_reload {
            self.reload(handle);
        }

        let mut shader_sources_to_reload = Vec::new();
        for (handle, _) in self.iter_assets::<ShaderSource>() {
            let Some(path) = self.asset_path(handle) else { continue };
            let Ok(file_metadata) = std::fs::metadata(path) else { continue };
            let Ok(modified_time) = file_metadata.modified() else { continue };
            let modified_timestamp = Timestamp::from(modified_time);
            if self.asset_timestamp(handle) < modified_timestamp {
                shader_sources_to_reload.push(handle);
            }
        }
        for handle in shader_sources_to_reload {
            self.reload(handle);
        }
    }

    pub(crate) fn asset_path<A: Asset>(&self, handle: Handle<A>) -> Option<&str> {
        self.get_metadata(handle).path.as_ref().map(|p| p.as_str())
    }

    pub(crate) fn set_asset_path<A: Asset>(&mut self, handle: Handle<A>, path: impl Into<String>) {
        self.get_metadata_mut(handle).path = Some(path.into());
    }

    pub(crate) fn asset_load_options<A: Asset>(&self, handle: Handle<A>) -> &str {
        self.get_metadata(handle).load_options.as_str()
    }

    pub(crate) fn set_asset_load_options<A: Asset>(
        &mut self,
        handle: Handle<A>,
        load_options: impl Into<String>,
    ) {
        self.get_metadata_mut(handle).load_options = load_options.into();
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

    fn finish_asset_reload<A: Asset>(&mut self, handle: Handle<A>) {
        self.changes.assets.insert(handle.to_type_erased());
    }

    fn make_work_threads(
        work_receiver: mpsc::Receiver<Work>,
        result_sender: mpsc::Sender<WorkResult>,
    ) -> thread::JoinHandle<()> {
        thread::spawn(move || {
            let available_work: Mutex<VecDeque<Work>> = Mutex::new(VecDeque::new());
            let finished_work: Mutex<VecDeque<WorkResult>> = Mutex::new(VecDeque::new());
            let terminate: RwLock<bool> = RwLock::new(false);

            const SPARE_TIME_SLEEP_DURATION: Duration = Duration::from_millis(100);

            thread::scope(|s| {
                let core_count = thread::available_parallelism()
                    .map(|c| c.get().saturating_sub(4)) // chosen by a fair dice roll.
                    .unwrap_or(1)
                    .max(1);
                for _ in 0..core_count {
                    s.spawn(|| {
                        while !*terminate.read().unwrap() {
                            let Some(work) = ({ available_work.lock().unwrap().pop_front() }) else {
                                thread::sleep(SPARE_TIME_SLEEP_DURATION);
                                continue;
                            };

                            match work {
                                Work::LoadFromPath {
                                    handle,
                                    mut loader,
                                    path,
                                } => {
                                    let result = loader.load_from_path(&path);
                                    finished_work.lock().unwrap().push_back((handle, result));
                                }
                                _ => (),
                            }
                        }
                    });
                }

                loop {
                    while let Ok(work) = work_receiver.recv_timeout(SPARE_TIME_SLEEP_DURATION) {
                        match work {
                            Work::Terminate => {
                                *terminate.write().unwrap() = true;
                                break;
                            }
                            work => {
                                available_work.lock().unwrap().push_back(work);
                            }
                        }
                    }

                    for work_result in finished_work.lock().unwrap().drain(..) {
                        result_sender.send(work_result).unwrap();
                    }
                }
            });
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
    fn new_loader(options: &str) -> Box<dyn Loader>;
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
    load_options: String,
}

impl Metadata {
    pub fn new() -> Self {
        Self {
            path: None,
            timestamp: Timestamp::now(),
            load_options: String::new(),
        }
    }
}

pub trait Loader: Send {
    fn load_from_path(&mut self, path: &str) -> Result<Box<dyn Asset>, String>;

    fn only_sync(&self) -> bool {
        false
    }
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

#[derive(Default)]
pub struct AssetChanges {
    pub assets: HashSet<TypeErasedHandle>,
}

impl AssetChanges {
    pub fn iter<A: Asset>(&self) -> impl Iterator<Item = Handle<A>> + '_ {
        self.assets
            .iter()
            .filter_map(|type_erased_handle| type_erased_handle.downcast().ok())
    }

    pub fn contains<A: Asset>(&self, handle: Handle<A>) -> bool {
        self.assets.contains(&handle.to_type_erased())
    }
}
