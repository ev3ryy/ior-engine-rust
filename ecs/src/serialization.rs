use super::{Position, Rotation, World};
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
}

pub fn save_world(world: &World, path: &str) -> anyhow::Result<()> {
    // Vec<u64> for compiler
    let mut entities: Vec<u64> = Vec::new();
    let mut positions = Vec::new();
    let mut rotations = Vec::new();

    world.inner.each_entity::<(&Position, &Rotation)>(|entity, (pos, rot)| {
        entities.push(*entity.id());
        positions.push(pos.0.to_array());
        rotations.push(rot.0.to_array());
    });

    let snapshot = WorldSnapshot {
        entities,
        positions,
        rotations,
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

        // O(1)
        let _archived = rkyv::access::<<WorldSnapshot as Archive>::Archived, Error>(&mmap)
            .map_err(|e| anyhow::anyhow!("Invalid binary snapshot format: {}", e))?;

        Ok(Self { _mmap: mmap })
    }

    pub fn instantiate_into_flecs(&self, world: &World) {
        let archived = rkyv::access::<<WorldSnapshot as Archive>::Archived, Error>(&self._mmap).unwrap();

        let count = archived.entities.len();

        for i in 0..count {
            let p = &archived.positions[i];
            let r = &archived.rotations[i];

            let p_arr: [f32; 3] = [p[0].to_native(), p[1].to_native(), p[2].to_native()];
            let r_arr: [f32; 4] = [r[0].to_native(), r[1].to_native(), r[2].to_native(), r[3].to_native()];

            world.inner
                .entity()
                .set(Position(Vec3::from_array(p_arr)))
                .set(Rotation(Quat::from_array(r_arr)));
        }
    }
}

pub fn load_world(world: &World, path: &str) -> anyhow::Result<()> {
    let mapped = MappedWorld::load(path)?;
    
    mapped.instantiate_into_flecs(world);
    
    Ok(())
}