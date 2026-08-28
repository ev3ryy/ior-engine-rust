pub mod serialization;
pub mod components;

pub use components::*; 

use flecs_ecs::prelude::*;

use glam::{Quat, Vec3, Mat4};
use asset::AssetId;
use shared::BakedModel;

fn make_unique_name(world: &flecs_ecs::core::World, parent: Option<flecs_ecs::core::EntityView>, base_name: &str) -> String {
    let base = if base_name.is_empty() { "Entity" } else { base_name };
    
    let exists = |name: &str| -> bool {
        if let Some(p) = parent {
            p.try_lookup(name).is_some()
        } else {
            world.try_lookup(name).is_some()
        }
    };

    if !exists(base) {
        return base.to_string();
    }

    let mut counter = 1;
    loop {
        let candidate = format!("{} ({})", base, counter);
        if !exists(&candidate) {
            return candidate;
        }
        counter += 1;
    }
}

pub struct World {
    pub inner: flecs_ecs::core::World,
}

impl World {
    pub fn clear_scene(&mut self) {
        let mut to_delete = Vec::new();

        self.inner.each_entity::<&SceneEntity>(|entity, _| {
            to_delete.push(*entity.id());
        });

        for id in to_delete {
            self.inner.entity_from_id(id).destruct();
        }
    }

    pub fn update_transforms(&self) {
        fn propagate_transform(entity: flecs_ecs::core::EntityView, parent_global: Mat4) {
            let mut current_global = parent_global;

            let mut has_transform = false;
            entity.try_get::<(&Position, &Rotation, &Scale)>(|(pos, rot, scale)| {
                let local_mat = Mat4::from_scale_rotation_translation(scale.0, rot.0, pos.0);
                current_global = parent_global * local_mat;
                has_transform = true;
            });

            if has_transform {
                entity.set(GlobalTransform(current_global));
            }

            entity.each_child(|child| {
                propagate_transform(child, current_global);
            });
        }

        self.inner.each_entity::<(&Position, &Rotation, &Scale)>(|entity, (pos, rot, scale)| {
            if entity.parent().is_none() {
                let root_matrix = Mat4::from_scale_rotation_translation(scale.0, rot.0, pos.0);
                entity.set(GlobalTransform(root_matrix));

                entity.each_child(|child| {
                    propagate_transform(child, root_matrix);
                });
            }
        });
    }

    pub fn spawn_empty(&self, name: &str) -> u64 {
        let unique_name = make_unique_name(&self.inner, None, name);
        let entity = self.inner.entity()
            .set_name(&unique_name)
            .set(SceneEntity(1));
        *entity.id()
    }

    pub fn spawn_child(&self, parent_id: u64, name: &str) -> u64 {
        let parent = self.inner.entity_from_id(parent_id);
        let unique_name = make_unique_name(&self.inner, Some(parent), name);
        let entity = self.inner.entity()
            .set_name(&unique_name)
            .child_of(parent)
            .set(SceneEntity(1));
        *entity.id()
    }

    pub fn delete_entity(&self, entity_id: u64) {
        let entity = self.inner.entity_from_id(entity_id);
        if entity.is_alive() {
            entity.destruct();
        }
    }

    pub fn is_alive(&self, entity_id: u64) -> bool {
        self.inner.entity_from_id(entity_id).is_alive()
    }

    pub fn set_entity_name(&self, entity_id: u64, name: &str) {
        let entity = self.inner.entity_from_id(entity_id);
        if entity.is_alive() {
            if entity.get_name().as_deref() != Some(name) {
                let unique_name = make_unique_name(&self.inner, entity.parent(), name);
                entity.set_name(&unique_name);
            }
        }
    }


    pub fn each_model<F>(&self, mut f: F)
    where
        F: FnMut(&GlobalTransform, &ModelComponent),
    {
        self.inner.each::<(&GlobalTransform, &ModelComponent)>(
            |(global, model)| f(global, model)
        );
    }

    pub fn each_light<F>(&self, mut f: F)
    where
        F: FnMut(&Position, &Rotation, &LightComponent),
    {
        self.inner.each::<(&GlobalTransform, &LightComponent)>(
            |(global, light)| {
                let (_, rot, pos) = global.0.to_scale_rotation_translation();
                f(&Position(pos), &Rotation(rot), light);
            }
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

    pub fn get_root_entities(&self) -> Vec<u64> {
        let mut roots = Vec::new();
        self.inner.each_entity::<&SceneEntity>(|entity, _| {
            if entity.parent().is_none() {
                roots.push(*entity.id());
            }
        });
        roots
    }

    pub fn get_children(&self, entity_id: u64) -> Vec<u64> {
        let mut children = Vec::new();
        let entity = self.inner.entity_from_id(entity_id);
        if entity.is_alive() {
            entity.each_child(|child| {
                children.push(*child.id());
            });
        }
        children
    }

    pub fn has_children(&self, entity_id: u64) -> bool {
        let entity = self.inner.entity_from_id(entity_id);
        if entity.is_alive() {
            entity.has_children()
        } else {
            false
        }
    }

    pub fn get_entity_name(&self, entity_id: u64) -> String {
        let entity = self.inner.entity_from_id(entity_id);
        if entity.is_alive() {
            entity.get_name().unwrap_or_default()
        } else {
            String::new()
        }
    }

    // --- Transform Component ---
    pub fn has_transform(&self, entity_id: u64) -> bool {
        let mut has = false;
        self.inner.entity_from_id(entity_id).try_get::<(&Position, &Rotation, &Scale)>(|_| has = true);
        has
    }

    pub fn add_transform(&self, entity_id: u64, pos: Vec3, rot: Quat, scale: Vec3) {
        let entity = self.inner.entity_from_id(entity_id);
        if entity.is_alive() {
            let local_mat = Mat4::from_scale_rotation_translation(scale, rot, pos);
            let parent_global = if let Some(parent) = entity.parent() {
                let mut p = Mat4::IDENTITY;
                parent.try_get::<&GlobalTransform>(|g| p = g.0);
                p
            } else {
                Mat4::IDENTITY
            };
            let global_mat = parent_global * local_mat;

            entity.set(Position(pos))
                .set(Rotation(rot))
                .set(Scale(scale))
                .set(GlobalTransform(global_mat))
                .add(Static);
        }
    }

    pub fn remove_transform(&self, entity_id: u64) {
        let entity = self.inner.entity_from_id(entity_id);
        if entity.is_alive() {
            entity.remove(Position::id());
            entity.remove(Rotation::id());
            entity.remove(Scale::id());
            entity.remove(GlobalTransform::id());
        }
    }

    pub fn get_transform_mut<F>(&self, entity_id: u64, mut f: F)
    where
        F: FnMut(&mut Position, &mut Rotation, &mut Scale),
    {
        self.inner.entity_from_id(entity_id).try_get::<(&mut Position, &mut Rotation, &mut Scale)>(|(p, r, s)| {
            f(p, r, s);
        });
    }

    pub fn get_global_transform(&self, entity_id: u64) -> Option<Mat4> {
        let mut mat = None;
        let entity = self.inner.entity_from_id(entity_id);
        if entity.is_alive() {
            entity.try_get::<&GlobalTransform>(|g| mat = Some(g.0));
        }
        mat
    }

    pub fn set_transform_from_global_matrix(&self, entity_id: u64, new_global: Mat4) {
        let entity = self.inner.entity_from_id(entity_id);
        if !entity.is_alive() { return; }

        let parent_global = if let Some(parent) = entity.parent() {
            let mut p_mat = Mat4::IDENTITY;
            parent.try_get::<&GlobalTransform>(|g| p_mat = g.0);
            p_mat
        } else {
            Mat4::IDENTITY
        };

        let local_matrix = parent_global.inverse() * new_global;
        let (scale, rot, pos) = local_matrix.to_scale_rotation_translation();

        entity.try_get::<(&mut Position, &mut Rotation, &mut Scale)>(|(p, r, s)| {
            p.0 = pos;
            r.0 = rot;
            s.0 = scale;
        });
        entity.set(GlobalTransform(new_global));
    }

    // --- Model Component ---
    pub fn has_model(&self, entity_id: u64) -> bool {
        let mut has = false;
        self.inner.entity_from_id(entity_id).try_get::<&ModelComponent>(|_| has = true);
        has
    }

    pub fn add_model(&self, entity_id: u64, model: ModelComponent) {
        let entity = self.inner.entity_from_id(entity_id);
        if entity.is_alive() {
            if !self.has_transform(entity_id) {
                self.add_transform(entity_id, Vec3::ZERO, Quat::IDENTITY, Vec3::ONE);
            }
            entity.set(model);
        }
    }

    pub fn remove_model(&self, entity_id: u64) {
        let entity = self.inner.entity_from_id(entity_id);
        if entity.is_alive() {
            entity.remove(ModelComponent::id());
        }
    }

    pub fn get_model<F>(&self, entity_id: u64, mut f: F)
    where
        F: FnMut(&ModelComponent),
    {
        self.inner.entity_from_id(entity_id).try_get::<&ModelComponent>(|c| f(c));
    }

    // --- Light Component ---
    pub fn has_light(&self, entity_id: u64) -> bool {
        let mut has = false;
        self.inner.entity_from_id(entity_id).try_get::<&LightComponent>(|_| has = true);
        has
    }

    pub fn add_light(&self, entity_id: u64, light: LightComponent) {
        let entity = self.inner.entity_from_id(entity_id);
        if entity.is_alive() {
            if !self.has_transform(entity_id) {
                self.add_transform(entity_id, Vec3::ZERO, Quat::IDENTITY, Vec3::ONE);
            }
            entity.set(light);
        }
    }

    pub fn remove_light(&self, entity_id: u64) {
        let entity = self.inner.entity_from_id(entity_id);
        if entity.is_alive() {
            entity.remove(LightComponent::id());
        }
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
) -> u64 {
    let model_name = asset_id.path();
    let clean_name = std::path::Path::new(&model_name)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("ModelRoot");

    let root_matrix = Mat4::from_translation(root_position);

    let model_root = world.inner.entity()
        .set_name(clean_name)
        .set(Position(root_position))
        .set(Rotation(Quat::IDENTITY))
        .set(Scale(Vec3::ONE))
        .set(GlobalTransform(root_matrix))
        .set(SceneEntity(1))
        .add(Static);

    fn spawn_node_recursive<'a>(
        world: &'a World,
        asset_id: AssetId,
        baked_model: &BakedModel,
        node_idx: usize,
        parent: flecs_ecs::core::EntityView<'a>,
        parent_global_matrix: Mat4,
        model_name: &str,
    ) {
        let node = &baked_model.nodes[node_idx];

        let local_pos = Vec3::from_array(node.position);
        let local_rot = Quat::from_array(node.rotation);
        let local_scale = Vec3::from_array(node.scale);

        let local_matrix = Mat4::from_scale_rotation_translation(local_scale, local_rot, local_pos);
        let global_matrix = parent_global_matrix * local_matrix;

        let node_name = if node.name.is_empty() {
            format!("Node_{}", node_idx)
        } else {
            node.name.clone()
        };

        let unique_node_name = make_unique_name(&world.inner, Some(parent), &node_name);

        let node_entity = world.inner.entity()
            .set_name(&unique_node_name)
            .child_of(parent) 
            .set(Position(local_pos))
            .set(Rotation(local_rot))
            .set(Scale(local_scale))
            .set(GlobalTransform(global_matrix))
            .set(SceneEntity(1))
            .add(Static);

        for (i, &mesh_idx) in node.meshes.iter().enumerate() {
            let mesh_name = baked_model.meshes.get(mesh_idx)
                .map(|m| m.name.as_str())
                .unwrap_or("Mesh");

            if node.meshes.len() == 1 {
                node_entity.set(ModelComponent {
                    id: asset_id,
                    mesh_index: mesh_idx,
                });
            } else {
                let unique_mesh_name = make_unique_name(&world.inner, Some(node_entity), &format!("{}_{}", mesh_name, i));
                world.inner.entity()
                    .set_name(&unique_mesh_name)
                    .child_of(node_entity)
                    .set(Position(Vec3::ZERO))
                    .set(Rotation(Quat::IDENTITY))
                    .set(Scale(Vec3::ONE))
                    .set(GlobalTransform(global_matrix))
                    .set(ModelComponent {
                        id: asset_id,
                        mesh_index: mesh_idx,
                    })
                    .set(SceneEntity(1))
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

            let (global_scale, global_rot, global_pos) = global_matrix.to_scale_rotation_translation();

            let light_component = LightComponent {
                light_type,
                color: b_light.color,
                intensity: b_light.intensity,
                range: b_light.range * global_scale.x,
                inner_angle: b_light.inner_angle,
                outer_angle: b_light.outer_angle,
            };

            if light_type == LightType::Directional {
                let unique_light_name = make_unique_name(&world.inner, None, &format!("DirectionalLight_{}", model_name));

                world.inner.entity()
                    .set_name(&unique_light_name)
                    .set(Position(global_pos))
                    .set(Rotation(global_rot))
                    .set(Scale(global_scale))
                    .set(GlobalTransform(global_matrix))
                    .set(light_component)
                    .set(SceneEntity(1))
                    .add(Static);
            } else {
                let unique_light_name = make_unique_name(&world.inner, Some(node_entity), &format!("Light_{}", node_name));

                world.inner.entity()
                    .set_name(&unique_light_name)
                    .child_of(node_entity)
                    .set(Position(Vec3::ZERO))
                    .set(Rotation(Quat::IDENTITY))
                    .set(Scale(Vec3::ONE))
                    .set(GlobalTransform(global_matrix))
                    .set(light_component)
                    .set(SceneEntity(1))
                    .add(Static);
            }
        }

        for &child_idx in &node.children {
            spawn_node_recursive(world, asset_id, baked_model, child_idx, node_entity, global_matrix, model_name);
        }
    }

    for &root_idx in &baked_model.roots {
        spawn_node_recursive(world, asset_id, baked_model, root_idx, model_root, root_matrix, clean_name);
    }

    *model_root.id()
}

pub fn create_world() -> World {
    let world = flecs_ecs::core::World::new();
    
    world.component::<Position>();
    world.component::<Rotation>();
    world.component::<Scale>();
    world.component::<GlobalTransform>();
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
                .set(GlobalTransform(Mat4::from_translation(Vec3::new(px, 0.0, pz))))
                .set(ModelComponent { id: model_id, mesh_index: 0})
                .add(Static);
        }
    }
}