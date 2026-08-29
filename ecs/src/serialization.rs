use super::{
    Dynamic, GlobalTransform, LightComponent, ModelComponent, SceneEntity, Static, Transform,
    World,
};
use flecs_ecs::prelude::*;
use glam::{Quat, Vec3};
use rkyv::{rancor::Error, Archive, Deserialize, Serialize};
use std::collections::HashMap;
use std::fs::File;
use std::io::Write;

#[derive(Archive, Serialize, Deserialize, Debug, Clone, Copy, PartialEq)]
pub struct SavedTransform {
    pub position: [f32; 3],
    pub rotation: [f32; 4],
    pub scale: [f32; 3],
}

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

#[derive(Archive, Serialize, Deserialize, Debug, Clone)]
pub struct EntityRecord {
    pub name: String,
    pub parent_index: Option<usize>,
    pub transform: Option<SavedTransform>,
    pub model: Option<ModelComponent>,
    pub light: Option<LightComponent>,
    pub is_static: bool,
    pub is_dynamic: bool,
}

#[derive(Archive, Serialize, Deserialize, Debug, Clone)]
pub struct WorldSnapshot {
    pub entities: Vec<EntityRecord>,
}

pub fn save_world(world: &World, path: &str) -> anyhow::Result<()> {
    let mut flecs_id_to_index = HashMap::new();
    let mut entity_ids = Vec::new();

    // Запрашиваем через query().with::<Tag>()
    world.inner.each_entity::<&Transform>(|entity, _| {
        if entity.has(SceneEntity::id()) {
            flecs_id_to_index.insert(*entity.id(), entity_ids.len());
            entity_ids.push(*entity.id());
        }
    });

    let mut records = Vec::with_capacity(entity_ids.len());

    for &id in &entity_ids {
        let entity = world.inner.entity_from_id(id);
        let name = entity.get_name().unwrap_or_default();

        let parent_index = entity
            .parent()
            .and_then(|p| flecs_id_to_index.get(&*p.id()).copied());

        let mut transform = None;
        entity.try_get::<&Transform>(|t| transform = Some((*t).into()));

        let mut model = None;
        entity.try_get::<&ModelComponent>(|m| model = Some(m.clone()));

        let mut light = None;
        entity.try_get::<&LightComponent>(|l| light = Some(l.clone()));

        let is_static = entity.has(Static::id());
        let is_dynamic = entity.has(Dynamic::id());

        records.push(EntityRecord {
            name,
            parent_index,
            transform,
            model,
            light,
            is_static,
            is_dynamic,
        });
    }

    let snapshot = WorldSnapshot { entities: records };

    let bytes = rkyv::to_bytes::<Error>(&snapshot)
        .map_err(|e| anyhow::anyhow!("Serialization failed: {}", e))?;

    let mut file = File::create(path)?;
    file.write_all(&bytes)?;

    Ok(())
}

pub fn load_world(world: &World, path: &str) -> anyhow::Result<()> {
    let bytes = std::fs::read(path)?;
    let snapshot = rkyv::from_bytes::<WorldSnapshot, Error>(&bytes)
        .map_err(|e| anyhow::anyhow!("Deserialization failed: {}", e))?;

    let mut spawned_entities = Vec::with_capacity(snapshot.entities.len());

    for record in &snapshot.entities {
        let entity = world.inner.entity().add(SceneEntity);

        if !record.name.is_empty() {
            entity.set_name(&record.name);
        }

        if let Some(st) = record.transform {
            entity.set(Transform::from(st));
        }

        if let Some(m) = &record.model {
            entity.set(m.clone());
        }

        if let Some(l) = &record.light {
            entity.set(l.clone());
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

    fn compute_hierarchy(entity: flecs_ecs::core::EntityView, parent_mat: glam::Mat4) {
        let mut current_mat = parent_mat;
        entity.try_get::<&Transform>(|t| {
            current_mat = parent_mat * t.compute_matrix();
            entity.set(GlobalTransform(current_mat));
        });

        entity.each_child(|child| {
            compute_hierarchy(child, current_mat);
        });
    }

    for entity in &spawned_entities {
        if entity.parent().is_none() {
            let mut root_mat = glam::Mat4::IDENTITY;
            entity.try_get::<&Transform>(|t| {
                root_mat = t.compute_matrix();
                entity.set(GlobalTransform(root_mat));
            });

            entity.each_child(|child| {
                compute_hierarchy(child, root_mat);
            });
        }
    }

    Ok(())
}