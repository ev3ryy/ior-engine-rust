use std::any::{Any, TypeId};
use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use rkyv::{Archive, Deserialize as RkyvDeserialize, Serialize as RkyvSerialize};
use serde::{Deserialize, Serialize};
use std::sync::{Arc, OnceLock};
use dashmap::DashMap;
use std::time::{Instant, Duration};
use tokio::sync::{watch, broadcast};
use notify::{Watcher};

static PATH_REGISTRY: OnceLock<DashMap<AssetId, String>> = OnceLock::new();

impl AssetId {
    pub fn path(&self) -> String {
        PATH_REGISTRY
            .get()
            .and_then(|registry| registry.get(self).map(|p| p.clone()))
            .unwrap_or_else(|| format!("UnknownPath({})", self.0))
    }
}

// determine
#[derive(
    Debug, 
    Clone, 
    Copy, 
    PartialEq, 
    Eq, 
    Hash, 
    Serialize, 
    Deserialize, 
    Archive, 
    RkyvSerialize, 
    RkyvDeserialize
)]
pub struct AssetId(pub u64);

impl AssetId {
    pub fn from_path(path: &str) -> Self {
        let mut hasher = fnv::FnvHasher::default();
        path.hash(&mut hasher);
        let id = AssetId(hasher.finish());

        #[cfg(debug_assertions)]
        {
            let registry = PATH_REGISTRY.get_or_init(DashMap::new);
            registry.insert(id, path.to_string());
        }

        id
    }
}

#[derive(Debug, Clone)]
pub enum AssetEvent {
    LoadStart { type_id: TypeId, id: AssetId },
    Loaded { type_id: TypeId, id: AssetId },
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

        // fast way: check hash without exclusive block
        if let Some(entry) = self.assets.get(&id) {
            match &*entry {
                Entry::Loaded(arc) => return Ok(arc.clone()),
                Entry::Failed { error, timestamp } if timestamp.elapsed() < self.error_ttl => {
                    return Err(error.clone());
                }
                _ => {}
            }
        }

        // 2. Slow way
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
                                #[cfg(debug_assertions)]
                                error: format!("Failed to load {}: {}", id.path(), e),
                                #[cfg(not(debug_assertions))]
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
                return Err(Arc::new(anyhow::anyhow!("Asset loading cancelled (channel closed)")));
            }
        }
    }

    pub fn unload(&self, id: AssetId) {
        self.assets.remove(&id);
    }
}

trait AnyStore: Send + Sync {
    fn as_any(&self) -> &dyn Any;
    fn remove(&self, id: AssetId);
}

struct StoreContainer<T: Send + Sync + 'static> {
    store: Arc<AssetStore<T>>,
}

impl<T: Send + Sync + 'static> AnyStore for StoreContainer<T> {
    fn as_any(&self) -> &dyn Any {
        &self.store
    }

    fn remove(&self, id: AssetId) {
        self.store.unload(id);
    }
}

pub struct AssetServer {
    stores: std::sync::RwLock<HashMap<TypeId, Arc<dyn AnyStore>>>,
    
    event_sender: broadcast::Sender<AssetEvent>,
    
    error_ttl: Duration,
}

impl AssetServer {
    pub fn new(error_ttl: Duration) -> Self {
        let (event_sender, _) = broadcast::channel(256);
        Self {
            stores: std::sync::RwLock::new(HashMap::new()),
            event_sender,
            error_ttl,
        }
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
            id 
        });
        
        store.load(id, &self.event_sender, loader).await
    }

    pub fn unload<T: Send + Sync + 'static>(&self, id: AssetId) {
        if let Some(store) = self.get_store::<T>() {
            store.unload(id);
            let _ = self.event_sender.send(AssetEvent::Unloaded {
                type_id: TypeId::of::<T>(),
                id,
            });
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
        
        // fast way
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

    pub fn watch_directory(self: &std::sync::Arc<Self>, path: impl AsRef<std::path::Path>) -> anyhow::Result<()> {
        let server = self.clone();
        let event_sender = self.event_sender.clone();
        
        let path_buf = path.as_ref().to_path_buf();
        
        std::thread::spawn(move || {
            let (tx, rx) = std::sync::mpsc::channel();
            let mut watcher = notify::recommended_watcher(tx).unwrap();
            
            watcher.watch(&path_buf, notify::RecursiveMode::Recursive).unwrap();
            log::info!("Hot-Reloading watcher started for {:?}", path_buf);

            let mut last_run = std::time::Instant::now() - std::time::Duration::from_secs(5);

            for res in rx {
                match res {
                    Ok(event) => {
                        if !event.kind.is_modify() && !event.kind.is_create() { continue; }
                        
                        if last_run.elapsed() < std::time::Duration::from_secs(2) {
                            continue;
                        }

                        for path in event.paths {
                            if let Some(file_name) = path.file_name().and_then(|n| n.to_str()) {
                                if file_name.ends_with(".glb") {
                                    log::info!("File modified: {}, running Cooker...", file_name);
                                    
                                    let cooker_path = std::path::PathBuf::from("target/release/cooker");
                                    
                                    let glb_path = path.to_str().unwrap();
                                    let bin_path = path.with_extension("bin");
                                    
                                    let status = std::process::Command::new(&cooker_path)
                                        .arg(glb_path)
                                        .arg(bin_path.to_str().unwrap())
                                        .status();
                                        
                                    if status.is_ok() {
                                        log::info!("Bake successful!");
                                        last_run = std::time::Instant::now();
                                        
                                        let id = AssetId::from_path(file_name);
                                        let stores = server.stores.read().unwrap();
                                        for store in stores.values() {
                                            store.remove(id); 
                                        }
                                        let _ = event_sender.send(AssetEvent::Unloaded { 
                                            type_id: std::any::TypeId::of::<()>(),
                                            id,
                                        });
                                    }
                                }
                            }
                        }
                    }
                    Err(e) => log::error!("Watch error: {:?}", e),
                }
            }
        });

        Ok(())
    }
}