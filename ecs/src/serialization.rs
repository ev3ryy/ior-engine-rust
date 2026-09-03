use super::{
    reflection::{RonDeserializer, RonSerializer},
    CameraComponent, Dynamic, EntityName, GlobalTransform, LightComponent, ModelComponent, RigidBodyComponent, ColliderComponent,
    SceneEntity, Static, Transform, World,
};
use flecs_ecs::prelude::*;
use glam::{Quat, Vec3};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap};
use std::fs::File;
use std::io::Write;
use std::path::Path;

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq)]
pub struct SavedTransform {
    #[serde(default)]
    pub position: [f32; 3],
    #[serde(default = "default_quat")]
    pub rotation: [f32; 4],
    #[serde(default = "default_scale")]
    pub scale: [f32; 3],
}

fn default_quat() -> [f32; 4] { [0.0, 0.0, 0.0, 1.0] }
fn default_scale() -> [f32; 3] { [1.0, 1.0, 1.0] }

impl From<Transform> for SavedTransform {
    fn from(t: Transform) -> Self {
        Self {
            position: t.position.to_array(),
            rotation: t.rotation.to_array(),
            scale: t.scale.to_array(),
        }
    }
}

impl From<SavedTransform> for Transform {
    fn from(s: SavedTransform) -> Self {
        Self {
            position: Vec3::from_array(s.position),
            rotation: Quat::from_array(s.rotation),
            scale: Vec3::from_array(s.scale),
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct SavedModel {
    pub path: String,
    #[serde(default)]
    pub mesh_index: usize,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct EntityRecord {
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent_index: Option<usize>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub transform: Option<SavedTransform>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub camera: Option<CameraComponent>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<SavedModel>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub light: Option<LightComponent>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rigid_body: Option<RigidBodyComponent>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub collider: Option<ColliderComponent>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub components: BTreeMap<String, BTreeMap<String, ron::Value>>,
    #[serde(default)]
    pub is_static: bool,
    #[serde(default)]
    pub is_dynamic: bool,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct WorldSnapshot {
    #[serde(default = "default_version")]
    pub version: u32,
    pub entities: Vec<EntityRecord>,
}

fn default_version() -> u32 { 1 }

pub fn create_world_snapshot(world: &World) -> anyhow::Result<Vec<u8>> {
    let mut flecs_id_to_index = HashMap::new();
    let mut entity_ids = Vec::new();

    world.inner.each_entity::<&EntityName>(|entity, _| {
        if entity.has(SceneEntity::id()) {
            flecs_id_to_index.insert(*entity.id(), entity_ids.len());
            entity_ids.push(*entity.id());
        }
    });

    let dynamic_entries = world.get_component_entries();
    let mut records = Vec::with_capacity(entity_ids.len());

    for &id in &entity_ids {
        let entity = world.inner.entity_from_id(id);
        let name = world.get_entity_name(id);

        let parent_index = entity
            .parent()
            .and_then(|p| flecs_id_to_index.get(&*p.id()).copied());

        let transform = entity.try_get::<&Transform>(|t| (*t).into());
        let camera = entity.try_get::<&CameraComponent>(|c| *c);
        let model = entity.try_get::<&ModelComponent>(|m| SavedModel {
            path: m.id.path(),
            mesh_index: m.mesh_index,
        });
        let light = entity.try_get::<&LightComponent>(|l| l.clone());

        let mut custom_components = BTreeMap::new();
        for entry in &dynamic_entries {
            if (entry.has)(world, id) {
                let mut comp_map = BTreeMap::new();
                let mut serializer = RonSerializer { map: &mut comp_map };
                (entry.inspect)(world, id, &mut serializer);
                custom_components.insert(entry.name.to_string(), comp_map);
            }
        }

        let is_static = entity.has(Static::id());
        let is_dynamic = entity.has(Dynamic::id());

        let rigid_body = entity.try_get::<&RigidBodyComponent>(|r| *r);
        let collider = entity.try_get::<&ColliderComponent>(|c| *c);

        records.push(EntityRecord {
            name,
            parent_index,
            transform,
            camera,
            model,
            light,
            rigid_body,
            collider,
            components: custom_components,
            is_static,
            is_dynamic,
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

    let dynamic_entries = world.get_component_entries();
    let mut spawned_entities = Vec::with_capacity(snapshot.entities.len());

    for record in &snapshot.entities {
        let entity = world
            .inner
            .entity()
            .set(EntityName(record.name.clone()))
            .add(SceneEntity);

        let entity_id = *entity.id();

        if let Some(st) = record.transform {
            entity.set(Transform::from(st));
            entity.set(GlobalTransform(Transform::from(st).compute_matrix()));
        }

        if let Some(c) = record.camera {
            entity.set(c);
        }

        if let Some(m) = &record.model {
            entity.set(ModelComponent {
                id: asset::AssetId::from_path(&m.path),
                mesh_index: m.mesh_index,
            });
        }

        if let Some(l) = &record.light {
            entity.set(l.clone());
        }

        if let Some(rb) = record.rigid_body {
            entity.set(rb);
        }

        if let Some(col) = record.collider {
            entity.set(col);
        }

        for (comp_name, comp_fields) in &record.components {
            if let Some(entry) = dynamic_entries.iter().find(|e| e.name == comp_name) {
                (entry.add_default)(world, entity_id);

                let mut deserializer = RonDeserializer {
                    entity_name: &record.name,
                    comp_name,
                    fields: comp_fields,
                };
                (entry.inspect)(world, entity_id, &mut deserializer);
            } else {
                log::warn!(
                    "[ECS] Entity '{}': Unknown component '{}' in save file. Skipping component.",
                    record.name,
                    comp_name
                );
            }
        }

        if record.is_static {
            entity.add(Static);
        }

        if record.is_dynamic {
            entity.add(Dynamic);
        }

        spawned_entities.push(entity);
    }

    for (i, record) in snapshot.entities.iter().enumerate() {
        if let Some(parent_idx) = record.parent_index {
            if let Some(parent_entity) = spawned_entities.get(parent_idx) {
                spawned_entities[i].child_of(*parent_entity);
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