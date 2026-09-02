use std::any::{Any, TypeId};
use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};
use std::sync::{Arc, OnceLock, RwLock};
use std::time::{Duration, Instant};

use dashmap::DashMap;
use notify::Watcher;
use rkyv::{Archive, Deserialize as RkyvDeserialize, Serialize as RkyvSerialize};
use serde::{Deserialize, Serialize};
use tokio::sync::{broadcast, watch};

static PATH_REGISTRY: OnceLock<DashMap<AssetId, String>> = OnceLock::new();

pub fn resolve_asset_root() -> PathBuf {
    if let Ok(path_str) = std::env::var("ENGINE_ASSETS_PATH") {
        let p = PathBuf::from(path_str);
        if p.exists() {
            return p;
        }
    }

    if let Ok(exe_path) = std::env::current_exe() {
        if let Some(exe_dir) = exe_path.parent() {
            let candidate = exe_dir.join("res");
            if candidate.exists() {
                return candidate;
            }
        }
    }

    let cwd_res = PathBuf::from("res");
    if cwd_res.exists() {
        return cwd_res.canonicalize().unwrap_or(cwd_res);
    }

    let mut cur = std::env::current_dir().unwrap_or_default();
    loop {
        let candidate = cur.join("res");
        if candidate.exists() {
            return candidate.canonicalize().unwrap_or(candidate);
        }
        if !cur.pop() {
            break;
        }
    }

    PathBuf::from("res")
}

fn find_cooker_executable() -> Option<PathBuf> {
    let mut candidates = Vec::new();

    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            candidates.push(dir.join("cooker"));
            candidates.push(dir.join("cooker.exe"));
        }
    }

    candidates.push(PathBuf::from("target/release/cooker"));
    candidates.push(PathBuf::from("target/release/cooker.exe"));
    candidates.push(PathBuf::from("target/debug/cooker"));
    candidates.push(PathBuf::from("target/debug/cooker.exe"));

    candidates.into_iter().find(|p| p.exists())
}

#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    PartialOrd,
    Ord,
    Eq,
    Hash,
    Serialize,
    Deserialize,
    Archive,
    RkyvSerialize,
    RkyvDeserialize,
)]
pub struct AssetId(pub u64);

impl AssetId {
    pub fn from_path(path: &str) -> Self {
        let normalized = path.replace('\\', "/");
        let mut hasher = fnv::FnvHasher::default();
        normalized.hash(&mut hasher);
        let id = AssetId(hasher.finish());

        let registry = PATH_REGISTRY.get_or_init(DashMap::new);
        if !registry.contains_key(&id) {
            registry.insert(id, normalized);
        }

        id
    }

    pub fn path(&self) -> String {
        PATH_REGISTRY
            .get()
            .and_then(|registry| registry.get(self).map(|p| p.clone()))
            .unwrap_or_else(|| format!("UnknownPath({})", self.0))
    }

    pub fn has_known_path(&self) -> bool {
        PATH_REGISTRY
            .get()
            .map(|registry| registry.contains_key(self))
            .unwrap_or(false)
    }
}

#[derive(Debug, Clone)]
pub enum AssetEvent {
    LoadStart { type_id: TypeId, id: AssetId },
    Loaded { type_id: TypeId, id: AssetId },
    Reloaded { type_id: TypeId, id: AssetId },
    Failed { type_id: TypeId, id: AssetId, error: String },
    Unloaded { type_id: TypeId, id: AssetId },
}

pub enum Entry<T: Send + Sync + 'static> {
    Loaded(Arc<T>),
    Loading(watch::Sender<Option<Result<Arc<T>, Arc<anyhow::Error>>>>),
    Failed {
        error: Arc<anyhow::Error>,
        timestamp: Instant,
    },
}

pub struct AssetStore<T: Send + Sync + 'static> {
    assets: Arc<DashMap<AssetId, Entry<T>>>,
    error_ttl: Duration,
}

impl<T: Send + Sync + 'static> AssetStore<T> {
    pub fn new(error_ttl: Duration) -> Self {
        Self {
            assets: Arc::new(DashMap::new()),
            error_ttl,
        }
    }

    pub fn get(&self, id: AssetId) -> Option<Arc<T>> {
        self.assets
            .get(&id)
            .and_then(|entry| match &*entry {
                Entry::Loaded(arc) => Some(arc.clone()),
                _ => None,
            })
    }

    pub async fn load<F, Fut>(
        &self,
        id: AssetId,
        event_sender: &broadcast::Sender<AssetEvent>,
        loader: F,
    ) -> Result<Arc<T>, Arc<anyhow::Error>>
    where
        F: FnOnce() -> Fut + Send + 'static,
        Fut: std::future::Future<Output = anyhow::Result<T>> + Send + 'static,
    {
        let type_id = TypeId::of::<T>();

        if let Some(entry) = self.assets.get(&id) {
            match &*entry {
                Entry::Loaded(arc) => return Ok(arc.clone()),
                Entry::Failed { error, timestamp } if timestamp.elapsed() < self.error_ttl => {
                    return Err(error.clone());
                }
                _ => {}
            }
        }

        let mut rx = loop {
            match self.assets.entry(id) {
                dashmap::mapref::entry::Entry::Vacant(vacant) => {
                    let (tx, rx) = watch::channel(None);
                    vacant.insert(Entry::Loading(tx.clone()));

                    let event_sender = event_sender.clone();
                    let store = Arc::clone(&self.assets);

                    tokio::spawn(async move {
                        let result_for_channel = loader().await.map(Arc::new).map_err(Arc::new);

                        let _ = tx.send(Some(result_for_channel.clone()));

                        let mut entry = store.entry(id).or_insert_with(|| Entry::Failed {
                            error: Arc::new(anyhow::anyhow!("impossible state")),
                            timestamp: Instant::now(),
                        });

                        match result_for_channel {
                            Ok(arc_data) => {
                                *entry = Entry::Loaded(arc_data);
                                let _ = event_sender.send(AssetEvent::Loaded { type_id, id });
                            }
                            Err(e) => {
                                *entry = Entry::Failed {
                                    error: e.clone(),
                                    timestamp: Instant::now(),
                                };
                                let _ = event_sender.send(AssetEvent::Failed {
                                    type_id,
                                    id,
                                    error: e.to_string(),
                                });
                            }
                        }
                    });
                    break rx;
                }
                dashmap::mapref::entry::Entry::Occupied(occupied) => {
                    match occupied.get() {
                        Entry::Loading(existing_tx) => break existing_tx.subscribe(),
                        Entry::Loaded(arc) => return Ok(arc.clone()),
                        Entry::Failed { error, timestamp } if timestamp.elapsed() < self.error_ttl => {
                            return Err(error.clone());
                        }
                        _ => {
                            drop(occupied);
                            self.assets.remove(&id);
                            continue;
                        }
                    }
                }
            }
        };

        Self::wait_for_result(&mut rx).await
    }

    async fn wait_for_result(
        rx: &mut watch::Receiver<Option<Result<Arc<T>, Arc<anyhow::Error>>>>,
    ) -> Result<Arc<T>, Arc<anyhow::Error>> {
        loop {
            {
                let borrowed = rx.borrow();
                if let Some(res) = borrowed.as_ref() {
                    match res {
                        Ok(data) => return Ok(data.clone()),
                        Err(e) => return Err(e.clone()),
                    }
                }
            }
            if rx.changed().await.is_err() {
                return Err(Arc::new(anyhow::anyhow!("Asset loading channel closed")));
            }
        }
    }

    pub fn unload(&self, id: AssetId) -> bool {
        self.assets.remove(&id).is_some()
    }
}

trait AnyStore: Send + Sync {
    fn as_any(&self) -> &dyn Any;
    fn remove(&self, id: AssetId) -> bool;
    //fn type_id(&self) -> TypeId;
}

struct StoreContainer<T: Send + Sync + 'static> {
    store: Arc<AssetStore<T>>,
}

impl<T: Send + Sync + 'static> AnyStore for StoreContainer<T> {
    fn as_any(&self) -> &dyn Any {
        &self.store
    }

    fn remove(&self, id: AssetId) -> bool {
        self.store.unload(id)
    }

    // fn type_id(&self) -> TypeId {
    //     TypeId::of::<T>()
    // }
}

pub struct AssetServer {
    root: PathBuf,
    stores: RwLock<HashMap<TypeId, Arc<dyn AnyStore>>>,
    event_sender: broadcast::Sender<AssetEvent>,
    error_ttl: Duration,
}

impl AssetServer {
    pub fn new(root: PathBuf, error_ttl: Duration) -> Self {
        let (event_sender, _) = broadcast::channel(512);
        Self {
            root,
            stores: RwLock::new(HashMap::new()),
            event_sender,
            error_ttl,
        }
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn resolve_path(&self, relative: impl AsRef<Path>) -> PathBuf {
        self.root.join(relative)
    }

    pub fn subscribe(&self) -> broadcast::Receiver<AssetEvent> {
        self.event_sender.subscribe()
    }

    pub fn get<T: Send + Sync + 'static>(&self, id: AssetId) -> Option<Arc<T>> {
        self.get_store::<T>()?.get(id)
    }

    pub async fn load<T, F, Fut>(
        &self,
        id: AssetId,
        loader: F,
    ) -> Result<Arc<T>, Arc<anyhow::Error>>
    where
        T: Send + Sync + 'static,
        F: FnOnce() -> Fut + Send + 'static,
        Fut: std::future::Future<Output = anyhow::Result<T>> + Send + 'static,
    {
        let store = self.get_or_create_store::<T>();

        let _ = self.event_sender.send(AssetEvent::LoadStart {
            type_id: TypeId::of::<T>(),
            id,
        });

        store.load(id, &self.event_sender, loader).await
    }

    pub fn unload<T: Send + Sync + 'static>(&self, id: AssetId) {
        if let Some(store) = self.get_store::<T>() {
            if store.unload(id) {
                let _ = self.event_sender.send(AssetEvent::Unloaded {
                    type_id: TypeId::of::<T>(),
                    id,
                });
            }
        }
    }

    fn get_store<T: Send + Sync + 'static>(&self) -> Option<Arc<AssetStore<T>>> {
        let stores = self.stores.read().unwrap();
        stores.get(&TypeId::of::<T>()).and_then(|any_store| {
            any_store
                .as_any()
                .downcast_ref::<Arc<AssetStore<T>>>()
                .cloned()
        })
    }

    fn get_or_create_store<T: Send + Sync + 'static>(&self) -> Arc<AssetStore<T>> {
        let tid = TypeId::of::<T>();

        {
            let stores = self.stores.read().unwrap();
            if let Some(store) = stores.get(&tid) {
                if let Some(arc) = store.as_any().downcast_ref::<Arc<AssetStore<T>>>() {
                    return arc.clone();
                }
            }
        }

        let mut stores = self.stores.write().unwrap();

        if let Some(store) = stores.get(&tid) {
            if let Some(arc) = store.as_any().downcast_ref::<Arc<AssetStore<T>>>() {
                return arc.clone();
            }
        }

        let new_store = Arc::new(AssetStore::<T>::new(self.error_ttl));
        let container = Arc::new(StoreContainer {
            store: new_store.clone(),
        }) as Arc<dyn AnyStore>;

        stores.insert(tid, container);
        new_store
    }

    pub fn watch_directory(self: &Arc<Self>) -> anyhow::Result<()> {
        let server = self.clone();
        let event_sender = self.event_sender.clone();
        let root_path = self.root.clone();

        std::thread::Builder::new()
            .name("asset-watcher".to_string())
            .spawn(move || {
                let (fs_tx, fs_rx) = std::sync::mpsc::channel();
                let mut watcher = match notify::recommended_watcher(fs_tx) {
                    Ok(w) => w,
                    Err(e) => {
                        log::error!("Failed to create file watcher: {:?}", e);
                        return;
                    }
                };

                if let Err(e) = watcher.watch(&root_path, notify::RecursiveMode::Recursive) {
                    log::error!("Failed to watch directory {:?}: {:?}", root_path, e);
                    return;
                }

                log::info!("Hot-Reloading watcher started for {:?}", root_path);

                let mut debounce_map: HashMap<PathBuf, Instant> = HashMap::new();

                loop {
                    while let Ok(res) = fs_rx.try_recv() {
                        if let Ok(event) = res {
                            if event.kind.is_modify() || event.kind.is_create() {
                                for path in event.paths {
                                    if path.extension().and_then(|e| e.to_str()) == Some("glb") {
                                        debounce_map.insert(path, Instant::now());
                                    }
                                }
                            }
                        }
                    }

                    let now = Instant::now();
                    let mut to_bake = Vec::new();

                    debounce_map.retain(|path, time| {
                        if now.duration_since(*time) >= Duration::from_millis(250) {
                            to_bake.push(path.clone());
                            false
                        } else {
                            true
                        }
                    });

                    for abs_path in to_bake {
                        let Some(cooker_bin) = find_cooker_executable() else {
                            log::error!("Cooker executable not found! Cannot bake {:?}", abs_path);
                            continue;
                        };

                        let bin_abs_path = abs_path.with_extension("bin");
                        log::info!("Debounced GLB modified: {:?}, triggering cooker...", abs_path);

                        let status = std::process::Command::new(&cooker_bin)
                            .arg(&abs_path)
                            .arg(&bin_abs_path)
                            .status();

                        if let Ok(status) = status {
                            if status.success() {
                                log::info!("Bake successful for {:?}", bin_abs_path);

                                if let Ok(rel_path) = bin_abs_path.strip_prefix(&server.root) {
                                    let rel_str = rel_path.to_string_lossy().replace('\\', "/");
                                    let bin_id = AssetId::from_path(&rel_str);

                                    let stores = server.stores.read().unwrap();
                                    for store in stores.values() {
                                        if store.remove(bin_id) {
                                            let _ = event_sender.send(AssetEvent::Reloaded {
                                                type_id: store.type_id(),
                                                id: bin_id,
                                            });
                                        }
                                    }
                                }
                            } else {
                                log::error!("Cooker returned error status: {:?}", status.code());
                            }
                        }
                    }

                    std::thread::sleep(Duration::from_millis(100));
                }
            })?;

        Ok(())
    }
}