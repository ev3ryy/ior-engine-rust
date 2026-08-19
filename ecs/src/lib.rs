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
            to_delete.push(*entity.id());
        });

        for id in to_delete {
            self.inner.entity_from_id(id).destruct();
        }
    }

    pub fn each_model<F>(&self, mut f: F)
    where
        F: FnMut(&Position, &Rotation, &Scale, &ModelComponent),
    {
        self.inner.each::<(&Position, &Rotation, &Scale, &ModelComponent)>(
            |(pos, rot, scale, model)| f(pos, rot, scale, model)
        );
    }

    pub fn each_light<F>(&self, mut f: F)
    where
        F: FnMut(&Position, &Rotation, &LightComponent),
    {
        self.inner.each::<(&Position, &Rotation, &LightComponent)>(
            |(pos, rot, light)| f(pos, rot, light)
        );
    }

    pub fn each_transform_mut<F>(&self, mut f: F)
    where
        F: FnMut(u64, &mut Position, &mut Rotation, &mut Scale),
    {
        self.inner.each_entity::<(&mut Position, &mut Rotation, &mut Scale)>(
            |entity, (pos, rot, scale)| f(*entity.id(), pos, rot, scale)
        );
    }

    pub fn has_model(&self, entity_id: u64) -> bool {
        let mut has = false;
        self.inner.entity_from_id(entity_id).try_get::<&ModelComponent>(|_| has = true);
        has
    }

    pub fn get_model<F>(&self, entity_id: u64, mut f: F)
    where
        F: FnMut(&ModelComponent),
    {
        self.inner.entity_from_id(entity_id).try_get::<&ModelComponent>(|c| f(c));
    }

    pub fn has_light(&self, entity_id: u64) -> bool {
        let mut has = false;
        self.inner.entity_from_id(entity_id).try_get::<&LightComponent>(|_| has = true);
        has
    }

    pub fn get_light_mut<F>(&self, entity_id: u64, mut f: F)
    where
        F: FnMut(&mut LightComponent),
    {
        self.inner.entity_from_id(entity_id).try_get::<&mut LightComponent>(|c| f(c));
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
        parent_global_matrix: glam::Mat4, 
    ) -> flecs_ecs::core::EntityView<'a> {
        let node = &baked_model.nodes[node_idx];

        let local_pos = Vec3::from_array(node.position);
        let local_rot = Quat::from_array(node.rotation);
        let local_scale = Vec3::from_array(node.scale);

        let local_matrix = glam::Mat4::from_scale_rotation_translation(local_scale, local_rot, local_pos);
        let global_matrix = parent_global_matrix * local_matrix;
        let (global_scale, global_rot, global_pos) = global_matrix.to_scale_rotation_translation();

        let entity = world.inner.entity()
            .set(Position(global_pos))
            .set(Rotation(global_rot))
            .set(Scale(global_scale))
            .add(Static);

        if let Some(parent) = parent_entity {
            entity.child_of(parent);
        }

        for (i, &mesh_idx) in node.meshes.iter().enumerate() {
            if i == 0 {
                entity.set(ModelComponent {
                    id: asset_id,
                    mesh_index: mesh_idx,
                });
            } else {
                world.inner.entity()
                    .child_of(entity)
                    .set(Position(global_pos))
                    .set(Rotation(global_rot))
                    .set(Scale(global_scale))
                    .set(ModelComponent {
                        id: asset_id,
                        mesh_index: mesh_idx,
                    })
                    .add(Static);
            }
        }

        if let Some(light_idx) = node.light {
            let b_light = &baked_model.lights[light_idx];
            let light_type = match b_light.light_type {
                shared::BakedLightType::Directional => LightType::Directional,
                shared::BakedLightType::Point => LightType::Point,
                shared::BakedLightType::Spot => LightType::Spot,
            };

            entity.set(LightComponent {
                light_type,
                color: b_light.color,
                intensity: b_light.intensity, 
                range: b_light.range * global_scale.x,
                inner_angle: b_light.inner_angle,
                outer_angle: b_light.outer_angle,
            });
        }

        for &child_idx in &node.children {
            spawn_node(world, asset_id, baked_model, child_idx, Some(entity), global_matrix);
        }

        entity
    }

    let root_matrix = glam::Mat4::from_translation(root_position);
    
    for &root_idx in &baked_model.roots {
        spawn_node(world, asset_id, baked_model, root_idx, None, root_matrix);
    }
}

pub fn create_world() -> World {
    let world = flecs_ecs::core::World::new();
    
    world.component::<Position>();
    world.component::<Rotation>();
    world.component::<Scale>();
    world.component::<LightComponent>();
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