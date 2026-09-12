pub mod components;
pub mod reflection;
pub mod serialization;

pub use components::*;
pub use reflection::*;
pub use inventory;

pub use flecs_ecs::{self, prelude::*};
pub use flecs_ecs_derive::Component;
pub use ecs_macros::Inspectable;

pub trait EntityExt {
    fn raw_id(&self) -> u64;
}

impl<'a> EntityExt for flecs_ecs::core::EntityView<'a> {
    #[inline]
    fn raw_id(&self) -> u64 {
        *self.id()
    }
}

pub mod prelude {
    pub use crate::flecs_ecs; 
    pub use flecs_ecs::prelude::*;
    pub use flecs_ecs_derive::Component;
    pub use ecs_macros::Inspectable;
    pub use crate::components::*;
    pub use crate::reflection::*;
    pub use crate::EntityExt;
    pub use crate::{create_world, World};
}

use asset::AssetId;
use glam::{Mat4, Quat, Vec3};
use shared::BakedModel;
use std::ops::{Deref, DerefMut};
use std::sync::{Arc, RwLock};

pub struct World {
    pub inner: flecs_ecs::core::World,
    pub component_entries: Arc<RwLock<Vec<DynamicComponentEntry>>>,
}

impl Deref for World {
    type Target = flecs_ecs::core::World;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

impl DerefMut for World {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.inner
    }
}

impl World {
    // --- Dynamic Component Reflection & Registration ---

    pub fn register_component<T>(&self)
    where
        T: Inspectable + Default + Clone + ComponentId + DataComponent,
    {
        self.register_component_internal::<T>(T::NAME, T::CATEGORY);
    }

    pub fn register_component_internal<T>(&self, name: &'static str, category: &'static str)
    where
        T: Inspectable + Default + Clone + ComponentId + DataComponent,
    {
        self.inner.component::<T>();

        let mut entries = self.component_entries.write().unwrap();
        if entries.iter().any(|e| e.name == name) {
            return;
        }

        entries.push(DynamicComponentEntry {
            name,
            category,
            add_default: Arc::new(|world, id| {
                let entity = world.inner.entity_from_id(id);
                if entity.is_alive() {
                    entity.set(T::default());
                }
            }),
            has: Arc::new(|world, id| {
                let entity = world.inner.entity_from_id(id);
                entity.is_alive() && entity.has(T::id())
            }),
            remove: Arc::new(|world, id| {
                let entity = world.inner.entity_from_id(id);
                if entity.is_alive() && entity.has(T::id()) {
                    entity.remove(T::id());
                }
            }),
            inspect: Arc::new(|world, id, visitor| {
                let entity = world.inner.entity_from_id(id);
                if entity.is_alive() && entity.has(T::id()) {
                    entity.try_get::<&mut T>(|comp| {
                        comp.inspect_fields(visitor);
                    });
                }
            }),
        });
    }

    pub fn auto_register_components(&self) {
        for entry in inventory::iter::<ComponentRegistryEntry> {
            (entry.register_fn)(self);
        }
    }

    pub fn get_component_entries(&self) -> Vec<DynamicComponentEntry> {
        self.component_entries.read().unwrap().clone()
    }

    // --- Snapshots & Execution ---
    pub fn create_snapshot(&self) -> anyhow::Result<Vec<u8>> {
        serialization::create_world_snapshot(self)
    }

    pub fn restore_snapshot(&mut self, bytes: &[u8]) -> anyhow::Result<()> {
        serialization::restore_world_snapshot(self, bytes)
    }

    pub fn progress(&self) {
        self.inner.progress();
    }

    // --- Scene Lifecycle ---

    pub fn clear_scene(&mut self) {
        let mut root_entities = Vec::new();

        self.inner.each_entity::<&EntityName>(|entity, _| {
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
        let entity = self
            .inner
            .entity()
            .set(EntityName(name.to_string()))
            .add(SceneEntity);
        *entity.id()
    }

    pub fn spawn_child(&self, parent_id: u64, name: &str) -> u64 {
        let parent = self.inner.entity_from_id(parent_id);
        let entity = self
            .inner
            .entity()
            .child_of(parent)
            .set(EntityName(name.to_string()))
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
        if !entity.is_alive() {
            return String::new();
        }

        if let Some(name) = entity.try_get::<&EntityName>(|n| n.0.clone()) {
            return name;
        }

        entity.get_name().unwrap_or_default()
    }

    pub fn set_entity_name(&self, entity_id: u64, name: &str) {
        let entity = self.inner.entity_from_id(entity_id);
        if entity.is_alive() {
            entity.set(EntityName(name.to_string()));
        }
    }

    pub fn get_root_entities(&self) -> Vec<u64> {
        let mut roots = Vec::new();
        self.inner.each_entity::<&EntityName>(|entity, _| {
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
                if child.has(SceneEntity::id()) {
                    children.push(*child.id());
                }
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
            let parent_global = entity
                .parent()
                .and_then(|parent| parent.try_get::<&GlobalTransform>(|g| g.0))
                .unwrap_or(Mat4::IDENTITY);

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
            entity.try_get::<&GlobalTransform>(|g| g.0)
        } else {
            None
        }
    }

    pub fn set_transform_from_global_matrix(&self, entity_id: u64, new_global: Mat4) {
        let entity = self.inner.entity_from_id(entity_id);
        if !entity.is_alive() {
            return;
        }

        let parent_global = entity
            .parent()
            .and_then(|parent| parent.try_get::<&GlobalTransform>(|g| g.0))
            .unwrap_or(Mat4::IDENTITY);

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

    pub fn update_transforms(&self) {
        fn propagate(entity: flecs_ecs::core::EntityView, parent_global: Mat4) {
            let mut current_global = parent_global;

            if let Some(global) = entity.try_get::<&Transform>(|transform| {
                parent_global * transform.compute_matrix()
            }) {
                current_global = global;
                entity.set(GlobalTransform(current_global));
            }

            entity.each_child(|child| {
                propagate(child, current_global);
            });
        }

        self.inner.each_entity::<&EntityName>(|entity, _| {
            if entity.has(SceneEntity::id()) && entity.parent().is_none() {
                let mut root_matrix = Mat4::IDENTITY;
                if let Some(mat) = entity.try_get::<&Transform>(|transform| {
                    transform.compute_matrix()
                }) {
                    root_matrix = mat;
                    entity.set(GlobalTransform(root_matrix));
                }

                entity.each_child(|child| {
                    propagate(child, root_matrix);
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

    pub fn get_model_mut<F>(&self, entity_id: u64, mut f: F)
    where
        F: FnMut(&mut ModelComponent),
    {
        let entity = self.inner.entity_from_id(entity_id);
        if entity.is_alive() {
            entity.try_get::<&mut ModelComponent>(|m| f(m));
        }
    }

    pub fn remove_model(&self, entity_id: u64) {
        let entity = self.inner.entity_from_id(entity_id);
        if entity.is_alive() {
            entity.remove(ModelComponent::id());
        }
    }

    // --- Camera Component Management ---

    pub fn has_camera(&self, entity_id: u64) -> bool {
        let entity = self.inner.entity_from_id(entity_id);
        entity.is_alive() && entity.has(CameraComponent::id())
    }

    pub fn add_camera(&self, entity_id: u64, camera: CameraComponent) {
        let entity = self.inner.entity_from_id(entity_id);
        if entity.is_alive() {
            if !self.has_transform(entity_id) {
                self.add_transform(entity_id, Transform::default());
            }
            entity.set(camera);
        }
    }

    pub fn remove_camera(&self, entity_id: u64) {
        let entity = self.inner.entity_from_id(entity_id);
        if entity.is_alive() {
            entity.remove(CameraComponent::id());
        }
    }

    pub fn get_camera_mut<F>(&self, entity_id: u64, mut f: F)
    where
        F: FnMut(&mut CameraComponent),
    {
        let entity = self.inner.entity_from_id(entity_id);
        if entity.is_alive() {
            entity.try_get::<&mut CameraComponent>(|c| f(c));
        }
    }

    pub fn get_active_game_camera(&self) -> Option<(Vec3, Quat, CameraComponent)> {
        let mut best_camera = None;
        let mut highest_priority = i32::MIN;

        self.inner.each::<(&GlobalTransform, &CameraComponent)>(|(global, camera)| {
            if camera.is_active && camera.priority >= highest_priority {
                highest_priority = camera.priority;
                let (_, rot, pos) = global.0.to_scale_rotation_translation();
                best_camera = Some((pos, rot, *camera));
            }
        });

        best_camera
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

    pub fn each_model_entity<F>(&self, mut f: F)
    where
        F: FnMut(u64, &GlobalTransform, &ModelComponent),
    {
        self.inner
            .each_entity::<(&GlobalTransform, &ModelComponent)>(|entity, (global, model)| {
                f(*entity.id(), global, model);
            });
    }

    pub fn each_light_entity<F>(&self, mut f: F)
    where
        F: FnMut(u64, &GlobalTransform, &LightComponent),
    {
        self.inner
            .each_entity::<(&GlobalTransform, &LightComponent)>(|entity, (global, light)| {
                f(*entity.id(), global, light);
            });
    }

    pub fn each_camera_entity<F>(&self, mut f: F)
    where
        F: FnMut(u64, &GlobalTransform, &CameraComponent),
    {
        self.inner
            .each_entity::<(&GlobalTransform, &CameraComponent)>(|entity, (global, camera)| {
                f(*entity.id(), global, camera);
            });
    }

    pub fn each_rigid_body_entity<F>(&self, mut f: F)
    where
        F: FnMut(u64, &RigidBodyComponent, &Transform, Option<&ColliderComponent>),
    {
        self.inner
            .each_entity::<(&RigidBodyComponent, &Transform)>(|entity, (rb_comp, transform)| {
                let entity_id = *entity.id();
                let collider = entity.try_get::<&ColliderComponent>(|c| *c);
                f(entity_id, rb_comp, transform, collider.as_ref());
            });
    }

    // --- RigidBody Component Management ---

    pub fn has_rigid_body(&self, entity_id: u64) -> bool {
        let entity = self.inner.entity_from_id(entity_id);
        entity.is_alive() && entity.has(RigidBodyComponent::id())
    }

    pub fn add_rigid_body(&self, entity_id: u64, rb: RigidBodyComponent) {
        let entity = self.inner.entity_from_id(entity_id);
        if entity.is_alive() {
            if !self.has_transform(entity_id) {
                self.add_transform(entity_id, Transform::default());
            }
            entity.set(rb);
        }
    }

    pub fn remove_rigid_body(&self, entity_id: u64) {
        let entity = self.inner.entity_from_id(entity_id);
        if entity.is_alive() {
            entity.remove(RigidBodyComponent::id());
        }
    }

    pub fn get_rigid_body_mut<F>(&self, entity_id: u64, mut f: F)
    where
        F: FnMut(&mut RigidBodyComponent),
    {
        let entity = self.inner.entity_from_id(entity_id);
        if entity.is_alive() {
            entity.try_get::<&mut RigidBodyComponent>(|r| f(r));
        }
    }

    // --- Collider Component Management ---

    pub fn has_collider(&self, entity_id: u64) -> bool {
        let entity = self.inner.entity_from_id(entity_id);
        entity.is_alive() && entity.has(ColliderComponent::id())
    }

    pub fn add_collider(&self, entity_id: u64, collider: ColliderComponent) {
        let entity = self.inner.entity_from_id(entity_id);
        if entity.is_alive() {
            if !self.has_transform(entity_id) {
                self.add_transform(entity_id, Transform::default());
            }
            entity.set(collider);
        }
    }

    pub fn remove_collider(&self, entity_id: u64) {
        let entity = self.inner.entity_from_id(entity_id);
        if entity.is_alive() {
            entity.remove(ColliderComponent::id());
        }
    }

    pub fn get_collider_mut<F>(&self, entity_id: u64, mut f: F)
    where
        F: FnMut(&mut ColliderComponent),
    {
        let entity = self.inner.entity_from_id(entity_id);
        if entity.is_alive() {
            entity.try_get::<&mut ColliderComponent>(|c| f(c));
        }
    }

    pub fn get_active_game_cameras(&self) -> Vec<(glam::Vec3, glam::Quat, CameraComponent)> {
        let mut cameras = Vec::new();

        self.inner.each::<(&GlobalTransform, &CameraComponent)>(|(global, camera)| {
            if camera.is_active {
                let (_, rot, pos) = global.0.to_scale_rotation_translation();
                cameras.push((pos, rot, *camera));
            }
        });

        cameras.sort_by_key(|(_, _, cam)| cam.priority);
        cameras
    }

    pub fn each_model_for_mask<F>(&self, mask: u32, mut f: F)
    where
        F: FnMut(&GlobalTransform, &ModelComponent),
    {
        self.inner
            .each::<(&GlobalTransform, &ModelComponent)>(|(global, model)| {
                if (model.render_layer & mask) != 0 {
                    f(global, model);
                }
            });
    }

    // --- CharacterController Component Management ---

    pub fn has_character_controller(&self, entity_id: u64) -> bool {
        let entity = self.inner.entity_from_id(entity_id);
        entity.is_alive() && entity.has(CharacterControllerComponent::id())
    }

    pub fn add_character_controller(&self, entity_id: u64, kcc: CharacterControllerComponent) {
        let entity = self.inner.entity_from_id(entity_id);
        if entity.is_alive() {
            entity.set(kcc);
        }
    }

    pub fn remove_character_controller(&self, entity_id: u64) {
        let entity = self.inner.entity_from_id(entity_id);
        if entity.is_alive() {
            entity.remove(CharacterControllerComponent::id());
        }
    }

    pub fn get_character_controller<F, R>(&self, entity_id: u64, mut f: F) -> Option<R>
    where
        F: FnMut(&CharacterControllerComponent) -> R,
    {
        let entity = self.inner.entity_from_id(entity_id);
        if entity.is_alive() {
            entity.try_get::<&CharacterControllerComponent>(|k| f(k))
        } else {
            None
        }
    }

    pub fn get_character_controller_mut<F>(&self, entity_id: u64, mut f: F)
    where
        F: FnMut(&mut CharacterControllerComponent),
    {
        let entity = self.inner.entity_from_id(entity_id);
        if entity.is_alive() {
            entity.try_get::<&mut CharacterControllerComponent>(|k| f(k));
        }
    }

    // --- PrefabRef Management ---

    pub fn has_prefab_ref(&self, entity_id: u64) -> bool {
        let entity = self.inner.entity_from_id(entity_id);
        entity.is_alive() && entity.has(PrefabRef::id())
    }

    pub fn add_prefab_ref(&self, entity_id: u64, prefab_ref: PrefabRef) {
        let entity = self.inner.entity_from_id(entity_id);
        if entity.is_alive() {
            entity.set(prefab_ref);
        }
    }

    pub fn remove_prefab_ref(&self, entity_id: u64) {
        let entity = self.inner.entity_from_id(entity_id);
        if entity.is_alive() {
            entity.remove(PrefabRef::id());
        }
    }

    pub fn get_prefab_ref<F, R>(&self, entity_id: u64, mut f: F) -> Option<R>
    where
        F: FnMut(&PrefabRef) -> R,
    {
        let entity = self.inner.entity_from_id(entity_id);
        if entity.is_alive() {
            entity.try_get::<&PrefabRef>(|p| f(p))
        } else {
            None
        }
    }

    pub fn apply_physics_interpolated_transform(
        &self,
        entity_id: u64,
        pos: glam::Vec3,
        rot: glam::Quat,
    ) {
        let entity = self.inner.entity_from_id(entity_id);
        if !entity.is_alive() {
            return;
        }

        let scale = entity
            .try_get::<&Transform>(|t| t.scale)
            .unwrap_or(glam::Vec3::ONE);

        let parent_global = entity
            .parent()
            .and_then(|p| p.try_get::<&GlobalTransform>(|g| g.0));

        let global_mat = match parent_global {
            None => {
                entity.try_get::<&mut Transform>(|t| {
                    t.position = pos;
                    t.rotation = rot;
                });
                let g = glam::Mat4::from_scale_rotation_translation(scale, rot, pos);
                entity.set(GlobalTransform(g));
                g
            }
            Some(parent_mat) => {
                let world_mat = glam::Mat4::from_scale_rotation_translation(scale, rot, pos);
                let local_mat = parent_mat.inverse() * world_mat;
                let (local_scale, local_rot, local_pos) = local_mat.to_scale_rotation_translation();

                entity.try_get::<&mut Transform>(|t| {
                    t.position = local_pos;
                    t.rotation = local_rot;
                    t.scale = local_scale;
                });
                entity.set(GlobalTransform(world_mat));
                world_mat
            }
        };

        fn propagate_children(e: flecs_ecs::core::EntityView, p_global: glam::Mat4) {
            e.each_child(|child| {
                let child_mat = child
                    .try_get::<&Transform>(|t| t.compute_matrix())
                    .unwrap_or(glam::Mat4::IDENTITY);
                let child_global = p_global * child_mat;
                child.set(GlobalTransform(child_global));
                propagate_children(child, child_global);
            });
        }

        propagate_children(entity, global_mat);
    }

    pub fn update_entity_and_children_transforms(&self, entity_id: u64) {
        let entity = self.inner.entity_from_id(entity_id);
        if !entity.is_alive() {
            return;
        }

        let parent_global = entity
            .parent()
            .and_then(|p| p.try_get::<&GlobalTransform>(|g| g.0))
            .unwrap_or(glam::Mat4::IDENTITY);

        let my_global = if let Some(local_mat) = entity.try_get::<&Transform>(|t| t.compute_matrix()) {
            let g = parent_global * local_mat;
            entity.set(GlobalTransform(g));
            g
        } else {
            parent_global
        };

        fn propagate(e: flecs_ecs::core::EntityView, p_mat: glam::Mat4) {
            e.each_child(|child| {
                let mut cur_mat = p_mat;
                if let Some(l_mat) = child.try_get::<&Transform>(|t| t.compute_matrix()) {
                    cur_mat = p_mat * l_mat;
                    child.set(GlobalTransform(cur_mat));
                }
                propagate(child, cur_mat);
            });
        }

        propagate(entity, my_global);
    }

    pub fn reload_prefab_instances(&self, prefab_path: &str) {
        let Ok(prefab) = serialization::Prefab::load_from_file(prefab_path) else {
            log::error!("[PREFAB] Failed to reload prefab file: {}", prefab_path);
            return;
        };

        let target_path = std::path::Path::new(prefab_path);
        let target_filename = target_path.file_name();

        let mut instances = Vec::new();

        self.inner.each_entity::<&PrefabRef>(|entity, pref_ref| {
            let inst_path = std::path::Path::new(&pref_ref.path);
            
            let is_match = inst_path == target_path 
                || (target_filename.is_some() && inst_path.file_name() == target_filename);

            if is_match {
                let id = *entity.id();
                let name = self.get_entity_name(id);
                let transform = entity.try_get::<&Transform>(|t| *t).unwrap_or_default();
                let parent_id = entity.parent().map(|p| *p.id());

                instances.push((id, name, transform, parent_id));
            }
        });

        for (old_id, custom_name, transform, parent_id) in instances {
            self.delete_entity(old_id);

            if let Ok(new_id) = prefab.instantiate(
                self,
                Some(transform.position),
                Some(transform.rotation),
                parent_id,
            ) {
                self.get_transform_mut(new_id, |t| {
                    t.scale = transform.scale;
                });

                if !custom_name.is_empty() {
                    self.set_entity_name(new_id, &custom_name);
                }

                self.inner.entity_from_id(new_id).set(PrefabRef {
                    path: prefab_path.replace('\\', "/"),
                });
            }
        }

        self.update_transforms();
        log::info!("[PREFAB] Successfully synchronized all instances of {:?}", target_filename);
    }

    pub fn spawn_prefab(
        &self,
        prefab_path: &str,
        position: glam::Vec3,
        rotation: glam::Quat,
    ) -> anyhow::Result<u64> {
        let prefab = serialization::Prefab::load_from_file(prefab_path)?;
        let root_id = prefab.instantiate(self, Some(position), Some(rotation), None)?;

        self.inner.entity_from_id(root_id).set(PrefabRef {
            path: prefab_path.replace('\\', "/"),
        });

        Ok(root_id)
    }

    pub fn save_entity_as_prefab(
        &self,
        entity_id: u64,
        save_path: &str,
    ) -> anyhow::Result<()> {
        let prefab = serialization::Prefab::from_entity(self, entity_id)?;
        prefab.save_to_file(save_path)
    }
}

pub fn create_world() -> World {
    let world = flecs_ecs::core::World::new();

    world.component::<EntityName>();
    world.component::<Transform>();
    world.component::<GlobalTransform>();
    world.component::<CameraComponent>();
    world.component::<LightComponent>();
    world.component::<ModelComponent>();
    world.component::<RigidBodyComponent>();
    world.component::<ColliderComponent>();
    world.component::<CharacterControllerComponent>();
    world.component::<PrefabRef>();
    world.component::<Static>();
    world.component::<Dynamic>();
    world.component::<SceneEntity>();

    let w = World {
        inner: world,
        component_entries: Arc::new(RwLock::new(Vec::new())),
    };

    w.auto_register_components();
    w
}

pub fn spawn_scene(
    world: &World,
    asset_id: AssetId,
    baked_model: &BakedModel,
    root_position: Vec3,
) -> u64 {
    spawn_scene_with_colliders(world, asset_id, baked_model, root_position, false)
}

// Расширенная функция с 5 аргументами (для уровней/комнат с генерацией коллизий)
pub fn spawn_scene_with_colliders(
    world: &World,
    asset_id: AssetId,
    baked_model: &BakedModel,
    root_position: Vec3,
    generate_colliders: bool,
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
        .set(EntityName(clean_name.to_string()))
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
        generate_colliders: bool,
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
            .child_of(parent)
            .set(EntityName(node_name.clone()))
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

            let target_entity = if node.meshes.len() == 1 {
                node_entity
            } else {
                let child_mesh_name = format!("{}_{}", mesh_name, i);
                world
                    .inner
                    .entity()
                    .child_of(node_entity)
                    .set(EntityName(child_mesh_name))
                    .set(Transform::default())
                    .set(GlobalTransform(global_matrix))
                    .add(SceneEntity)
                    .add(Static)
            };

            target_entity.set(ModelComponent {
                id: asset_id,
                mesh_index: mesh_idx,
                render_layer: RenderLayers::DEFAULT,
            });

            if generate_colliders {
                target_entity.set(ColliderComponent {
                    shape_type: ColliderShapeType::TriMesh,
                    friction: 0.7,
                    restitution: 0.0,
                    ..Default::default()
                });
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
                .child_of(node_entity)
                .set(EntityName(light_name))
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
                generate_colliders,
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
            generate_colliders,
        );
    }

    *model_root.id()
}