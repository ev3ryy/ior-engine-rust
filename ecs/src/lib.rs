pub mod serialization;
pub mod components;

pub use components::*; 

use flecs_ecs::prelude::*;

use glam::{Quat, Vec3};
use asset::AssetId;
use shared::BakedModel;

pub struct World {
    pub inner: flecs_ecs::core::World,
}

impl World {
    pub fn clear_scene(&mut self) {
        let mut to_delete = Vec::new();
        
        self.inner.each_entity::<&ModelComponent>(|entity, _model| {
            to_delete.push(entity.id());
        });
        
        for id in to_delete {
            self.inner.entity_from_id(id).destruct();
        }
    }
}

pub fn spawn_scene(
    world: &World,
    asset_id: AssetId,
    baked_model: &BakedModel,
    root_position: Vec3,
) {
    fn spawn_node<'a>(
        world: &'a World,
        asset_id: AssetId,
        baked_model: &BakedModel,
        node_idx: usize,
        parent_entity: Option<flecs_ecs::core::EntityView<'a>>,
        global_offset: Vec3,
    ) -> flecs_ecs::core::EntityView<'a> {
        let node = &baked_model.nodes[node_idx];

        let pos = Vec3::from_array(node.position);
        let rot = Quat::from_array(node.rotation);
        let scale = Vec3::from_array(node.scale);

        let final_pos = if parent_entity.is_none() {
            pos + global_offset
        } else {
            pos
        };

        let entity = world.inner.entity()
            .set(Position(final_pos))
            .set(Rotation(rot))
            .set(Scale(scale))
            .add(Static);

        if !node.meshes.is_empty() {
            entity.set(ModelComponent {
                id: asset_id,
                mesh_index: node.meshes[0],
            });
        }

        if let Some(parent) = parent_entity {
            entity.child_of(parent);
        }

        for &child_idx in &node.children {
            spawn_node(world, asset_id, baked_model, child_idx, Some(entity), global_offset);
        }

        entity
    }

    for &root_idx in &baked_model.roots {
        spawn_node(world, asset_id, baked_model, root_idx, None, root_position);
    }
}

pub fn create_world() -> World {
    let world = flecs_ecs::core::World::new();
    
    world.component::<Position>();
    world.component::<Rotation>();
    world.component::<Scale>();
    world.component::<ModelComponent>();
    world.component::<Static>();
    world.component::<Dynamic>();

    World { inner: world }
}

pub fn spawn_instance_grid(world: &World, model_id: AssetId, rows: u32, spacing: f32) {
    let half = (rows as f32 - 1.0) * 0.5;
    for z in 0..rows {
        for x in 0..rows {
            let px = spacing * (x as f32 - half);
            let pz = spacing * (z as f32 - half);
            
            world.inner
                .entity()
                .set(Position(Vec3::new(px, 0.0, pz)))
                .set(Rotation(Quat::IDENTITY))
                .set(Scale(Vec3::ONE))
                .set(ModelComponent { id: model_id, mesh_index: 0})
                .add(Static);
        }
    }
}