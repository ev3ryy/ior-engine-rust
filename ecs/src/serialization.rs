use crate::components::*;
use crate::entity::Entity;
use crate::world::{Component, World};
use flecs_ecs::prelude::*;
use glam::{Quat, Vec3};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap};
use std::fs::File;
use std::io::Write;
use std::path::Path;
use std::sync::Arc;

pub type SerializeFn = Arc<dyn Fn(&World, Entity) -> Option<String> + Send + Sync>;
pub type DeserializeFn = Arc<dyn Fn(&World, Entity, &str) -> anyhow::Result<()> + Send + Sync>;
pub type HasFn = Arc<dyn Fn(&World, Entity) -> bool + Send + Sync>;

#[derive(Clone)]
pub struct ComponentRegistration {
    pub name: &'static str,
    pub serialize: SerializeFn,
    pub deserialize: DeserializeFn,
    pub has: HasFn,
    pub add_default: Arc<dyn Fn(&World, Entity) + Send + Sync>,
    pub remove: Arc<dyn Fn(&World, Entity) + Send + Sync>,
}

#[derive(Default, Clone)]
pub struct SerializationRegistry {
    entries: HashMap<&'static str, ComponentRegistration>,
    ordered_names: Vec<&'static str>,
}

impl SerializationRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register<T>(&mut self, name: &'static str)
    where
        T: Component + DataComponent + Serialize + DeserializeOwned + Default + 'static,
    {
        if self.entries.contains_key(name) {
            return;
        }

        let serialize: SerializeFn = Arc::new(|world, entity| {
            world.with::<T, _>(entity, |comp| ron::to_string(comp).ok()).flatten()
        });

        let deserialize: DeserializeFn = Arc::new(|world, entity, ron_str| {
            let comp: T = ron::from_str(ron_str)?;
            world.set(entity, comp);
            Ok(())
        });

        let has: HasFn = Arc::new(|world, entity| world.has::<T>(entity));
        let add_default = Arc::new(|world: &World, entity: Entity| {
            world.set(entity, T::default());
        });
        let remove = Arc::new(|world: &World, entity: Entity| {
            world.remove::<T>(entity);
        });

        self.entries.insert(
            name,
            ComponentRegistration {
                name,
                serialize,
                deserialize,
                has,
                add_default,
                remove,
            },
        );
        self.ordered_names.push(name);
    }

    pub fn register_tag<T>(&mut self, name: &'static str)
    where
        T: Component + 'static,
    {
        if self.entries.contains_key(name) {
            return;
        }

        let serialize: SerializeFn = Arc::new(|_world, _entity| Some("()".to_string()));

        let deserialize: DeserializeFn = Arc::new(|world, entity, _ron_str| {
            world.add_tag::<T>(entity);
            Ok(())
        });

        let has: HasFn = Arc::new(|world, entity| world.has::<T>(entity));
        let add_default = Arc::new(|world: &World, entity: Entity| {
            world.add_tag::<T>(entity);
        });
        let remove = Arc::new(|world: &World, entity: Entity| {
            world.remove::<T>(entity);
        });

        self.entries.insert(
            name,
            ComponentRegistration {
                name,
                serialize,
                deserialize,
                has,
                add_default,
                remove,
            },
        );
        self.ordered_names.push(name);
    }

    pub fn get(&self, name: &str) -> Option<&ComponentRegistration> {
        self.entries.get(name)
    }

    pub fn iter(&self) -> impl Iterator<Item = &ComponentRegistration> {
        self.ordered_names
            .iter()
            .filter_map(|name| self.entries.get(name))
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct EntityRecord {
    pub name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent_index: Option<usize>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub components: BTreeMap<String, String>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct WorldSnapshot {
    #[serde(default = "default_version")]
    pub version: u32,
    pub entities: Vec<EntityRecord>,
}

fn default_version() -> u32 {
    1
}

pub fn create_world_snapshot(world: &World) -> anyhow::Result<Vec<u8>> {
    let mut flecs_id_to_index = HashMap::new();
    let mut entity_ids = Vec::new();

    world.inner.each_entity::<&SceneEntity>(|entity, _| {
        flecs_id_to_index.insert(*entity.id(), entity_ids.len());
        entity_ids.push(Entity(*entity.id()));
    });

    let registry = world.serialization_registry.read().unwrap();
    let mut records = Vec::with_capacity(entity_ids.len());

    for &entity in &entity_ids {
        let name = {
            let n = world.get_entity_name(entity);
            if n.is_empty() {
                None
            } else {
                Some(n)
            }
        };

        let parent_index = {
            let e = world.inner.entity_from_id(entity.0);
            e.parent().and_then(|p| flecs_id_to_index.get(&*p.id()).copied())
        };

        let mut components = BTreeMap::new();
        for reg in registry.iter() {
            if (reg.has)(world, entity) {
                if let Some(val_str) = (reg.serialize)(world, entity) {
                    components.insert(reg.name.to_string(), val_str);
                }
            }
        }

        records.push(EntityRecord {
            name,
            parent_index,
            components,
        });
    }

    let snapshot = WorldSnapshot {
        version: 1,
        entities: records,
    };

    let pretty = ron::ser::PrettyConfig::default()
        .depth_limit(6)
        .struct_names(true);
    let ron_string = ron::ser::to_string_pretty(&snapshot, pretty)?;

    Ok(ron_string.into_bytes())
}

pub fn restore_world_snapshot(world: &mut World, bytes: &[u8]) -> anyhow::Result<()> {
    let ron_str = std::str::from_utf8(bytes)
        .map_err(|e| anyhow::anyhow!("Invalid UTF-8 in scene file: {}", e))?;

    let snapshot: WorldSnapshot = ron::from_str(ron_str)
        .map_err(|e| anyhow::anyhow!("RON scene syntax error: {}", e))?;

    world.clear_scene();

    let registry = world.serialization_registry.read().unwrap().clone();
    let mut spawned_entities = Vec::with_capacity(snapshot.entities.len());

    for record in &snapshot.entities {
        let name = record.name.as_deref().unwrap_or("Entity");
        let entity = world.spawn_empty(name);
        spawned_entities.push(entity);
    }

    for (i, record) in snapshot.entities.iter().enumerate() {
        if let Some(parent_idx) = record.parent_index {
            if let Some(&parent_entity) = spawned_entities.get(parent_idx) {
                let child = world.inner.entity_from_id(spawned_entities[i].0);
                let parent = world.inner.entity_from_id(parent_entity.0);
                child.child_of(parent);
            }
        }
    }

    for (i, record) in snapshot.entities.iter().enumerate() {
        let entity = spawned_entities[i];
        for (comp_name, comp_str) in &record.components {
            if let Some(entry) = registry.get(comp_name.as_str()) {
                if let Err(e) = (entry.deserialize)(world, entity, comp_str) {
                    log::warn!(
                        "[ECS] Error deserializing '{}' for entity {:?}: {}",
                        comp_name,
                        record.name,
                        e
                    );
                }
            } else {
                log::warn!(
                    "[ECS] Unknown component '{}' for entity {:?}. Skipping.",
                    comp_name,
                    record.name
                );
            }
        }
    }

    world.update_transforms();
    Ok(())
}

pub fn save_world(world: &World, path: &str) -> anyhow::Result<()> {
    let bytes = create_world_snapshot(world)?;

    let path_obj = Path::new(path);
    if path_obj.exists() {
        let bak_path = format!("{}.bak", path);
        let _ = std::fs::copy(path, &bak_path);
    }

    let mut file = File::create(path)?;
    file.write_all(&bytes)?;
    Ok(())
}

pub fn load_world(world: &mut World, path: &str) -> anyhow::Result<()> {
    let bytes = std::fs::read(path)?;
    restore_world_snapshot(world, &bytes)?;
    Ok(())
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Prefab {
    #[serde(default = "default_version")]
    pub version: u32,
    pub entities: Vec<EntityRecord>,
}

impl Prefab {
    pub fn from_entity(world: &World, root_entity: impl Into<Entity>) -> anyhow::Result<Self> {
        let root_id = root_entity.into();
        if !world.is_alive(root_id) {
            anyhow::bail!("Entity {} does not exist", root_id);
        }

        let mut ordered_entities = vec![root_id];
        fn collect_descendants(world: &World, entity: Entity, out: &mut Vec<Entity>) {
            for child in world.get_children(entity) {
                out.push(child);
                collect_descendants(world, child, out);
            }
        }
        collect_descendants(world, root_id, &mut ordered_entities);

        let mut entity_to_index = HashMap::new();
        for (idx, &e) in ordered_entities.iter().enumerate() {
            entity_to_index.insert(e.0, idx);
        }

        let registry = world.serialization_registry.read().unwrap();
        let mut records = Vec::with_capacity(ordered_entities.len());

        for (idx, &entity) in ordered_entities.iter().enumerate() {
            let name = {
                let n = world.get_entity_name(entity);
                if n.is_empty() {
                    None
                } else {
                    Some(n)
                }
            };

            let parent_index = if idx == 0 {
                None
            } else {
                let e = world.inner.entity_from_id(entity.0);
                e.parent().and_then(|p| entity_to_index.get(&*p.id()).copied())
            };

            let mut components = BTreeMap::new();
            for reg in registry.iter() {
                if reg.name == "PrefabRef" {
                    continue;
                }
                if (reg.has)(world, entity) {
                    if let Some(val_str) = (reg.serialize)(world, entity) {
                        components.insert(reg.name.to_string(), val_str);
                    }
                }
            }

            records.push(EntityRecord {
                name,
                parent_index,
                components,
            });
        }

        Ok(Self {
            version: 1,
            entities: records,
        })
    }

    pub fn instantiate(
        &self,
        world: &World,
        override_position: Option<Vec3>,
        override_rotation: Option<Quat>,
        parent_id: Option<impl Into<Entity>>,
    ) -> anyhow::Result<Entity> {
        if self.entities.is_empty() {
            anyhow::bail!("Cannot instantiate an empty prefab");
        }

        let parent_entity: Option<Entity> = parent_id.map(|p| p.into());
        let registry = world.serialization_registry.read().unwrap().clone();
        let mut spawned_entities = Vec::with_capacity(self.entities.len());

        for record in &self.entities {
            let name = record.name.as_deref().unwrap_or("PrefabEntity");
            let entity = world.spawn_empty(name);
            spawned_entities.push(entity);
        }

        for (idx, record) in self.entities.iter().enumerate() {
            let entity_e = world.inner.entity_from_id(spawned_entities[idx].0);
            if idx == 0 {
                if let Some(pid) = parent_entity {
                    let parent_e = world.inner.entity_from_id(pid.0);
                    if parent_e.is_alive() {
                        entity_e.child_of(parent_e);
                    }
                }
            } else if let Some(parent_idx) = record.parent_index {
                if let Some(&p_entity) = spawned_entities.get(parent_idx) {
                    let parent_e = world.inner.entity_from_id(p_entity.0);
                    entity_e.child_of(parent_e);
                }
            }
        }

        for (idx, record) in self.entities.iter().enumerate() {
            let entity = spawned_entities[idx];
            for (comp_name, comp_str) in &record.components {
                if let Some(entry) = registry.get(comp_name.as_str()) {
                    if let Err(e) = (entry.deserialize)(world, entity, comp_str) {
                        log::warn!(
                            "[PREFAB] Failed to deserialize '{}' for prefab entity {:?}: {}",
                            comp_name,
                            record.name,
                            e
                        );
                    }
                }
            }
        }

        let root_id = spawned_entities[0];

        if override_position.is_some() || override_rotation.is_some() {
            world.modify(root_id, |t: &mut Transform| {
                if let Some(pos) = override_position {
                    t.position = pos;
                }
                if let Some(rot) = override_rotation {
                    t.rotation = rot;
                }
            });
        }

        world.update_entity_and_children_transforms(root_id);

        Ok(root_id)
    }

    pub fn save_to_file(&self, path: impl AsRef<Path>) -> anyhow::Result<()> {
        let pretty = ron::ser::PrettyConfig::default()
            .depth_limit(6)
            .struct_names(true);
        let ron_string = ron::ser::to_string_pretty(self, pretty)?;
        let mut file = File::create(path)?;
        file.write_all(ron_string.as_bytes())?;
        Ok(())
    }

    pub fn load_from_file(path: impl AsRef<Path>) -> anyhow::Result<Self> {
        let bytes = std::fs::read(path)?;
        let ron_str = std::str::from_utf8(&bytes)?;
        let prefab: Self = ron::from_str(ron_str)?;
        Ok(prefab)
    }
}