use super::{
    reflection::{RonDeserializer, RonSerializer},
    CameraComponent, Dynamic, EntityName, GlobalTransform, LightComponent, ModelComponent, RigidBodyComponent, ColliderComponent, CharacterControllerComponent,
    SceneEntity, Static, Transform, World, PrefabRef,
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
    #[serde(default = "default_layer_default")]
    pub render_layer: u32,
}

fn default_layer_default() -> u32 {
    crate::RenderLayers::DEFAULT
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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub character_controller: Option<CharacterControllerComponent>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub prefab_ref: Option<PrefabRef>,
    
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
            render_layer: m.render_layer,
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

        let prefab_ref = entity.try_get::<&PrefabRef>(|p| p.clone());
        let rigid_body = entity.try_get::<&RigidBodyComponent>(|r| *r);
        let collider = entity.try_get::<&ColliderComponent>(|c| *c);
        let character_controller = entity.try_get::<&CharacterControllerComponent>(|k| *k);

        records.push(EntityRecord {
            name,
            parent_index,
            transform,
            camera,
            model,
            light,
            rigid_body,
            collider,
            character_controller,
            prefab_ref,
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
                render_layer: m.render_layer,
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

        if let Some(kcc) = record.character_controller {
            entity.set(kcc);
        }

        if let Some(pref) = &record.prefab_ref {
            entity.set(pref.clone());
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

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Prefab {
    #[serde(default = "default_version")]
    pub version: u32,
    pub entities: Vec<EntityRecord>,
}

impl Prefab {
    pub fn from_entity(world: &World, root_entity_id: u64) -> anyhow::Result<Self> {
        let root_entity = world.inner.entity_from_id(root_entity_id);
        if !root_entity.is_alive() {
            anyhow::bail!("Entity {} does not exist", root_entity_id);
        }

        let mut ordered_ids = vec![root_entity_id];
        fn collect_descendants(world: &World, entity_id: u64, out: &mut Vec<u64>) {
            for child_id in world.get_children(entity_id) {
                out.push(child_id);
                collect_descendants(world, child_id, out);
            }
        }
        collect_descendants(world, root_entity_id, &mut ordered_ids);

        let mut id_to_index = HashMap::new();
        for (idx, &id) in ordered_ids.iter().enumerate() {
            id_to_index.insert(id, idx);
        }

        let dynamic_entries = world.get_component_entries();
        let mut records = Vec::with_capacity(ordered_ids.len());

        for (idx, &id) in ordered_ids.iter().enumerate() {
            let entity = world.inner.entity_from_id(id);
            let name = world.get_entity_name(id);

            let parent_index = if idx == 0 {
                None
            } else {
                entity
                    .parent()
                    .and_then(|p| id_to_index.get(&*p.id()).copied())
            };

            let transform = entity.try_get::<&Transform>(|t| (*t).into());
            let camera = entity.try_get::<&CameraComponent>(|c| *c);
            let model = entity.try_get::<&ModelComponent>(|m| SavedModel {
                path: m.id.path(),
                mesh_index: m.mesh_index,
                render_layer: m.render_layer,
            });
            let light = entity.try_get::<&LightComponent>(|l| l.clone());
            let rigid_body = entity.try_get::<&RigidBodyComponent>(|r| *r);
            let collider = entity.try_get::<&ColliderComponent>(|c| *c);
            let character_controller = entity.try_get::<&CharacterControllerComponent>(|k| *k);

            let mut custom_components = BTreeMap::new();
            for entry in &dynamic_entries {
                if (entry.has)(world, id) {
                    let mut comp_map = BTreeMap::new();
                    let mut serializer = RonSerializer { map: &mut comp_map };
                    (entry.inspect)(world, id, &mut serializer);
                    custom_components.insert(entry.name.to_string(), comp_map);
                }
            }

            records.push(EntityRecord {
                name,
                parent_index,
                transform,
                camera,
                model,
                light,
                rigid_body,
                collider,
                character_controller,
                prefab_ref: None,
                components: custom_components,
                is_static: entity.has(Static::id()),
                is_dynamic: entity.has(Dynamic::id()),
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
        parent_id: Option<u64>,
    ) -> anyhow::Result<u64> {
        if self.entities.is_empty() {
            anyhow::bail!("Cannot instantiate an empty prefab");
        }

        let dynamic_entries = world.get_component_entries();
        let mut spawned_ids = Vec::with_capacity(self.entities.len());

        for (idx, record) in self.entities.iter().enumerate() {
            let entity = world
                .inner
                .entity()
                .set(EntityName(record.name.clone()))
                .add(SceneEntity);

            let entity_id = *entity.id();

            let mut transform = record
                .transform
                .map(Transform::from)
                .unwrap_or_default();

            if idx == 0 {
                if let Some(pos) = override_position {
                    transform.position = pos;
                }
                if let Some(rot) = override_rotation {
                    transform.rotation = rot;
                }
            }

            entity.set(transform);
            entity.set(GlobalTransform(transform.compute_matrix()));

            if let Some(c) = record.camera { entity.set(c); }
            if let Some(m) = &record.model {
                entity.set(ModelComponent {
                    id: asset::AssetId::from_path(&m.path),
                    mesh_index: m.mesh_index,
                    render_layer: m.render_layer,
                });
            }
            if let Some(l) = &record.light { entity.set(l.clone()); }
            if let Some(rb) = record.rigid_body { entity.set(rb); }
            if let Some(col) = record.collider { entity.set(col); }
            if let Some(kcc) = record.character_controller { entity.set(kcc); }
            if let Some(pref) = &record.prefab_ref { entity.set(pref.clone()); }

            for (comp_name, comp_fields) in &record.components {
                if let Some(entry) = dynamic_entries.iter().find(|e| e.name == comp_name) {
                    (entry.add_default)(world, entity_id);
                    let mut deserializer = RonDeserializer {
                        entity_name: &record.name,
                        comp_name,
                        fields: comp_fields,
                    };
                    (entry.inspect)(world, entity_id, &mut deserializer);
                }
            }

            if record.is_static { entity.add(Static); }
            if record.is_dynamic { entity.add(Dynamic); }

            spawned_ids.push(entity_id);
        }

        // Восстанавливаем связи иерархии
        for (idx, record) in self.entities.iter().enumerate() {
            let entity = world.inner.entity_from_id(spawned_ids[idx]);
            if idx == 0 {
                if let Some(pid) = parent_id {
                    let parent = world.inner.entity_from_id(pid);
                    if parent.is_alive() {
                        entity.child_of(parent);
                    }
                }
            } else if let Some(parent_idx) = record.parent_index {
                if let Some(&p_id) = spawned_ids.get(parent_idx) {
                    let parent = world.inner.entity_from_id(p_id);
                    entity.child_of(parent);
                }
            }
        }

        let root_id = spawned_ids[0];
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