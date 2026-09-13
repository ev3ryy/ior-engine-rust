use crate::components::*;
use crate::entity::Entity;
use crate::reflection::*;
use crate::serialization;
use asset::AssetId;
use glam::{Mat4, Quat, Vec3};
use shared::BakedModel;
use std::sync::{Arc, RwLock};

use flecs_ecs::prelude::*;

pub trait Component: ComponentId + 'static {}
impl<T: ComponentId + 'static> Component for T {}

pub struct World {
    pub(crate) inner: flecs_ecs::core::World,
    pub(crate) component_entries: Arc<RwLock<Vec<DynamicComponentEntry>>>,
    pub(crate) serialization_registry: Arc<RwLock<serialization::SerializationRegistry>>,
}

impl Default for World {
    fn default() -> Self {
        create_world()
    }
}

impl World {
    #[inline]
    pub fn set<T: Component + DataComponent>(&self, entity: impl Into<Entity>, component: T) {
        let e = self.inner.entity_from_id(entity.into().0);
        if e.is_alive() {
            e.set(component);
        }
    }

    #[inline]
    pub fn get<T: Component + DataComponent + Clone>(&self, entity: impl Into<Entity>) -> Option<T> {
        let e = self.inner.entity_from_id(entity.into().0);
        if e.is_alive() {
            e.try_get::<&T>(|c| c.clone())
        } else {
            None
        }
    }

    #[inline]
    pub fn with<T: Component + DataComponent, R>(
        &self,
        entity: impl Into<Entity>,
        f: impl FnOnce(&T) -> R,
    ) -> Option<R> {
        let e = self.inner.entity_from_id(entity.into().0);
        if e.is_alive() {
            e.try_get::<&T>(f)
        } else {
            None
        }
    }

    #[inline]
    pub fn modify<T: Component + DataComponent, R>(
        &self,
        entity: impl Into<Entity>,
        f: impl FnOnce(&mut T) -> R,
    ) -> Option<R> {
        let e = self.inner.entity_from_id(entity.into().0);
        if e.is_alive() {
            e.try_get::<&mut T>(f)
        } else {
            None
        }
    }

    #[inline]
    pub fn has<T: Component>(&self, entity: impl Into<Entity>) -> bool {
        let e = self.inner.entity_from_id(entity.into().0);
        e.is_alive() && e.has(T::id())
    }

    #[inline]
    pub fn remove<T: Component>(&self, entity: impl Into<Entity>) {
        let e = self.inner.entity_from_id(entity.into().0);
        if e.is_alive() {
            e.remove(T::id());
        }
    }

    #[inline]
    pub fn add_tag<T: Component>(&self, entity: impl Into<Entity>) {
        let e = self.inner.entity_from_id(entity.into().0);
        if e.is_alive() {
            e.add(T::id());
        }
    }

    // --- Dynamic Component Reflection & Registration ---

    pub fn register_component<T>(&self)
    where
        T: Inspectable + Default + Clone + flecs_ecs::prelude::ComponentId + flecs_ecs::prelude::DataComponent,
    {
        self.register_component_internal::<T>(T::NAME, T::CATEGORY);
    }

    pub fn register_component_internal<T>(&self, name: &'static str, category: &'static str)
    where
        T: Inspectable + Default + Clone + flecs_ecs::prelude::ComponentId + flecs_ecs::prelude::DataComponent,
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

    pub fn register_serializable<T>(&self, name: &'static str)
    where
        T: Component + DataComponent + serde::Serialize + serde::de::DeserializeOwned + Default + 'static,
    {
        self.serialization_registry
            .write()
            .unwrap()
            .register::<T>(name);
    }

    pub fn register_tag<T>(&self, name: &'static str)
    where
        T: Component + 'static,
    {
        self.serialization_registry
            .write()
            .unwrap()
            .register_tag::<T>(name);
    }

    pub fn get_component_entries(&self) -> Vec<DynamicComponentEntry> {
        self.component_entries.read().unwrap().clone()
    }

    pub fn get_serialization_registry(&self) -> serialization::SerializationRegistry {
        self.serialization_registry.read().unwrap().clone()
    }

    // --- Snapshots & Execution ---

    pub fn create_snapshot(&self) -> anyhow::Result<Vec<u8>> {
        serialization::create_world_snapshot(self)
    }

    pub fn restore_snapshot(&mut self, bytes: &[u8]) -> anyhow::Result<()> {
        serialization::restore_world_snapshot(self, bytes)
    }

    #[inline]
    pub fn progress(&self) {
        self.inner.progress();
    }

    // --- Scene Lifecycle ---

    pub fn clear_scene(&mut self) {
        let mut root_entities = Vec::new();

        self.inner.each_entity::<&SceneEntity>(|entity, _| {
            if entity.parent().is_none() {
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

    pub fn spawn_empty(&self, name: &str) -> Entity {
        let entity = self
            .inner
            .entity()
            .set(EntityName(name.to_string()))
            .set(SceneEntity::default());
        Entity(*entity.id())
    }

    pub fn spawn_child(&self, parent: impl Into<Entity>, name: &str) -> Entity {
        let parent_e = self.inner.entity_from_id(parent.into().0);
        let entity = self
            .inner
            .entity()
            .child_of(parent_e)
            .set(EntityName(name.to_string()))
            .set(SceneEntity::default());
        Entity(*entity.id())
    }

    pub fn delete_entity(&self, entity: impl Into<Entity>) {
        let e = self.inner.entity_from_id(entity.into().0);
        if e.is_alive() {
            e.destruct();
        }
    }

    #[inline]
    pub fn is_alive(&self, entity: impl Into<Entity>) -> bool {
        self.inner.entity_from_id(entity.into().0).is_alive()
    }

    // --- Entity Hierarchy & Naming ---

    pub fn get_entity_name(&self, entity: impl Into<Entity>) -> String {
        let e = self.inner.entity_from_id(entity.into().0);
        if !e.is_alive() {
            return String::new();
        }

        if let Some(name) = e.try_get::<&EntityName>(|n| n.0.clone()) {
            return name;
        }

        e.get_name().unwrap_or_default()
    }

    pub fn set_entity_name(&self, entity: impl Into<Entity>, name: &str) {
        let e = self.inner.entity_from_id(entity.into().0);
        if e.is_alive() {
            e.set(EntityName(name.to_string()));
        }
    }

    pub fn get_root_entities(&self) -> Vec<Entity> {
        let mut roots = Vec::new();
        self.inner.each_entity::<&SceneEntity>(|entity, _| {
            if entity.parent().is_none() {
                roots.push(Entity(*entity.id()));
            }
        });
        roots
    }

    pub fn get_children(&self, entity: impl Into<Entity>) -> Vec<Entity> {
        let mut children = Vec::new();
        let e = self.inner.entity_from_id(entity.into().0);
        if e.is_alive() {
            e.each_child(|child| {
                if child.has(SceneEntity::id()) {
                    children.push(Entity(*child.id()));
                }
            });
        }
        children
    }

    pub fn has_children(&self, entity: impl Into<Entity>) -> bool {
        let e = self.inner.entity_from_id(entity.into().0);
        if e.is_alive() {
            e.has_children()
        } else {
            false
        }
    }

    // --- Transform Management ---

    pub fn has_transform(&self, entity: impl Into<Entity>) -> bool {
        self.has::<Transform>(entity)
    }

    pub fn add_transform(&self, entity: impl Into<Entity>, transform: Transform) {
        let entity_id = entity.into();
        let e = self.inner.entity_from_id(entity_id.0);
        if e.is_alive() {
            let parent_global = e
                .parent()
                .and_then(|parent| parent.try_get::<&GlobalTransform>(|g| g.0))
                .unwrap_or(Mat4::IDENTITY);

            let global_mat = parent_global * transform.compute_matrix();

            e.set(transform)
                .set(GlobalTransform(global_mat))
                .add(Static);
        }
    }

    pub fn remove_transform(&self, entity: impl Into<Entity>) {
        let entity_id = entity.into();
        self.remove::<Transform>(entity_id);
        self.remove::<GlobalTransform>(entity_id);
    }

    pub fn get_transform_mut<F>(&self, entity: impl Into<Entity>, mut f: F)
    where
        F: FnMut(&mut Transform),
    {
        self.modify(entity, |t| f(t));
    }

    pub fn get_global_transform(&self, entity: impl Into<Entity>) -> Option<Mat4> {
        let e = self.inner.entity_from_id(entity.into().0);
        if e.is_alive() {
            e.try_get::<&GlobalTransform>(|g| g.0)
        } else {
            None
        }
    }

    pub fn set_transform_from_global_matrix(&self, entity: impl Into<Entity>, new_global: Mat4) {
        let e = self.inner.entity_from_id(entity.into().0);
        if !e.is_alive() {
            return;
        }

        let parent_global = e
            .parent()
            .and_then(|parent| parent.try_get::<&GlobalTransform>(|g| g.0))
            .unwrap_or(Mat4::IDENTITY);

        let local_matrix = parent_global.inverse() * new_global;
        let (scale, rotation, position) = local_matrix.to_scale_rotation_translation();

        e.set(Transform {
            position,
            rotation,
            scale,
        });
        e.set(GlobalTransform(new_global));
        e.add(Dynamic);
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

        self.inner.each_entity::<&SceneEntity>(|entity, _| {
            if entity.parent().is_none() {
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

    pub fn has_light(&self, entity: impl Into<Entity>) -> bool {
        self.has::<LightComponent>(entity)
    }

    pub fn add_light(&self, entity: impl Into<Entity>, light: LightComponent) {
        let id = entity.into();
        if !self.has_transform(id) {
            self.add_transform(id, Transform::default());
        }
        self.set(id, light);
    }

    pub fn remove_light(&self, entity: impl Into<Entity>) {
        self.remove::<LightComponent>(entity);
    }

    pub fn get_light_mut<F>(&self, entity: impl Into<Entity>, mut f: F)
    where
        F: FnMut(&mut LightComponent),
    {
        self.modify(entity, |l| f(l));
    }

    // --- Model Component Management ---

    pub fn has_model(&self, entity: impl Into<Entity>) -> bool {
        self.has::<ModelComponent>(entity)
    }

    pub fn get_model<F>(&self, entity: impl Into<Entity>, mut f: F)
    where
        F: FnMut(&ModelComponent),
    {
        let e = self.inner.entity_from_id(entity.into().0);
        if e.is_alive() {
            e.try_get::<&ModelComponent>(|m| f(m));
        }
    }

    pub fn get_model_mut<F>(&self, entity: impl Into<Entity>, mut f: F)
    where
        F: FnMut(&mut ModelComponent),
    {
        self.modify(entity, |m| f(m));
    }

    pub fn remove_model(&self, entity: impl Into<Entity>) {
        self.remove::<ModelComponent>(entity);
    }

    // --- Camera Component Management ---

    pub fn has_camera(&self, entity: impl Into<Entity>) -> bool {
        self.has::<CameraComponent>(entity)
    }

    pub fn add_camera(&self, entity: impl Into<Entity>, camera: CameraComponent) {
        let id = entity.into();
        if !self.has_transform(id) {
            self.add_transform(id, Transform::default());
        }
        self.set(id, camera);
    }

    pub fn remove_camera(&self, entity: impl Into<Entity>) {
        self.remove::<CameraComponent>(entity);
    }

    pub fn get_camera_mut<F>(&self, entity: impl Into<Entity>, mut f: F)
    where
        F: FnMut(&mut CameraComponent),
    {
        self.modify(entity, |c| f(c));
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
        F: FnMut(Entity, &GlobalTransform, &ModelComponent),
    {
        self.inner
            .each_entity::<(&GlobalTransform, &ModelComponent)>(|entity, (global, model)| {
                f(Entity(*entity.id()), global, model);
            });
    }

    pub fn each_light_entity<F>(&self, mut f: F)
    where
        F: FnMut(Entity, &GlobalTransform, &LightComponent),
    {
        self.inner
            .each_entity::<(&GlobalTransform, &LightComponent)>(|entity, (global, light)| {
                f(Entity(*entity.id()), global, light);
            });
    }

    pub fn each_camera_entity<F>(&self, mut f: F)
    where
        F: FnMut(Entity, &GlobalTransform, &CameraComponent),
    {
        self.inner
            .each_entity::<(&GlobalTransform, &CameraComponent)>(|entity, (global, camera)| {
                f(Entity(*entity.id()), global, camera);
            });
    }

    pub fn each_rigid_body_entity<F>(&self, mut f: F)
    where
        F: FnMut(Entity, &RigidBodyComponent, &Transform, Option<&ColliderComponent>),
    {
        self.inner
            .each_entity::<(&RigidBodyComponent, &Transform)>(|entity, (rb_comp, transform)| {
                let entity_id = Entity(*entity.id());
                let collider = entity.try_get::<&ColliderComponent>(|c| *c);
                f(entity_id, rb_comp, transform, collider.as_ref());
            });
    }

    pub fn each_rigid_body<F>(&self, mut f: F)
    where
        F: FnMut(Entity, &RigidBodyComponent, Option<&GlobalTransform>, Option<&Transform>),
    {
        self.inner.each_entity::<&RigidBodyComponent>(|entity, rb_comp| {
            let global = entity.try_get::<&GlobalTransform>(|g| *g);
            let transform = entity.try_get::<&Transform>(|t| *t);
            f(Entity(*entity.id()), rb_comp, global.as_ref(), transform.as_ref());
        });
    }

    pub fn each_collider<F>(&self, mut f: F)
    where
        F: FnMut(
            Entity,
            &ColliderComponent,
            Option<&GlobalTransform>,
            Option<&Transform>,
            Option<&ModelComponent>,
        ),
    {
        self.inner.each_entity::<&ColliderComponent>(|entity, col_comp| {
            let global = entity.try_get::<&GlobalTransform>(|g| *g);
            let transform = entity.try_get::<&Transform>(|t| *t);
            let model = entity.try_get::<&ModelComponent>(|m| m.clone());
            f(
                Entity(*entity.id()),
                col_comp,
                global.as_ref(),
                transform.as_ref(),
                model.as_ref(),
            );
        });
    }

    // --- Physics Components Management ---

    pub fn has_rigid_body(&self, entity: impl Into<Entity>) -> bool {
        self.has::<RigidBodyComponent>(entity)
    }

    pub fn add_rigid_body(&self, entity: impl Into<Entity>, rb: RigidBodyComponent) {
        let id = entity.into();
        if !self.has_transform(id) {
            self.add_transform(id, Transform::default());
        }
        self.set(id, rb);
    }

    pub fn remove_rigid_body(&self, entity: impl Into<Entity>) {
        self.remove::<RigidBodyComponent>(entity);
    }

    pub fn get_rigid_body_mut<F>(&self, entity: impl Into<Entity>, mut f: F)
    where
        F: FnMut(&mut RigidBodyComponent),
    {
        self.modify(entity, |r| f(r));
    }

    pub fn has_collider(&self, entity: impl Into<Entity>) -> bool {
        self.has::<ColliderComponent>(entity)
    }

    pub fn add_collider(&self, entity: impl Into<Entity>, collider: ColliderComponent) {
        let id = entity.into();
        if !self.has_transform(id) {
            self.add_transform(id, Transform::default());
        }
        self.set(id, collider);
    }

    pub fn remove_collider(&self, entity: impl Into<Entity>) {
        self.remove::<ColliderComponent>(entity);
    }

    pub fn get_collider_mut<F>(&self, entity: impl Into<Entity>, mut f: F)
    where
        F: FnMut(&mut ColliderComponent),
    {
        self.modify(entity, |c| f(c));
    }

    pub fn get_active_game_cameras(&self) -> Vec<(Vec3, Quat, CameraComponent)> {
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

    pub fn has_character_controller(&self, entity: impl Into<Entity>) -> bool {
        self.has::<CharacterControllerComponent>(entity)
    }

    pub fn add_character_controller(&self, entity: impl Into<Entity>, kcc: CharacterControllerComponent) {
        self.set(entity, kcc);
    }

    pub fn remove_character_controller(&self, entity: impl Into<Entity>) {
        self.remove::<CharacterControllerComponent>(entity);
    }

    pub fn get_character_controller<F, R>(&self, entity: impl Into<Entity>, mut f: F) -> Option<R>
    where
        F: FnMut(&CharacterControllerComponent) -> R,
    {
        let e = self.inner.entity_from_id(entity.into().0);
        if e.is_alive() {
            e.try_get::<&CharacterControllerComponent>(|k| f(k))
        } else {
            None
        }
    }

    pub fn get_character_controller_mut<F>(&self, entity: impl Into<Entity>, mut f: F)
    where
        F: FnMut(&mut CharacterControllerComponent),
    {
        self.modify(entity, |k| f(k));
    }

    // --- PrefabRef Management ---

    pub fn has_prefab_ref(&self, entity: impl Into<Entity>) -> bool {
        self.has::<PrefabRef>(entity)
    }

    pub fn add_prefab_ref(&self, entity: impl Into<Entity>, prefab_ref: PrefabRef) {
        self.set(entity, prefab_ref);
    }

    pub fn remove_prefab_ref(&self, entity: impl Into<Entity>) {
        self.remove::<PrefabRef>(entity);
    }

    pub fn get_prefab_ref<F, R>(&self, entity: impl Into<Entity>, mut f: F) -> Option<R>
    where
        F: FnMut(&PrefabRef) -> R,
    {
        let e = self.inner.entity_from_id(entity.into().0);
        if e.is_alive() {
            e.try_get::<&PrefabRef>(|p| f(p))
        } else {
            None
        }
    }

    pub fn apply_physics_interpolated_transform(
        &self,
        entity: impl Into<Entity>,
        pos: Vec3,
        rot: Quat,
    ) {
        let e = self.inner.entity_from_id(entity.into().0);
        if !e.is_alive() {
            return;
        }

        let scale = e.try_get::<&Transform>(|t| t.scale).unwrap_or(Vec3::ONE);
        let parent_global = e.parent().and_then(|p| p.try_get::<&GlobalTransform>(|g| g.0));

        let global_mat = match parent_global {
            None => {
                e.try_get::<&mut Transform>(|t| {
                    t.position = pos;
                    t.rotation = rot;
                });
                let g = Mat4::from_scale_rotation_translation(scale, rot, pos);
                e.set(GlobalTransform(g));
                g
            }
            Some(parent_mat) => {
                let world_mat = Mat4::from_scale_rotation_translation(scale, rot, pos);
                let local_mat = parent_mat.inverse() * world_mat;
                let (local_scale, local_rot, local_pos) = local_mat.to_scale_rotation_translation();

                e.try_get::<&mut Transform>(|t| {
                    t.position = local_pos;
                    t.rotation = local_rot;
                    t.scale = local_scale;
                });
                e.set(GlobalTransform(world_mat));
                world_mat
            }
        };

        fn propagate_children(e: flecs_ecs::core::EntityView, p_global: Mat4) {
            e.each_child(|child| {
                let child_mat = child
                    .try_get::<&Transform>(|t| t.compute_matrix())
                    .unwrap_or(Mat4::IDENTITY);
                let child_global = p_global * child_mat;
                child.set(GlobalTransform(child_global));
                propagate_children(child, child_global);
            });
        }

        propagate_children(e, global_mat);
    }

    pub fn update_entity_and_children_transforms(&self, entity: impl Into<Entity>) {
        let e = self.inner.entity_from_id(entity.into().0);
        if !e.is_alive() {
            return;
        }

        let parent_global = e
            .parent()
            .and_then(|p| p.try_get::<&GlobalTransform>(|g| g.0))
            .unwrap_or(Mat4::IDENTITY);

        let my_global = if let Some(local_mat) = e.try_get::<&Transform>(|t| t.compute_matrix()) {
            let g = parent_global * local_mat;
            e.set(GlobalTransform(g));
            g
        } else {
            parent_global
        };

        fn propagate(e: flecs_ecs::core::EntityView, p_mat: Mat4) {
            e.each_child(|child| {
                let mut cur_mat = p_mat;
                if let Some(l_mat) = child.try_get::<&Transform>(|t| t.compute_matrix()) {
                    cur_mat = p_mat * l_mat;
                    child.set(GlobalTransform(cur_mat));
                }
                propagate(child, cur_mat);
            });
        }

        propagate(e, my_global);
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

                self.set(new_id, PrefabRef {
                    path: prefab_path.replace('\\', "/"),
                });
            }
        }

        self.update_transforms();
        log::info!("[PREFAB] Successfully synchronized all instances of {:?}", target_filename);
    }

    // --- Query Helpers ---
    pub fn query1<T: Component + DataComponent, F>(&self, mut f: F)
    where
        F: FnMut(Entity, &T),
    {
        self.inner.each_entity::<&T>(|entity, c| {
            f(Entity(*entity.id()), c);
        });
    }

    pub fn query1_mut<T: Component + DataComponent, F>(&self, mut f: F)
    where
        F: FnMut(Entity, &mut T),
    {
        self.inner.each_entity::<&mut T>(|entity, c| {
            f(Entity(*entity.id()), c);
        });
    }

    pub fn query2<T1: Component + DataComponent, T2: Component + DataComponent, F>(&self, mut f: F)
    where
        F: FnMut(Entity, &T1, &T2),
    {
        self.inner.each_entity::<(&T1, &T2)>(|entity, (c1, c2)| {
            f(Entity(*entity.id()), c1, c2);
        });
    }

    pub fn query2_mut1<T1: Component + DataComponent, T2: Component + DataComponent, F>(&self, mut f: F)
    where
        F: FnMut(Entity, &mut T1, &T2),
    {
        self.inner.each_entity::<(&mut T1, &T2)>(|entity, (c1, c2)| {
            f(Entity(*entity.id()), c1, c2);
        });
    }

    pub fn spawn_prefab(
        &self,
        prefab_path: &str,
        position: Vec3,
        rotation: Quat,
    ) -> anyhow::Result<Entity> {
        let prefab = serialization::Prefab::load_from_file(prefab_path)?;
        let root_entity = prefab.instantiate(self, Some(position), Some(rotation), None::<Entity>)?;

        self.inner.entity_from_id(root_entity.0).set(PrefabRef {
            path: prefab_path.replace('\\', "/"),
        });

        Ok(root_entity)
    }

    pub fn save_entity_as_prefab(
        &self,
        entity: impl Into<Entity>,
        save_path: &str,
    ) -> anyhow::Result<()> {
        let entity_id = entity.into();
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
        serialization_registry: Arc::new(RwLock::new(serialization::SerializationRegistry::new())),
    };

    w.register_serializable::<Transform>("Transform");
    w.register_serializable::<CameraComponent>("CameraComponent");
    w.register_serializable::<LightComponent>("LightComponent");
    w.register_serializable::<ModelComponent>("ModelComponent");
    w.register_serializable::<RigidBodyComponent>("RigidBodyComponent");
    w.register_serializable::<ColliderComponent>("ColliderComponent");
    w.register_serializable::<CharacterControllerComponent>("CharacterControllerComponent");
    w.register_serializable::<PrefabRef>("PrefabRef");
    w.register_tag::<Static>("Static");
    w.register_tag::<Dynamic>("Dynamic");

    w.auto_register_components();
    w
}

pub fn spawn_scene(
    world: &World,
    asset_id: AssetId,
    baked_model: &BakedModel,
    root_position: Vec3,
) -> Entity {
    spawn_scene_with_colliders(world, asset_id, baked_model, root_position, false)
}

pub fn spawn_scene_with_colliders(
    world: &World,
    asset_id: AssetId,
    baked_model: &BakedModel,
    root_position: Vec3,
    generate_colliders: bool,
) -> Entity {
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
        .set(SceneEntity::default())
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
            .set(SceneEntity::default())
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
                    .set(SceneEntity::default())
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
                .set(SceneEntity::default())
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

    Entity(*model_root.id())
}