use ecs::flecs_ecs::prelude::*;
use ecs::{
    BodyType, CharacterControllerComponent, ColliderComponent, ColliderShapeType,
    GlobalTransform, ModelComponent, RigidBodyComponent, Transform, World,
};
use rapier3d::control::{
    CharacterAutostep, CharacterLength, KinematicCharacterController,
};
use rapier3d::geometry::{InteractionGroups, InteractionTestMode};
use rapier3d::pipeline::ChannelEventCollector;
use rapier3d::prelude::*;
use std::collections::{HashMap, HashSet};
use std::sync::mpsc::Receiver;
use std::sync::Arc;

#[inline]
pub fn transform_to_pose(pos: glam::Vec3, rot: glam::Quat) -> Pose {
    Pose::from_parts(pos, rot)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CollisionEventType {
    Started,
    Stopped,
}

#[derive(Debug, Clone, Copy)]
pub struct PhysicsCollisionEvent {
    pub entity1: u64,
    pub entity2: u64,
    pub event_type: CollisionEventType,
    pub is_sensor: bool,
}

impl PhysicsCollisionEvent {
    #[inline]
    pub fn involves(&self, entity_id: u64) -> bool {
        self.entity1 == entity_id || self.entity2 == entity_id
    }

    #[inline]
    pub fn other_entity(&self, my_entity_id: u64) -> Option<u64> {
        if self.entity1 == my_entity_id {
            Some(self.entity2)
        } else if self.entity2 == my_entity_id {
            Some(self.entity1)
        } else {
            None
        }
    }
}

#[derive(Debug, Clone)]
pub struct MeshGeometry {
    pub vertices: Vec<glam::Vec3>,
    pub indices: Vec<u32>,
}

#[derive(Debug, Clone, Copy)]
pub struct CharacterMovement {
    pub translation: glam::Vec3,
    pub is_grounded: bool,
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
    pub groups: Option<InteractionGroups>,
}

impl Default for RaycastFilter {
    fn default() -> Self {
        Self {
            exclude_entity: None,
            ignore_sensors: true,
            solid: true,
            groups: None,
        }
    }
}

pub struct CollisionLayer;

impl CollisionLayer {
    pub const NONE: u32        = 0;
    pub const WORLD: u32       = 1 << 0;
    pub const CHARACTER: u32      = 1 << 1;
    pub const INTERACTABLE: u32= 1 << 2;
    pub const TRIGGER: u32     = 1 << 3; 
    pub const DOOR: u32        = 1 << 4;

    // pub fn weapon_raycast_groups() -> InteractionGroups {
    //     InteractionGroups::new(
    //         Group::ALL, 
    //         Group::from_bits_truncate(Self::WORLD | Self::CHARACTER | Self::DOOR)
    //     )
    // }

    // pub fn interact_raycast_groups() -> InteractionGroups {
    //     InteractionGroups::new(
    //         Group::ALL, 
    //         Group::from_bits_truncate(Self::INTERACTABLE | Self::DOOR | Self::WORLD)
    //     )
    // }
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

    pub event_handler: ChannelEventCollector,
    collision_recv: Receiver<CollisionEvent>,
    contact_force_recv: Receiver<ContactForceEvent>,
    pub collision_events: Vec<PhysicsCollisionEvent>,

    pub entity_to_body: HashMap<u64, RigidBodyHandle>,
    pub body_to_entity: HashMap<RigidBodyHandle, u64>,
    pub entity_to_collider: HashMap<u64, ColliderHandle>,
    pub collider_to_entity: HashMap<ColliderHandle, u64>,

    pub mesh_cache: HashMap<(asset::AssetId, usize), Arc<MeshGeometry>>,

    collider_shape_cache: HashMap<u64, (ColliderShapeType, [f32; 3], glam::Vec3)>,

    pub previous_poses: HashMap<RigidBodyHandle, Pose>,

    pub debug_lines_buffer: Vec<[f32; 7]>,

    active_bodies_scratch: HashSet<u64>,
    active_colliders_scratch: HashSet<u64>,
    to_remove_scratch: Vec<u64>,
}

impl Default for PhysicsWorld {
    fn default() -> Self {
        Self::new()
    }
}

#[inline]
fn push_debug_line(buffer: &mut Vec<[f32; 7]>, a: glam::Vec3, b: glam::Vec3, color: [f32; 4]) {
    buffer.push([a.x, a.y, a.z, color[0], color[1], color[2], color[3]]);
    buffer.push([b.x, b.y, b.z, color[0], color[1], color[2], color[3]]);
}

fn push_debug_circle(
    buffer: &mut Vec<[f32; 7]>,
    center: glam::Vec3,
    u: glam::Vec3,
    v: glam::Vec3,
    radius: f32,
    color: [f32; 4],
) {
    let segments = 16;
    for i in 0..segments {
        let a0 = (i as f32 / segments as f32) * std::f32::consts::TAU;
        let a1 = ((i + 1) as f32 / segments as f32) * std::f32::consts::TAU;
        let p0 = center + (u * a0.cos() + v * a0.sin()) * radius;
        let p1 = center + (u * a1.cos() + v * a1.sin()) * radius;
        push_debug_line(buffer, p0, p1, color);
    }
}

impl PhysicsWorld {
    pub fn new() -> Self {
        let (collision_send, collision_recv) = std::sync::mpsc::channel();
        let (contact_force_send, contact_force_recv) = std::sync::mpsc::channel();
        let event_handler = ChannelEventCollector::new(collision_send, contact_force_send);

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

            event_handler,
            collision_recv,
            contact_force_recv,
            collision_events: Vec::with_capacity(64),

            entity_to_body: HashMap::new(),
            body_to_entity: HashMap::new(),
            entity_to_collider: HashMap::new(),
            collider_to_entity: HashMap::new(),
            mesh_cache: HashMap::new(),
            collider_shape_cache: HashMap::new(),

            previous_poses: HashMap::new(),
            debug_lines_buffer: Vec::with_capacity(4096),

            active_bodies_scratch: HashSet::with_capacity(256),
            active_colliders_scratch: HashSet::with_capacity(512),
            to_remove_scratch: Vec::with_capacity(64),
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
        self.entity_to_collider.clear();
        self.collider_to_entity.clear();
        self.collider_shape_cache.clear();
        self.previous_poses.clear();
        self.debug_lines_buffer.clear();

        self.collision_events.clear();
        while self.collision_recv.try_recv().is_ok() {}
        while self.contact_force_recv.try_recv().is_ok() {}

        self.active_bodies_scratch.clear();
        self.active_colliders_scratch.clear();
        self.to_remove_scratch.clear();
    }

    // --- Mesh Geometry Registration ---

    pub fn register_mesh_geometry(
        &mut self,
        asset_id: asset::AssetId,
        mesh_index: usize,
        vertices: Vec<glam::Vec3>,
        indices: Vec<u32>,
    ) {
        self.mesh_cache.insert(
            (asset_id, mesh_index),
            Arc::new(MeshGeometry { vertices, indices }),
        );
    }

    pub fn register_baked_model(
        &mut self,
        asset_id: asset::AssetId,
        baked_model: &shared::BakedModel,
    ) {
        for (mesh_idx, mesh) in baked_model.meshes.iter().enumerate() {
            let vertices: Vec<glam::Vec3> = mesh
                .vertices
                .iter()
                .map(|v| glam::Vec3::from(v.position))
                .collect();

            self.register_mesh_geometry(
                asset_id,
                mesh_idx,
                vertices,
                mesh.indices.clone(),
            );
        }
    }

    // --- Kinematic Character Controller (KCC) ---

    pub fn move_character_controller(
        &mut self,
        entity_id: u64,
        kcc_comp: &CharacterControllerComponent,
        desired_translation: glam::Vec3,
        dt: f32,
    ) -> CharacterMovement {
        let Some(&body_handle) = self.entity_to_body.get(&entity_id) else {
            return CharacterMovement { translation: desired_translation, is_grounded: false };
        };

        let Some(&col_handle) = self.entity_to_collider.get(&entity_id) else {
            return CharacterMovement { translation: desired_translation, is_grounded: false };
        };

        let (Some(_body), Some(collider)) = (
            self.rigid_body_set.get(body_handle),
            self.collider_set.get(col_handle),
        ) else {
            return CharacterMovement { translation: desired_translation, is_grounded: false };
        };

        let autostep = if kcc_comp.enable_autostep {
            Some(CharacterAutostep {
                max_height: CharacterLength::Absolute(kcc_comp.autostep_max_height),
                min_width: CharacterLength::Absolute(kcc_comp.autostep_min_width),
                include_dynamic_bodies: true,
            })
        } else {
            None
        };

        let snap_to_ground = if kcc_comp.enable_snap_to_ground {
            Some(CharacterLength::Absolute(kcc_comp.snap_to_ground_distance))
        } else {
            None
        };

        let kcc = KinematicCharacterController {
            offset: CharacterLength::Relative(0.01),
            autostep,
            snap_to_ground,
            slide: kcc_comp.slide,
            max_slope_climb_angle: kcc_comp.max_slope_climb_angle.to_radians(),
            min_slope_slide_angle: kcc_comp.min_slope_slide_angle.to_radians(),
            ..Default::default()
        };

        let query_filter = QueryFilter::default()
            .exclude_rigid_body(body_handle)
            .exclude_collider(col_handle);

        let query_pipeline = self.broad_phase.as_query_pipeline(
            self.narrow_phase.query_dispatcher(),
            &self.rigid_body_set,
            &self.collider_set,
            query_filter,
        );

        let effective_movement = kcc.move_shape(
            dt,
            &query_pipeline,
            collider.shape(),
            collider.position(),
            desired_translation,
            |_| {},
        );

        CharacterMovement {
            translation: effective_movement.translation,
            is_grounded: effective_movement.grounded,
        }
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

        if let Some(groups) = filter.groups {
            query_filter = query_filter.groups(groups);
        }

        if let Some(exclude_id) = filter.exclude_entity {
            if let Some(&rb_handle) = self.entity_to_body.get(&exclude_id) {
                query_filter = query_filter.exclude_rigid_body(rb_handle);
            }
            if let Some(&col_handle) = self.entity_to_collider.get(&exclude_id) {
                query_filter = query_filter.exclude_collider(col_handle);
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
                .collider_to_entity
                .get(&collider_handle)
                .copied()
                .or_else(|| {
                    self.collider_set.get(collider_handle).and_then(|c| {
                        if c.user_data != 0 {
                            Some(c.user_data as u64)
                        } else if let Some(parent) = c.parent() {
                            self.body_to_entity.get(&parent).copied()
                        } else {
                            None
                        }
                    })
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
            groups: None,
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
        let pose = transform_to_pose(pos, rot);

        if let Some(&handle) = self.entity_to_body.get(&entity_id) {
            if let Some(body) = self.rigid_body_set.get_mut(handle) {
                body.set_position(pose, true);
                body.set_linvel(glam::Vec3::ZERO, true);
                body.set_angvel(glam::Vec3::ZERO, true);
            }
        } else if let Some(&col_handle) = self.entity_to_collider.get(&entity_id) {
            if let Some(collider) = self.collider_set.get_mut(col_handle) {
                collider.set_position(pose);
            }
        }
    }

    #[inline]
    pub fn collision_events(&self) -> &[PhysicsCollisionEvent] {
        &self.collision_events
    }

    // --- Transform Interpolation ---

    pub fn update_render_transforms(&self, world: &World, alpha: f32) {
        let alpha = alpha.clamp(0.0, 1.0);

        for (&body_handle, &entity_id) in &self.body_to_entity {
            if let Some(body) = self.rigid_body_set.get(body_handle) {
                if body.is_dynamic() || body.is_kinematic() {
                    let curr_pose = body.position();
                    let prev_pose = self.previous_poses.get(&body_handle).unwrap_or(curr_pose);

                    let interp_pos = prev_pose.translation.lerp(curr_pose.translation, alpha);
                    let interp_rot = prev_pose.rotation.slerp(curr_pose.rotation, alpha);

                    world.apply_physics_interpolated_transform(entity_id, interp_pos, interp_rot);
                }
            }
        }
    }

    // --- Debug Line Generation for GPU ---

    pub fn generate_debug_lines(&mut self) -> &[[f32; 7]] {
        let mut buffer = std::mem::take(&mut self.debug_lines_buffer);
        buffer.clear();

        for (_handle, collider) in self.collider_set.iter() {
            let pose = collider.position();
            let pos = pose.translation;
            let rot = pose.rotation;

            let color = if collider.is_sensor() {
                [1.0, 0.8, 0.0, 1.0]
            } else if collider.parent().is_none() {
                [0.2, 0.6, 1.0, 1.0]
            } else {
                [0.0, 1.0, 0.4, 1.0]
            };

            let shape = collider.shape();
            if let Some(cuboid) = shape.as_cuboid() {
                let hx = cuboid.half_extents.x;
                let hy = cuboid.half_extents.y;
                let hz = cuboid.half_extents.z;

                let corners = [
                    pos + rot * glam::Vec3::new(-hx, -hy, -hz),
                    pos + rot * glam::Vec3::new(hx, -hy, -hz),
                    pos + rot * glam::Vec3::new(hx, hy, -hz),
                    pos + rot * glam::Vec3::new(-hx, hy, -hz),
                    pos + rot * glam::Vec3::new(-hx, -hy, hz),
                    pos + rot * glam::Vec3::new(hx, -hy, hz),
                    pos + rot * glam::Vec3::new(hx, hy, hz),
                    pos + rot * glam::Vec3::new(-hx, hy, hz),
                ];

                let edges = [
                    (0, 1), (1, 2), (2, 3), (3, 0),
                    (4, 5), (5, 6), (6, 7), (7, 4),
                    (0, 4), (1, 5), (2, 6), (3, 7),
                ];

                for (i, j) in edges {
                    push_debug_line(&mut buffer, corners[i], corners[j], color);
                }
            } else if let Some(ball) = shape.as_ball() {
                let r = ball.radius;
                push_debug_circle(&mut buffer, pos, rot * glam::Vec3::X, rot * glam::Vec3::Y, r, color);
                push_debug_circle(&mut buffer, pos, rot * glam::Vec3::X, rot * glam::Vec3::Z, r, color);
                push_debug_circle(&mut buffer, pos, rot * glam::Vec3::Y, rot * glam::Vec3::Z, r, color);
            } else if let Some(capsule) = shape.as_capsule() {
                let r = capsule.radius;
                let p1 = glam::Vec3::new(capsule.segment.a.x, capsule.segment.a.y, capsule.segment.a.z);
                let p2 = glam::Vec3::new(capsule.segment.b.x, capsule.segment.b.y, capsule.segment.b.z);
                let wp1 = pos + rot * p1;
                let wp2 = pos + rot * p2;

                push_debug_circle(&mut buffer, wp1, rot * glam::Vec3::X, rot * glam::Vec3::Z, r, color);
                push_debug_circle(&mut buffer, wp2, rot * glam::Vec3::X, rot * glam::Vec3::Z, r, color);
                push_debug_line(&mut buffer, wp1 + rot * (glam::Vec3::X * r), wp2 + rot * (glam::Vec3::X * r), color);
                push_debug_line(&mut buffer, wp1 - rot * (glam::Vec3::X * r), wp2 - rot * (glam::Vec3::X * r), color);
                push_debug_line(&mut buffer, wp1 + rot * (glam::Vec3::Z * r), wp2 + rot * (glam::Vec3::Z * r), color);
                push_debug_line(&mut buffer, wp1 - rot * (glam::Vec3::Z * r), wp2 - rot * (glam::Vec3::Z * r), color);
            } else if let Some(cylinder) = shape.as_cylinder() {
                let r = cylinder.radius;
                let hh = cylinder.half_height;
                let top = pos + rot * glam::Vec3::new(0.0, hh, 0.0);
                let bot = pos - rot * glam::Vec3::new(0.0, hh, 0.0);
                push_debug_circle(&mut buffer, top, rot * glam::Vec3::X, rot * glam::Vec3::Z, r, color);
                push_debug_circle(&mut buffer, bot, rot * glam::Vec3::X, rot * glam::Vec3::Z, r, color);
                push_debug_line(&mut buffer, top + rot * (glam::Vec3::X * r), bot + rot * (glam::Vec3::X * r), color);
                push_debug_line(&mut buffer, top - rot * (glam::Vec3::X * r), bot - rot * (glam::Vec3::X * r), color);
                push_debug_line(&mut buffer, top + rot * (glam::Vec3::Z * r), bot + rot * (glam::Vec3::Z * r), color);
                push_debug_line(&mut buffer, top - rot * (glam::Vec3::Z * r), bot - rot * (glam::Vec3::Z * r), color);
            } else if let Some(trimesh) = shape.as_trimesh() {
                let vertices = trimesh.vertices();
                let indices = trimesh.indices();
                for tri in indices {
                    let p0 = pos + rot * glam::Vec3::new(vertices[tri[0] as usize].x, vertices[tri[0] as usize].y, vertices[tri[0] as usize].z);
                    let p1 = pos + rot * glam::Vec3::new(vertices[tri[1] as usize].x, vertices[tri[1] as usize].y, vertices[tri[1] as usize].z);
                    let p2 = pos + rot * glam::Vec3::new(vertices[tri[2] as usize].x, vertices[tri[2] as usize].y, vertices[tri[2] as usize].z);
                    push_debug_line(&mut buffer, p0, p1, color);
                    push_debug_line(&mut buffer, p1, p2, color);
                    push_debug_line(&mut buffer, p2, p0, color);
                }
            }
        }

        self.debug_lines_buffer = buffer;
        &self.debug_lines_buffer
    }

    // --- Pipeline Sync ---

    fn build_shape_from_cache(
        mesh_cache: &HashMap<(asset::AssetId, usize), Arc<MeshGeometry>>,
        col_comp: &ColliderComponent,
        scale: glam::Vec3,
        model_comp: Option<&ModelComponent>,
    ) -> SharedShape {
        let s = scale.abs();
        match col_comp.shape_type {
            ColliderShapeType::Cuboid => SharedShape::cuboid(
                (col_comp.size[0] * s.x).max(0.001),
                (col_comp.size[1] * s.y).max(0.001),
                (col_comp.size[2] * s.z).max(0.001),
            ),
            ColliderShapeType::Sphere => {
                let max_s = s.x.max(s.y).max(s.z);
                SharedShape::ball((col_comp.size[0] * max_s).max(0.001))
            }
            ColliderShapeType::Capsule => {
                let r_scale = s.x.max(s.z);
                SharedShape::capsule_y(
                    (col_comp.size[1] * s.y).max(0.001),
                    (col_comp.size[0] * r_scale).max(0.001),
                )
            }
            ColliderShapeType::Cylinder => {
                let r_scale = s.x.max(s.z);
                SharedShape::cylinder(
                    (col_comp.size[1] * s.y).max(0.001),
                    (col_comp.size[0] * r_scale).max(0.001),
                )
            }
            ColliderShapeType::TriMesh => {
                if let Some(geom) = model_comp.and_then(|m| mesh_cache.get(&(m.id, m.mesh_index))) {
                    let scaled_vertices: Vec<glam::Vec3> =
                        geom.vertices.iter().map(|&v| v * scale).collect();
                    let tri_indices: Vec<[u32; 3]> = geom
                        .indices
                        .chunks_exact(3)
                        .map(|c| [c[0], c[1], c[2]])
                        .collect();

                    if !tri_indices.is_empty() && !scaled_vertices.is_empty() {
                        match SharedShape::trimesh(scaled_vertices, tri_indices) {
                            Ok(shape) => shape,
                            Err(e) => {
                                log::warn!("Failed to create TriMesh: {e:?}, fallback to cuboid");
                                SharedShape::cuboid(0.5, 0.5, 0.5)
                            }
                        }
                    } else {
                        SharedShape::cuboid(0.5, 0.5, 0.5)
                    }
                } else {
                    SharedShape::cuboid(0.5, 0.5, 0.5)
                }
            }
            ColliderShapeType::ConvexHull => {
                if let Some(geom) = model_comp.and_then(|m| mesh_cache.get(&(m.id, m.mesh_index))) {
                    let scaled_vertices: Vec<glam::Vec3> =
                        geom.vertices.iter().map(|&v| v * scale).collect();

                    SharedShape::convex_hull(&scaled_vertices).unwrap_or_else(|| {
                        log::warn!("Failed to create ConvexHull, fallback to ball");
                        SharedShape::ball(0.5 * scale.max_element())
                    })
                } else {
                    SharedShape::ball(0.5)
                }
            }
        }
    }

    pub fn build_shape(
        &self,
        col_comp: &ColliderComponent,
        scale: glam::Vec3,
        model_comp: Option<&ModelComponent>,
    ) -> SharedShape {
        Self::build_shape_from_cache(&self.mesh_cache, col_comp, scale, model_comp)
    }

    pub fn sync_from_ecs(&mut self, world: &World) {
        self.active_bodies_scratch.clear();
        self.active_colliders_scratch.clear();

        world.inner.each_entity::<&RigidBodyComponent>(|entity, rb_comp| {
            let entity_id = *entity.id();
            self.active_bodies_scratch.insert(entity_id);

            let (world_pos, world_rot, _) = if let Some(global) = entity.try_get::<&GlobalTransform>(|g| g.0) {
                let (_, r, p) = global.to_scale_rotation_translation();
                (p, r, glam::Vec3::ONE)
            } else if let Some(t) = entity.try_get::<&Transform>(|t| *t) {
                (t.position, t.rotation, t.scale)
            } else {
                (glam::Vec3::ZERO, glam::Quat::IDENTITY, glam::Vec3::ONE)
            };

            let body_pose = transform_to_pose(world_pos, world_rot);

            if let Some(&body_handle) = self.entity_to_body.get(&entity_id) {
                if let Some(body) = self.rigid_body_set.get_mut(body_handle) {
                    if body.is_kinematic() {
                        body.set_next_kinematic_position(body_pose);
                    } else if body.is_fixed() {
                        let current_pose = body.position();
                        let pos_diff = (current_pose.translation - body_pose.translation).length_squared() > 1e-6;
                        let rot_diff = current_pose.rotation.angle_between(body_pose.rotation).abs() > 1e-4;
                        if pos_diff || rot_diff {
                            body.set_position(body_pose, true);
                        }
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
                    .pose(body_pose)
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

                self.entity_to_body.insert(entity_id, body_handle);
                self.body_to_entity.insert(body_handle, entity_id);
            }
        });

        world.inner.each_entity::<&ColliderComponent>(|entity, col_comp| {
            let entity_id = *entity.id();
            self.active_colliders_scratch.insert(entity_id);

            let (world_pos, world_rot, scale) = if let Some(global) = entity.try_get::<&GlobalTransform>(|g| g.0) {
                let (s, r, p) = global.to_scale_rotation_translation();
                (p, r, s)
            } else if let Some(t) = entity.try_get::<&Transform>(|t| *t) {
                (t.position, t.rotation, t.scale)
            } else {
                (glam::Vec3::ZERO, glam::Quat::IDENTITY, glam::Vec3::ONE)
            };

            let offset_pos = glam::Vec3::from_array(col_comp.offset_position);
            let offset_rot = glam::Quat::from_array(col_comp.offset_rotation);
            let local_offset_pose = transform_to_pose(offset_pos, offset_rot);

            let target_body_handle = self.entity_to_body.get(&entity_id).copied();

            let groups = InteractionGroups::new(
                Group::from_bits_truncate(col_comp.collision_memberships),
                Group::from_bits_truncate(col_comp.collision_filters),
                InteractionTestMode::And,
            );

            let shape_signature = (col_comp.shape_type, col_comp.size, scale);

            if let Some(&col_handle) = self.entity_to_collider.get(&entity_id) {
                let parent_changed = self
                    .collider_set
                    .get(col_handle)
                    .map(|c| c.parent() != target_body_handle)
                    .unwrap_or(true);

                if parent_changed {
                    self.collider_set.remove(
                        col_handle,
                        &mut self.island_manager,
                        &mut self.rigid_body_set,
                        true,
                    );
                    self.entity_to_collider.remove(&entity_id);
                    self.collider_to_entity.remove(&col_handle);
                    self.collider_shape_cache.remove(&entity_id);
                } else if let Some(collider) = self.collider_set.get_mut(col_handle) {
                    if target_body_handle.is_some() {
                        collider.set_position_wrt_parent(local_offset_pose);
                    } else {
                        let combined_pos = world_pos + world_rot * offset_pos;
                        let combined_rot = (world_rot * offset_rot).normalize();
                        let combined_pose = transform_to_pose(combined_pos, combined_rot);

                        let current = collider.position();
                        let pos_diff = (current.translation - combined_pose.translation).length_squared() > 1e-6;
                        let rot_diff = current.rotation.angle_between(combined_pose.rotation).abs() > 1e-4;
                        if pos_diff || rot_diff {
                            collider.set_position(combined_pose);
                        }
                    }

                    if self.collider_shape_cache.get(&entity_id) != Some(&shape_signature) {
                        let model_comp = entity.try_get::<&ModelComponent>(|m| m.clone());
                        let new_shape = Self::build_shape_from_cache(
                            &self.mesh_cache,
                            col_comp,
                            scale,
                            model_comp.as_ref(),
                        );
                        collider.set_shape(new_shape);
                        self.collider_shape_cache.insert(entity_id, shape_signature);
                    }

                    if collider.is_sensor() != col_comp.is_sensor {
                        collider.set_sensor(col_comp.is_sensor);
                    }
                    if collider.collision_groups() != groups {
                        collider.set_collision_groups(groups);
                    }
                    if (collider.friction() - col_comp.friction).abs() > 1e-4 {
                        collider.set_friction(col_comp.friction);
                    }
                    if (collider.restitution() - col_comp.restitution).abs() > 1e-4 {
                        collider.set_restitution(col_comp.restitution);
                    }

                    return;
                }
            }

            let model_comp = entity.try_get::<&ModelComponent>(|m| m.clone());
            let shape = Self::build_shape_from_cache(
                &self.mesh_cache,
                col_comp,
                scale,
                model_comp.as_ref(),
            );

            let mut col_builder = ColliderBuilder::new(shape)
                .friction(col_comp.friction)
                .restitution(col_comp.restitution)
                .sensor(col_comp.is_sensor)
                .collision_groups(groups)
                .user_data(entity_id as u128);

            if col_comp.generate_collision_events {
                col_builder = col_builder.active_events(ActiveEvents::COLLISION_EVENTS);
            }

            let col_handle = if let Some(body_handle) = target_body_handle {
                col_builder = col_builder.position(local_offset_pose);
                self.collider_set.insert_with_parent(
                    col_builder.build(),
                    body_handle,
                    &mut self.rigid_body_set,
                )
            } else {
                let combined_pos = world_pos + world_rot * offset_pos;
                let combined_rot = (world_rot * offset_rot).normalize();
                col_builder = col_builder.position(transform_to_pose(combined_pos, combined_rot));
                self.collider_set.insert(col_builder.build())
            };

            self.entity_to_collider.insert(entity_id, col_handle);
            self.collider_to_entity.insert(col_handle, entity_id);
            self.collider_shape_cache.insert(entity_id, shape_signature);
        });

        self.to_remove_scratch.clear();
        for &id in self.entity_to_body.keys() {
            if !self.active_bodies_scratch.contains(&id) {
                self.to_remove_scratch.push(id);
            }
        }

        for entity_id in &self.to_remove_scratch {
            if let Some(body_handle) = self.entity_to_body.remove(entity_id) {
                self.body_to_entity.remove(&body_handle);
                self.previous_poses.remove(&body_handle);

                if let Some(col_handle) = self.entity_to_collider.remove(entity_id) {
                    self.collider_to_entity.remove(&col_handle);
                    self.collider_shape_cache.remove(entity_id);
                }

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

        self.to_remove_scratch.clear();
        for &id in self.entity_to_collider.keys() {
            if !self.active_colliders_scratch.contains(&id) {
                self.to_remove_scratch.push(id);
            }
        }

        for entity_id in &self.to_remove_scratch {
            if let Some(col_handle) = self.entity_to_collider.remove(entity_id) {
                self.collider_to_entity.remove(&col_handle);
                self.collider_shape_cache.remove(entity_id);
                self.collider_set.remove(
                    col_handle,
                    &mut self.island_manager,
                    &mut self.rigid_body_set,
                    true,
                );
            }
        }
    }

    pub fn step(&mut self, dt: f32) {
        for (&handle, _) in &self.body_to_entity {
            if let Some(body) = self.rigid_body_set.get(handle) {
                self.previous_poses.insert(handle, *body.position());
            }
        }

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

        self.collision_events.clear();
        while let Ok(event) = self.collision_recv.try_recv() {
            match event {
                CollisionEvent::Started(h1, h2, flags) => {
                    let e1 = self.collider_to_entity.get(&h1).copied().unwrap_or(0);
                    let e2 = self.collider_to_entity.get(&h2).copied().unwrap_or(0);
                    if e1 != 0 && e2 != 0 {
                        self.collision_events.push(PhysicsCollisionEvent {
                            entity1: e1,
                            entity2: e2,
                            event_type: CollisionEventType::Started,
                            is_sensor: flags.contains(CollisionEventFlags::SENSOR),
                        });
                    }
                }
                CollisionEvent::Stopped(h1, h2, flags) => {
                    let e1 = self.collider_to_entity.get(&h1).copied().unwrap_or(0);
                    let e2 = self.collider_to_entity.get(&h2).copied().unwrap_or(0);
                    if e1 != 0 && e2 != 0 {
                        self.collision_events.push(PhysicsCollisionEvent {
                            entity1: e1,
                            entity2: e2,
                            event_type: CollisionEventType::Stopped,
                            is_sensor: flags.contains(CollisionEventFlags::SENSOR),
                        });
                    }
                }
            }
        }
    }

    pub fn sync_to_ecs(&self, world: &World) {
        for (&body_handle, &entity_id) in &self.body_to_entity {
            if let Some(body) = self.rigid_body_set.get(body_handle) {
                if body.is_dynamic() {
                    let pose = body.position();
                    world.apply_physics_interpolated_transform(entity_id, pose.translation, pose.rotation);
                }
            }
        }
    }
}