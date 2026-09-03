use ecs::flecs_ecs::prelude::*;
use ecs::{BodyType, ColliderComponent, ColliderShapeType, RigidBodyComponent, Transform, World};
use rapier3d::prelude::*;
use std::collections::{HashMap, HashSet};

#[inline]
pub fn transform_to_pose(pos: glam::Vec3, rot: glam::Quat) -> Pose {
    Pose::from_parts(pos, rot)
}

#[derive(Debug, Clone, Copy)]
pub struct RayHit {
    pub entity: u64,
    pub point: glam::Vec3,
    pub normal: glam::Vec3,
    pub distance: f32,
}

#[derive(Debug, Clone, Copy)]
pub struct RaycastFilter {
    pub exclude_entity: Option<u64>,
    pub ignore_sensors: bool,
    pub solid: bool,
}

impl Default for RaycastFilter {
    fn default() -> Self {
        Self {
            exclude_entity: None,
            ignore_sensors: true,
            solid: true,
        }
    }
}

pub struct PhysicsWorld {
    pub gravity: Vector,
    pub integration_parameters: IntegrationParameters,
    pub physics_pipeline: PhysicsPipeline,
    pub island_manager: IslandManager,
    pub broad_phase: DefaultBroadPhase,
    pub narrow_phase: NarrowPhase,
    pub rigid_body_set: RigidBodySet,
    pub collider_set: ColliderSet,
    pub impulse_joint_set: ImpulseJointSet,
    pub multibody_joint_set: MultibodyJointSet,
    pub ccd_solver: CCDSolver,
    pub physics_hooks: (),
    pub event_handler: (),

    pub entity_to_body: HashMap<u64, RigidBodyHandle>,
    pub body_to_entity: HashMap<RigidBodyHandle, u64>,
}

impl Default for PhysicsWorld {
    fn default() -> Self {
        Self::new()
    }
}

impl PhysicsWorld {
    pub fn new() -> Self {
        Self {
            gravity: Vector::new(0.0, -9.81, 0.0),
            integration_parameters: IntegrationParameters::default(),
            physics_pipeline: PhysicsPipeline::new(),
            island_manager: IslandManager::new(),
            broad_phase: DefaultBroadPhase::new(),
            narrow_phase: NarrowPhase::new(),
            rigid_body_set: RigidBodySet::new(),
            collider_set: ColliderSet::new(),
            impulse_joint_set: ImpulseJointSet::new(),
            multibody_joint_set: MultibodyJointSet::new(),
            ccd_solver: CCDSolver::new(),
            physics_hooks: (),
            event_handler: (),
            entity_to_body: HashMap::new(),
            body_to_entity: HashMap::new(),
        }
    }

    pub fn clear(&mut self) {
        self.rigid_body_set = RigidBodySet::new();
        self.collider_set = ColliderSet::new();
        self.island_manager = IslandManager::new();
        self.broad_phase = DefaultBroadPhase::new();
        self.narrow_phase = NarrowPhase::new();
        self.impulse_joint_set = ImpulseJointSet::new();
        self.multibody_joint_set = MultibodyJointSet::new();
        self.ccd_solver = CCDSolver::new();
        self.entity_to_body.clear();
        self.body_to_entity.clear();
    }

    // --- Spatial Queries (Raycasting) ---
    pub fn cast_ray(
        &self,
        origin: glam::Vec3,
        dir: glam::Vec3,
        max_distance: f32,
        filter: RaycastFilter,
    ) -> Option<RayHit> {
        if dir.length_squared() < 1e-6 {
            return None;
        }
        let norm_dir = dir.normalize();
        let ray = Ray::new(origin, norm_dir);

        let mut query_filter = QueryFilter::default();
        if filter.ignore_sensors {
            query_filter = query_filter.exclude_sensors();
        }

        if let Some(exclude_id) = filter.exclude_entity {
            if let Some(&rb_handle) = self.entity_to_body.get(&exclude_id) {
                query_filter = query_filter.exclude_rigid_body(rb_handle);
            }
        }

        let query_pipeline = self.broad_phase.as_query_pipeline(
            self.narrow_phase.query_dispatcher(),
            &self.rigid_body_set,
            &self.collider_set,
            query_filter,
        );

        if let Some((collider_handle, hit)) =
            query_pipeline.cast_ray_and_get_normal(&ray, max_distance, filter.solid)
        {
            let hit_entity = self
                .collider_set
                .get(collider_handle)
                .and_then(|c| {
                    if c.user_data != 0 {
                        Some(c.user_data as u64)
                    } else if let Some(parent) = c.parent() {
                        self.body_to_entity.get(&parent).copied()
                    } else {
                        None
                    }
                })
                .unwrap_or(0);

            let hit_point = origin + norm_dir * hit.time_of_impact;

            Some(RayHit {
                entity: hit_entity,
                point: hit_point,
                normal: hit.normal,
                distance: hit.time_of_impact,
            })
        } else {
            None
        }
    }

    pub fn cast_ray_simple(&self, origin: glam::Vec3, dir: glam::Vec3, max_dist: f32) -> Option<RayHit> {
        self.cast_ray(origin, dir, max_dist, RaycastFilter::default())
    }

    pub fn is_on_ground(&self, entity_id: u64, current_pos: glam::Vec3, check_distance: f32) -> bool {
        let filter = RaycastFilter {
            exclude_entity: Some(entity_id),
            ignore_sensors: true,
            solid: false,
        };
        if let Some(hit) = self.cast_ray(current_pos, glam::Vec3::NEG_Y, check_distance, filter) {
            hit.normal.y > 0.5
        } else {
            false
        }
    }

    // --- Gameplay Manipulation Helpers ---
    pub fn apply_impulse(&mut self, entity_id: u64, impulse: glam::Vec3) {
        if let Some(&handle) = self.entity_to_body.get(&entity_id) {
            if let Some(body) = self.rigid_body_set.get_mut(handle) {
                body.apply_impulse(impulse, true);
            }
        }
    }

    pub fn apply_force(&mut self, entity_id: u64, force: glam::Vec3) {
        if let Some(&handle) = self.entity_to_body.get(&entity_id) {
            if let Some(body) = self.rigid_body_set.get_mut(handle) {
                body.add_force(force, true);
            }
        }
    }

    pub fn set_linear_velocity(&mut self, entity_id: u64, velocity: glam::Vec3) {
        if let Some(&handle) = self.entity_to_body.get(&entity_id) {
            if let Some(body) = self.rigid_body_set.get_mut(handle) {
                body.set_linvel(velocity, true);
            }
        }
    }

    pub fn get_linear_velocity(&self, entity_id: u64) -> glam::Vec3 {
        self.entity_to_body
            .get(&entity_id)
            .and_then(|&h| self.rigid_body_set.get(h))
            .map(|b| b.linvel()) 
            .unwrap_or(glam::Vec3::ZERO)
    }

    pub fn teleport_entity(&mut self, entity_id: u64, pos: glam::Vec3, rot: glam::Quat) {
        if let Some(&handle) = self.entity_to_body.get(&entity_id) {
            if let Some(body) = self.rigid_body_set.get_mut(handle) {
                body.set_position(transform_to_pose(pos, rot), true);
                body.set_linvel(glam::Vec3::ZERO, true);
                body.set_angvel(glam::Vec3::ZERO, true);
            }
        }
    }

    // --- Pipeline Sync ---

    pub fn sync_from_ecs(&mut self, world: &World) {
        let mut active_entities = HashSet::new();

        world.inner.each_entity::<(&RigidBodyComponent, &Transform)>(|entity, (rb_comp, transform)| {
            let entity_id = *entity.id();
            active_entities.insert(entity_id);

            if let Some(&body_handle) = self.entity_to_body.get(&entity_id) {
                if let Some(body) = self.rigid_body_set.get_mut(body_handle) {
                    if body.is_kinematic() || body.is_fixed() {
                        let pose = transform_to_pose(transform.position, transform.rotation);
                        body.set_position(pose, true);
                    }
                }
            } else {
                let rb_type = match rb_comp.body_type {
                    BodyType::Dynamic => RigidBodyType::Dynamic,
                    BodyType::Fixed => RigidBodyType::Fixed,
                    BodyType::KinematicPositionBased => RigidBodyType::KinematicPositionBased,
                    BodyType::KinematicVelocityBased => RigidBodyType::KinematicVelocityBased,
                };

                let mut builder = RigidBodyBuilder::new(rb_type)
                    .pose(transform_to_pose(transform.position, transform.rotation))
                    .additional_mass(rb_comp.mass.max(0.001))
                    .linear_damping(rb_comp.linear_damping)
                    .angular_damping(rb_comp.angular_damping)
                    .gravity_scale(rb_comp.gravity_scale)
                    .ccd_enabled(rb_comp.ccd_enabled)
                    .user_data(entity_id as u128);

                if rb_comp.lock_rotations {
                    builder = builder.lock_rotations();
                }

                let body = builder.build();
                let body_handle = self.rigid_body_set.insert(body);

                entity.try_get::<&ColliderComponent>(|col_comp| {
                    let shape = match col_comp.shape_type {
                        ColliderShapeType::Cuboid => {
                            SharedShape::cuboid(col_comp.size[0], col_comp.size[1], col_comp.size[2])
                        }
                        ColliderShapeType::Sphere => SharedShape::ball(col_comp.size[0]),
                        ColliderShapeType::Capsule => {
                            SharedShape::capsule_y(col_comp.size[1], col_comp.size[0])
                        }
                        ColliderShapeType::Cylinder => {
                            SharedShape::cylinder(col_comp.size[1], col_comp.size[0])
                        }
                    };

                    let collider = ColliderBuilder::new(shape)
                        .friction(col_comp.friction)
                        .restitution(col_comp.restitution)
                        .sensor(col_comp.is_sensor)
                        .user_data(entity_id as u128)
                        .build();

                    self.collider_set.insert_with_parent(
                        collider,
                        body_handle,
                        &mut self.rigid_body_set,
                    );
                });

                self.entity_to_body.insert(entity_id, body_handle);
                self.body_to_entity.insert(body_handle, entity_id);
            }
        });

        let to_remove: Vec<u64> = self
            .entity_to_body
            .keys()
            .copied()
            .filter(|id| !active_entities.contains(id))
            .collect();

        for entity_id in to_remove {
            if let Some(body_handle) = self.entity_to_body.remove(&entity_id) {
                self.body_to_entity.remove(&body_handle);
                self.rigid_body_set.remove(
                    body_handle,
                    &mut self.island_manager,
                    &mut self.collider_set,
                    &mut self.impulse_joint_set,
                    &mut self.multibody_joint_set,
                    true,
                );
            }
        }
    }

    pub fn step(&mut self, dt: f32) {
        self.integration_parameters.dt = dt;

        self.physics_pipeline.step(
            self.gravity,
            &self.integration_parameters,
            &mut self.island_manager,
            &mut self.broad_phase,
            &mut self.narrow_phase,
            &mut self.rigid_body_set,
            &mut self.collider_set,
            &mut self.impulse_joint_set,
            &mut self.multibody_joint_set,
            &mut self.ccd_solver,
            &self.physics_hooks,
            &self.event_handler,
        );
    }

    pub fn sync_to_ecs(&self, world: &World) {
        for (&body_handle, &entity_id) in &self.body_to_entity {
            if let Some(body) = self.rigid_body_set.get(body_handle) {
                if body.is_dynamic() {
                    let pose = body.position();
                    let pos = pose.translation;
                    let rot = pose.rotation;

                    world.get_transform_mut(entity_id, |t| {
                        t.position = pos;
                        t.rotation = rot;
                    });
                }
            }
        }
    }
}