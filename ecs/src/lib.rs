pub mod components;
pub mod serialization;

pub use components::*;

use asset::AssetId;
use flecs_ecs::prelude::*;
use glam::{Mat4, Quat, Vec3};
use shared::BakedModel;

pub struct World {
    pub inner: flecs_ecs::core::World,
}

impl World {
    // --- Scene Lifecycle ---

    pub fn clear_scene(&mut self) {
        let mut root_entities = Vec::new();

        // Прямой проход без аллокаций query!
        self.inner.each_entity::<&Transform>(|entity, _| {
            if entity.has(SceneEntity::id()) && entity.parent().is_none() {
                root_entities.push(*entity.id());
            }
        });

        for id in root_entities {
            let entity = self.inner.entity_from_id(id);
            if entity.is_alive() {
                entity.destruct();
            }
        }
    }

    pub fn spawn_empty(&self, name: &str) -> u64 {
        let entity = self.inner.entity().set_name(name).add(SceneEntity);
        *entity.id()
    }

    pub fn spawn_child(&self, parent_id: u64, name: &str) -> u64 {
        let parent = self.inner.entity_from_id(parent_id);
        let entity = self
            .inner
            .entity()
            .set_name(name)
            .child_of(parent)
            .add(SceneEntity);
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

    // --- Entity Hierarchy & Naming ---

    pub fn get_entity_name(&self, entity_id: u64) -> String {
        let entity = self.inner.entity_from_id(entity_id);
        if entity.is_alive() {
            entity.get_name().unwrap_or_default()
        } else {
            String::new()
        }
    }

    pub fn set_entity_name(&self, entity_id: u64, name: &str) {
        let entity = self.inner.entity_from_id(entity_id);
        if entity.is_alive() {
            entity.set_name(name);
        }
    }

    pub fn get_root_entities(&self) -> Vec<u64> {
        let mut roots = Vec::new();
        self.inner.each_entity::<&Transform>(|entity, _| {
            if entity.has(SceneEntity::id()) && entity.parent().is_none() {
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

    // --- Transform Management ---

    pub fn has_transform(&self, entity_id: u64) -> bool {
        let entity = self.inner.entity_from_id(entity_id);
        entity.is_alive() && entity.has(Transform::id())
    }

    pub fn add_transform(&self, entity_id: u64, transform: Transform) {
        let entity = self.inner.entity_from_id(entity_id);
        if entity.is_alive() {
            let parent_global = if let Some(parent) = entity.parent() {
                let mut p = Mat4::IDENTITY;
                parent.try_get::<&GlobalTransform>(|g| p = g.0);
                p
            } else {
                Mat4::IDENTITY
            };
            let global_mat = parent_global * transform.compute_matrix();

            entity
                .set(transform)
                .set(GlobalTransform(global_mat))
                .add(Static);
        }
    }

    pub fn remove_transform(&self, entity_id: u64) {
        let entity = self.inner.entity_from_id(entity_id);
        if entity.is_alive() {
            entity.remove(Transform::id());
            entity.remove(GlobalTransform::id());
        }
    }

    pub fn get_transform_mut<F>(&self, entity_id: u64, mut f: F)
    where
        F: FnMut(&mut Transform),
    {
        let entity = self.inner.entity_from_id(entity_id);
        if entity.is_alive() {
            entity.try_get::<&mut Transform>(|t| f(t));
        }
    }

    pub fn get_global_transform(&self, entity_id: u64) -> Option<Mat4> {
        let entity = self.inner.entity_from_id(entity_id);
        if entity.is_alive() {
            let mut mat = None;
            entity.try_get::<&GlobalTransform>(|g| mat = Some(g.0));
            mat
        } else {
            None
        }
    }

    pub fn set_transform_from_global_matrix(&self, entity_id: u64, new_global: Mat4) {
        let entity = self.inner.entity_from_id(entity_id);
        if !entity.is_alive() {
            return;
        }

        let parent_global = if let Some(parent) = entity.parent() {
            let mut p_mat = Mat4::IDENTITY;
            parent.try_get::<&GlobalTransform>(|g| p_mat = g.0);
            p_mat
        } else {
            Mat4::IDENTITY
        };

        let local_matrix = parent_global.inverse() * new_global;
        let (scale, rotation, position) = local_matrix.to_scale_rotation_translation();

        entity.set(Transform {
            position,
            rotation,
            scale,
        });
        entity.set(GlobalTransform(new_global));
        entity.add(Dynamic);
    }

    // БЫСТРОЕ ОБНОВЛЕНИЕ БЕЗ АЛЛОКАЦИЙ
    pub fn update_transforms(&self) {
        fn propagate_dynamic(entity: flecs_ecs::core::EntityView, parent_global: Mat4) {
            let mut current_global = parent_global;

            entity.try_get::<&Transform>(|transform| {
                current_global = parent_global * transform.compute_matrix();
                entity.set(GlobalTransform(current_global));
            });

            entity.each_child(|child| {
                propagate_dynamic(child, current_global);
            });
        }

        self.inner
            .each_entity::<&Transform>(|entity, transform| {
                // Обновляем только если сущность динамическая и в корне
                if entity.has(Dynamic::id()) && entity.parent().is_none() {
                    let root_matrix = transform.compute_matrix();
                    entity.set(GlobalTransform(root_matrix));

                    entity.each_child(|child| {
                        propagate_dynamic(child, root_matrix);
                    });
                }
            });
    }

    // --- Light Component Management ---

    pub fn has_light(&self, entity_id: u64) -> bool {
        let entity = self.inner.entity_from_id(entity_id);
        entity.is_alive() && entity.has(LightComponent::id())
    }

    pub fn add_light(&self, entity_id: u64, light: LightComponent) {
        let entity = self.inner.entity_from_id(entity_id);
        if entity.is_alive() {
            if !self.has_transform(entity_id) {
                self.add_transform(entity_id, Transform::default());
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
        let entity = self.inner.entity_from_id(entity_id);
        if entity.is_alive() {
            entity.try_get::<&mut LightComponent>(|l| f(l));
        }
    }

    // --- Model Component Management ---

    pub fn has_model(&self, entity_id: u64) -> bool {
        let entity = self.inner.entity_from_id(entity_id);
        entity.is_alive() && entity.has(ModelComponent::id())
    }

    pub fn get_model<F>(&self, entity_id: u64, mut f: F)
    where
        F: FnMut(&ModelComponent),
    {
        let entity = self.inner.entity_from_id(entity_id);
        if entity.is_alive() {
            entity.try_get::<&ModelComponent>(|m| f(m));
        }
    }

    pub fn remove_model(&self, entity_id: u64) {
        let entity = self.inner.entity_from_id(entity_id);
        if entity.is_alive() {
            entity.remove(ModelComponent::id());
        }
    }

    // --- Render Queries ---

    pub fn each_model<F>(&self, mut f: F)
    where
        F: FnMut(&GlobalTransform, &ModelComponent),
    {
        self.inner
            .each::<(&GlobalTransform, &ModelComponent)>(|(global, model)| f(global, model));
    }

    pub fn each_light<F>(&self, mut f: F)
    where
        F: FnMut(Vec3, Quat, &LightComponent),
    {
        self.inner
            .each::<(&GlobalTransform, &LightComponent)>(|(global, light)| {
                let (_, rot, pos) = global.0.to_scale_rotation_translation();
                f(pos, rot, light);
            });
    }
}

pub fn create_world() -> World {
    let world = flecs_ecs::core::World::new();

    world.component::<Transform>();
    world.component::<GlobalTransform>();
    world.component::<LightComponent>();
    world.component::<ModelComponent>();
    world.component::<Static>();
    world.component::<Dynamic>();
    world.component::<SceneEntity>();

    World { inner: world }
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

    let root_transform = Transform::from_translation(root_position);
    let root_matrix = root_transform.compute_matrix();

    let model_root = world
        .inner
        .entity()
        .set_name(clean_name)
        .set(root_transform)
        .set(GlobalTransform(root_matrix))
        .add(SceneEntity)
        .add(Static);

    fn spawn_node_recursive<'a>(
        world: &'a World,
        asset_id: AssetId,
        baked_model: &BakedModel,
        node_idx: usize,
        parent: flecs_ecs::core::EntityView<'a>,
        parent_global_matrix: Mat4,
    ) {
        let node = &baked_model.nodes[node_idx];

        let local_transform = Transform {
            position: Vec3::from_array(node.position),
            rotation: Quat::from_array(node.rotation),
            scale: Vec3::from_array(node.scale),
        };

        let global_matrix = parent_global_matrix * local_transform.compute_matrix();

        let node_name = if node.name.is_empty() {
            format!("Node_{}", node_idx)
        } else {
            node.name.clone()
        };

        let node_entity = world
            .inner
            .entity()
            .set_name(&node_name)
            .child_of(parent)
            .set(local_transform)
            .set(GlobalTransform(global_matrix))
            .add(SceneEntity)
            .add(Static);

        for (i, &mesh_idx) in node.meshes.iter().enumerate() {
            let mesh_name = baked_model
                .meshes
                .get(mesh_idx)
                .map(|m| m.name.as_str())
                .unwrap_or("Mesh");

            if node.meshes.len() == 1 {
                node_entity.set(ModelComponent {
                    id: asset_id,
                    mesh_index: mesh_idx,
                });
            } else {
                let child_mesh_name = format!("{}_{}", mesh_name, i);
                world
                    .inner
                    .entity()
                    .set_name(&child_mesh_name)
                    .child_of(node_entity)
                    .set(Transform::default())
                    .set(GlobalTransform(global_matrix))
                    .set(ModelComponent {
                        id: asset_id,
                        mesh_index: mesh_idx,
                    })
                    .add(SceneEntity)
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

            let (global_scale, _, _) = global_matrix.to_scale_rotation_translation();

            let light_component = LightComponent {
                light_type,
                color: b_light.color,
                intensity: b_light.intensity,
                range: b_light.range * global_scale.x,
                inner_angle: b_light.inner_angle,
                outer_angle: b_light.outer_angle,
            };

            let light_name = format!("Light_{}", node_name);
            world
                .inner
                .entity()
                .set_name(&light_name)
                .child_of(node_entity)
                .set(Transform::default())
                .set(GlobalTransform(global_matrix))
                .set(light_component)
                .add(SceneEntity)
                .add(Static);
        }

        for &child_idx in &node.children {
            spawn_node_recursive(
                world,
                asset_id,
                baked_model,
                child_idx,
                node_entity,
                global_matrix,
            );
        }
    }

    for &root_idx in &baked_model.roots {
        spawn_node_recursive(
            world,
            asset_id,
            baked_model,
            root_idx,
            model_root,
            root_matrix,
        );
    }

    *model_root.id()
}