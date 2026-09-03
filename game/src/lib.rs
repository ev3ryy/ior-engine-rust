use ecs::prelude::*;
use engine::game::{GameInput, GamePlugin};
use engine::physics::{PhysicsWorld, RaycastFilter};
use glam::{Quat, Vec3};

#[derive(Component, Inspectable, Default, Clone)]
#[component(category = "FPS")]
pub struct FpsPlayer {
    #[inspect(range = 1.0..=20.0)]
    pub speed: f32,
    #[inspect(range = 0.0005..=0.01, speed = 0.0002)]
    pub mouse_sensitivity: f32,
    #[inspect(range = 1.0..=15.0)]
    pub jump_force: f32,

    pub pitch: f32,
    pub yaw: f32,
}

#[derive(Component, Inspectable, Default, Clone)]
#[component(category = "FPS")]
pub struct Gun {
    #[inspect(range = 10.0..=500.0)]
    pub damage: f32,
    #[inspect(range = 5.0..=100.0)]
    pub impact_force: f32,
    pub fire_rate: f32,
    pub timer: f32,
}

#[derive(Default)]
pub struct MyGamePlugin;

impl GamePlugin for MyGamePlugin  {
    fn init(&mut self, world: &World) {
        log::info!("[GAME] Initializing FPS Components...");
        world.register_component::<FpsPlayer>();
        world.register_component::<Gun>();
    }

    fn on_start(&mut self, _world: &World, _physics: &mut PhysicsWorld) {
        log::info!("[GAME] FPS Play Mode Started!");
    }

    fn on_stop(&mut self, _world: &World) {
        log::info!("[GAME] FPS Play Mode Stopped!");
    }

    fn on_fixed_update(
        &mut self,
        world: &World,
        physics: &mut PhysicsWorld,
        input: &GameInput,
        _fixed_dt: f32,
    ) {
        world.each_entity::<(&Transform, &FpsPlayer)>(|entity, (transform, player)| {
            let id = entity.raw_id();

            let forward = transform.rotation * Vec3::NEG_Z;
            let right = transform.rotation * Vec3::X;
            let mut move_dir = forward * input.move_axis.y + right * input.move_axis.x;
            move_dir.y = 0.0;
            if move_dir.length_squared() > 1e-4 {
                move_dir = move_dir.normalize();
            }

            let mut vel = physics.get_linear_velocity(id);
            vel.x = move_dir.x * player.speed;
            vel.z = move_dir.z * player.speed;

            let is_grounded = physics.is_on_ground(id, transform.position, 1.1);
            if is_grounded && input.is_jump() {
                vel.y = player.jump_force;
            }

            physics.set_linear_velocity(id, vel);
        });
    }

    fn on_update(
        &mut self,
        world: &World,
        physics: &mut PhysicsWorld,
        input: &GameInput,
        dt: f32,
    ) {
        world.each::<(&mut Transform, &mut FpsPlayer)>(|(transform, player)| {
            player.yaw -= input.look_delta.x * player.mouse_sensitivity;
            player.pitch -= input.look_delta.y * player.mouse_sensitivity;
            player.pitch = player.pitch.clamp(-1.5, 1.5);

            transform.rotation = Quat::from_rotation_y(player.yaw);
        });

        world.each_entity::<(&Transform, &FpsPlayer, &mut Gun)>(|entity, (transform, player, gun)| {
            gun.timer -= dt;
            let player_id = entity.raw_id();

            if input.is_shoot_just_pressed() && gun.timer <= 0.0 {
                gun.timer = gun.fire_rate;

                let shoot_rot = Quat::from_rotation_y(player.yaw) * Quat::from_rotation_x(player.pitch);
                let ray_origin = transform.position + Vec3::new(0.0, 1.6, 0.0);
                let ray_dir = shoot_rot * Vec3::NEG_Z;

                let filter = RaycastFilter {
                    exclude_entity: Some(player_id),
                    ..Default::default()
                };

                if let Some(hit) = physics.cast_ray(ray_origin, ray_dir, 150.0, filter) {
                    log::info!("HIT Entity ID: {} at {:?}, dist: {:.2}m", hit.entity, hit.point, hit.distance);

                    physics.apply_impulse(hit.entity, ray_dir * gun.impact_force);
                }
            }
        });
    }

    fn on_late_update(
        &mut self,
        world: &World,
        _physics: &mut PhysicsWorld,
        _input: &GameInput,
        _dt: f32,
    ) {
        let mut player_state = None;
        world.each::<(&Transform, &FpsPlayer)>(|(t, p)| {
            player_state = Some((t.position, p.yaw, p.pitch));
        });

        if let Some((player_pos, player_yaw, player_pitch)) = player_state {
            world.each::<(&mut Transform, &CameraComponent)>(|(cam_t, _)| {
                cam_t.position = player_pos + Vec3::new(0.0, 1.6, 0.0);
                cam_t.rotation = Quat::from_rotation_y(player_yaw) * Quat::from_rotation_x(player_pitch);
            });
        }
    }
}