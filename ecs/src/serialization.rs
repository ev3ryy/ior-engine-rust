use super::{Position, Rotation, Scale, LightComponent, ModelComponent, World};
use asset::AssetId;
use flecs_ecs::prelude::*;
use glam::{Quat, Vec3};
use memmap2::Mmap;
use rkyv::{Archive, Deserialize, Serialize, rancor::Error};
use std::fs::File;
use std::io::Write;

#[derive(Archive, Serialize, Deserialize)]
pub struct WorldSnapshot {
    pub entities: Vec<u64>,
    pub positions: Vec<[f32; 3]>,
    pub rotations: Vec<[f32; 4]>,
    pub scales: Vec<[f32; 3]>,
    pub model_ids: Vec<AssetId>, 
    pub mesh_indices: Vec<usize>,

    pub light_entities: Vec<u64>,
    pub light_positions: Vec<[f32; 3]>,
    pub light_rotations: Vec<[f32; 4]>,
    pub lights: Vec<LightComponent>,
}

pub fn save_world(world: &World, path: &str) -> anyhow::Result<()> {
    let mut entities = Vec::new();
    let mut positions = Vec::new();
    let mut rotations = Vec::new();
    let mut scales = Vec::new();
    let mut model_ids = Vec::new();
    let mut mesh_indices = Vec::new();

    world.inner.each_entity::<(&Position, &Rotation, &Scale, &ModelComponent)>(|entity, (pos, rot, scale, model)| {
        entities.push(*entity.id());
        positions.push(pos.0.to_array());
        rotations.push(rot.0.to_array());
        scales.push(scale.0.to_array());
        model_ids.push(model.id);
        mesh_indices.push(model.mesh_index);
    });

    let mut light_entities = Vec::new();
    let mut light_positions = Vec::new();
    let mut light_rotations = Vec::new();
    let mut lights = Vec::new();

    world.inner.each_entity::<(&Position, &Rotation, &LightComponent)>(|entity, (pos, rot, light)| {
        light_entities.push(*entity.id());
        light_positions.push(pos.0.to_array());
        light_rotations.push(rot.0.to_array());
        lights.push(light.clone());
    });

    let snapshot = WorldSnapshot {
        entities,
        positions,
        rotations,
        scales,
        model_ids,
        mesh_indices,
        light_entities,
        light_positions,
        light_rotations,
        lights,
    };

    let bytes = rkyv::to_bytes::<Error>(&snapshot)
        .map_err(|e| anyhow::anyhow!("Serialization failed: {}", e))?;

    let mut file = File::create(path)?;
    file.write_all(&bytes)?;

    Ok(())
}

pub struct MappedWorld {
    _mmap: Mmap,
}

impl MappedWorld {
    pub fn load(path: &str) -> anyhow::Result<Self> {
        let file = File::open(path)?;
        let mmap = unsafe { Mmap::map(&file)? };

        let _archived = rkyv::access::<<WorldSnapshot as Archive>::Archived, Error>(&mmap)
            .map_err(|e| anyhow::anyhow!("Invalid binary snapshot format: {}", e))?;

        Ok(Self { _mmap: mmap })
    }

    pub fn instantiate_into_flecs(&self, world: &World) {
        let archived = rkyv::access::<<WorldSnapshot as Archive>::Archived, Error>(&self._mmap).unwrap();
        
        let snapshot = rkyv::deserialize::<WorldSnapshot, Error>(archived).unwrap();

        let count = snapshot.entities.len();

        for i in 0..count {
            let p = snapshot.positions[i];
            let r = snapshot.rotations[i];
            let s = snapshot.scales[i];
            let m_id = snapshot.model_ids[i];
            let m_idx = snapshot.mesh_indices[i];

            world.inner
                .entity()
                .set(Position(Vec3::from_array(p)))
                .set(Rotation(Quat::from_array(r)))
                .set(Scale(Vec3::from_array(s)))
                .set(ModelComponent { id: m_id, mesh_index: m_idx })
                .add(super::Static);
        }

        let light_count = snapshot.light_entities.len();
        for i in 0..light_count {
            world.inner
                .entity()
                .set(Position(Vec3::from_array(snapshot.light_positions[i])))
                .set(Rotation(Quat::from_array(snapshot.light_rotations[i])))
                .set(snapshot.lights[i].clone())
                .add(super::Static);
        }
    }
}

pub fn load_world(world: &World, path: &str) -> anyhow::Result<()> {
    let mapped = MappedWorld::load(path)?;
    
    mapped.instantiate_into_flecs(world);
    
    Ok(())
}