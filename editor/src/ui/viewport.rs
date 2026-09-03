use crate::ui::EditorContext;
use crate::EditorGizmoMode;
use ecs::{ColliderComponent, World};
use egui::{Color32, Painter, Pos2, Rect, Stroke};
use engine::camera::{Camera, Ray};
use glam::{camera, Mat4, Vec2, Vec3};
use std::sync::Arc;
use transform_gizmo_egui::math::{DMat4, DQuat, DVec3, Transform as GizmoTransform};
use transform_gizmo_egui::prelude::*;

pub fn draw_viewport(ui: &mut egui::Ui, ctx: &mut EditorContext) {
    let available_size = ui.available_size();
    if available_size.x <= 1.0 || available_size.y <= 1.0 {
        return;
    }

    let ppp = ui.ctx().pixels_per_point();
    let phys_w = (available_size.x * ppp).round().max(1.0) as u32;
    let phys_h = (available_size.y * ppp).round().max(1.0) as u32;
    *ctx.desired_viewport_size = Some((phys_w, phys_h));

    ctx.camera.aspect = available_size.x / available_size.y;

    // display offscreen viewport texture image
    let (rect, response) = if let Some(tex_id) = ctx.viewport_texture_id {
        let image = egui::Image::new(egui::load::SizedTexture::new(tex_id, available_size))
            .sense(egui::Sense::click_and_drag());
        let response = ui.add(image);
        (response.rect, response)
    } else {
        ui.allocate_exact_size(available_size, egui::Sense::click_and_drag())
    };

    let viewport_size = Vec2::new(rect.width(), rect.height());
    let painter = ui.painter().with_clip_rect(rect);
    let is_editing = ctx.play_state != engine::game::PlayState::Playing;
    let mut gizmo_was_dragged = false;

    // drag and drop spawning from asset browser
    if is_editing {
        if let Some(ref dragged_path) = ctx.asset_browser_state.dragged_asset.clone() {
            ui.ctx().set_cursor_icon(egui::CursorIcon::Grabbing);

            if let Some(mouse_pos) = ui.ctx().pointer_hover_pos() {
                if rect.contains(mouse_pos) {
                    let click_pos = Vec2::new(mouse_pos.x - rect.min.x, mouse_pos.y - rect.min.y);
                    let ray = ctx.camera.screen_to_ray(click_pos, viewport_size);

                    // raycast ground plane Y = 0 or scene objects
                    let mut spawn_pos = Vec3::ZERO;
                    if ray.direction.y.abs() > 1e-4 {
                        let t = -ray.origin.y / ray.direction.y;
                        if t > 0.0 {
                            spawn_pos = ray.origin + ray.direction * t;
                        }
                    } else {
                        spawn_pos = ctx.camera.eye + ray.direction * 5.0;
                    }

                    // draw spawn preview ring
                    draw_spawn_marker(&painter, ctx.camera, spawn_pos, rect);

                    // spawn on release
                    if ui.input(|i| i.pointer.any_released()) {
                        if let Ok(data) = std::fs::read(dragged_path) {
                            if let Ok(baked_model) =
                                rkyv::from_bytes::<shared::BakedModel, rkyv::rancor::Error>(&data)
                            {
                                if let Some(file_name) =
                                    dragged_path.file_name().and_then(|n| n.to_str())
                                {
                                    let model_id = asset::AssetId::from_path(file_name);
                                    let spawned_id = ecs::spawn_scene(
                                        ctx.world,
                                        model_id,
                                        &baked_model,
                                        spawn_pos,
                                    );
                                    *ctx.selected_entity = Some(spawned_id);
                                    ctx.load_requests.push(file_name.to_string());
                                    log::info!("Spawned {:?} at {}", file_name, spawn_pos);
                                }
                            }
                        }
                        ctx.asset_browser_state.dragged_asset = None;
                    }
                }
            }

            if ui.input(|i| i.pointer.any_released()) {
                ctx.asset_browser_state.dragged_asset = None;
            }
        }
    }

    // collider wireframe visualization
    if is_editing {
        if let Some(entity_id) = *ctx.selected_entity {
            if let Some(model_mat) = ctx.world.get_global_transform(entity_id) {
                if ctx.world.has_collider(entity_id) {
                    ctx.world.get_collider_mut(entity_id, |col| {
                        draw_collider_wireframe(&painter, ctx.camera, model_mat, col, rect);
                    });
                }
            }
        }
    }

    // 3d transform gizmo
    if is_editing {
        if let Some(entity_id) = *ctx.selected_entity {
            if let Some(model_mat) = ctx.world.get_global_transform(entity_id) {
                let eye = DVec3::new(
                    ctx.camera.eye.x as f64,
                    ctx.camera.eye.y as f64,
                    ctx.camera.eye.z as f64,
                );
                let target = DVec3::new(
                    ctx.camera.target.x as f64,
                    ctx.camera.target.y as f64,
                    ctx.camera.target.z as f64,
                );
                let up = DVec3::new(
                    ctx.camera.up.x as f64,
                    ctx.camera.up.y as f64,
                    ctx.camera.up.z as f64,
                );

                let view_mat = DMat4::look_at_rh(eye, target, up);
                let proj_mat = DMat4::perspective_infinite_rh(
                    (ctx.camera.fovy as f64).to_radians(),
                    (rect.width() / rect.height().max(1.0)) as f64,
                    ctx.camera.znear as f64,
                );

                let modes: EnumSet<GizmoMode> = match *ctx.gizmo_mode {
                    EditorGizmoMode::Translate => GizmoMode::all()
                        .into_iter()
                        .filter(|m| m.is_translate())
                        .collect(),
                    EditorGizmoMode::Rotate => GizmoMode::all()
                        .into_iter()
                        .filter(|m| m.is_rotate())
                        .collect(),
                    EditorGizmoMode::Scale => GizmoMode::all()
                        .into_iter()
                        .filter(|m| m.is_scale())
                        .collect(),
                    EditorGizmoMode::All => GizmoMode::all(),
                };

                ctx.gizmo.update_config(GizmoConfig {
                    view_matrix: view_mat.into(),
                    projection_matrix: proj_mat.into(),
                    modes,
                    orientation: *ctx.gizmo_orientation,
                    ..Default::default()
                });

                let (scale, rot, pos) = model_mat.to_scale_rotation_translation();
                let transform = GizmoTransform::from_scale_rotation_translation(
                    DVec3::new(scale.x as f64, scale.y as f64, scale.z as f64),
                    DQuat::from_xyzw(
                        rot.x as f64,
                        rot.y as f64,
                        rot.z as f64,
                        rot.w as f64,
                    ),
                    DVec3::new(pos.x as f64, pos.y as f64, pos.z as f64),
                );

                if let Some((_result, new_transforms)) = ctx.gizmo.interact(ui, &[transform]) {
                    gizmo_was_dragged = true;
                    if let Some(new_t) = new_transforms.first() {
                        let s = new_t.scale;
                        let r = new_t.rotation;
                        let p = new_t.translation;

                        let new_global = Mat4::from_scale_rotation_translation(
                            Vec3::new(s.x as f32, s.y as f32, s.z as f32),
                            glam::Quat::from_xyzw(
                                r.v.x as f32,
                                r.v.y as f32,
                                r.v.z as f32,
                                r.s as f32,
                            ),
                            Vec3::new(p.x as f32, p.y as f32, p.z as f32),
                        );

                        ctx.world.set_transform_from_global_matrix(entity_id, new_global);
                        ctx.world.update_transforms();
                    }
                }
            }
        }
    }

    // viewport click picking
    if is_editing && response.clicked() && !gizmo_was_dragged {
        if let Some(mouse_pos) = response.interact_pointer_pos() {
            let click_pos = Vec2::new(mouse_pos.x - rect.min.x, mouse_pos.y - rect.min.y);
            let ray = ctx.camera.screen_to_ray(click_pos, viewport_size);

            if let Some(hit_entity) = pick_entity(&ray, ctx.world, ctx.asset_server) {
                *ctx.selected_entity = Some(hit_entity);
            } else {
                *ctx.selected_entity = None;
            }
        }
    }
}

pub fn pick_entity(
    ray: &Ray,
    world: &World,
    asset_server: &Arc<asset::AssetServer>,
) -> Option<u64> {
    let mut closest_hit: Option<(u64, f32)> = None;

    world.each_model_entity(|entity_id, global, model| {
        if let Some(asset_model) = asset_server.get::<render::model::Model>(model.id) {
            if let Some(mesh) = asset_model.meshes.get(model.mesh_index) {
                let aabb_min = Vec3::from(mesh.aabb_min);
                let aabb_max = Vec3::from(mesh.aabb_max);

                if let Some(t) = ray.intersect_obb(aabb_min, aabb_max, global.0) {
                    if t > 0.0 && closest_hit.map_or(true, |(_, min_t)| t < min_t) {
                        closest_hit = Some((entity_id, t));
                    }
                }
            }
        }
    });

    world.each_light_entity(|entity_id, global, _| {
        let center = global.translation();
        if let Some(t) = ray.intersect_sphere(center, 0.5) {
            if t > 0.0 && closest_hit.map_or(true, |(_, min_t)| t < min_t) {
                closest_hit = Some((entity_id, t));
            }
        }
    });

    world.each_camera_entity(|entity_id, global, _| {
        let center = global.translation();
        if let Some(t) = ray.intersect_sphere(center, 0.5) {
            if t > 0.0 && closest_hit.map_or(true, |(_, min_t)| t < min_t) {
                closest_hit = Some((entity_id, t));
            }
        }
    });

    closest_hit.map(|(id, _)| id)
}

fn draw_spawn_marker(painter: &Painter, camera: &Camera, pos: Vec3, viewport_rect: Rect) {
    let view = camera::rh::view::look_at_mat4(camera.eye, camera.target, camera.up);
    let proj = camera::rh::proj::directx::perspective(
        camera.fovy.to_radians(),
        camera.aspect,
        camera.znear,
        camera.zfar,
    );
    let view_proj = proj * view;

    let stroke = Stroke::new(2.0, Color32::from_rgb(100, 220, 255));
    let radius = 0.6;
    let segments = 24;

    for i in 0..segments {
        let a0 = (i as f32 / segments as f32) * std::f32::consts::TAU;
        let a1 = ((i + 1) as f32 / segments as f32) * std::f32::consts::TAU;

        let p0 = pos + Vec3::new(a0.cos() * radius, 0.02, a0.sin() * radius);
        let p1 = pos + Vec3::new(a1.cos() * radius, 0.02, a1.sin() * radius);

        if let Some((s0, s1)) = project_3d_line(p0, p1, view_proj, viewport_rect) {
            painter.line_segment([s0, s1], stroke);
        }
    }

    let p_up0 = pos;
    let p_up1 = pos + Vec3::new(0.0, 1.0, 0.0);
    if let Some((s0, s1)) = project_3d_line(p_up0, p_up1, view_proj, viewport_rect) {
        painter.line_segment([s0, s1], stroke);
    }
}

fn project_3d_line(
    p0: Vec3,
    p1: Vec3,
    view_proj: Mat4,
    viewport_rect: Rect,
) -> Option<(Pos2, Pos2)> {
    let mut v0 = view_proj * p0.extend(1.0);
    let mut v1 = view_proj * p1.extend(1.0);

    let near = 0.001;
    let in0 = v0.w > near;
    let in1 = v1.w > near;

    if !in0 && !in1 {
        return None;
    }

    if !in0 {
        let t = (near - v0.w) / (v1.w - v0.w);
        v0 = v0.lerp(v1, t);
    } else if !in1 {
        let t = (near - v1.w) / (v0.w - v1.w);
        v1 = v1.lerp(v0, t);
    }

    let ndc0 = v0.truncate() / v0.w;
    let ndc1 = v1.truncate() / v1.w;

    let to_screen = |ndc: Vec3| -> Pos2 {
        let x = viewport_rect.min.x + (ndc.x * 0.5 + 0.5) * viewport_rect.width();
        let y = viewport_rect.min.y + (1.0 - (ndc.y * 0.5 + 0.5)) * viewport_rect.height();
        egui::pos2(x, y)
    };

    Some((to_screen(ndc0), to_screen(ndc1)))
}

pub fn draw_collider_wireframe(
    painter: &Painter,
    camera: &Camera,
    model_mat: Mat4,
    col: &ColliderComponent,
    viewport_rect: Rect,
) {
    let view = camera::rh::view::look_at_mat4(camera.eye, camera.target, camera.up);
    let proj = camera::rh::proj::directx::perspective(
        camera.fovy.to_radians(),
        camera.aspect,
        camera.znear,
        camera.zfar,
    );
    let view_proj = proj * view;

    let color = if col.is_sensor {
        Color32::from_rgb(255, 200, 0)
    } else {
        Color32::from_rgb(0, 255, 120)
    };
    let stroke = Stroke::new(1.5, color);

    let mut lines: Vec<(Vec3, Vec3)> = Vec::new();

    match col.shape_type {
        ecs::ColliderShapeType::Cuboid => {
            let hx = col.size[0].max(0.001);
            let hy = col.size[1].max(0.001);
            let hz = col.size[2].max(0.001);

            let v = [
                Vec3::new(-hx, -hy, -hz),
                Vec3::new(hx, -hy, -hz),
                Vec3::new(hx, hy, -hz),
                Vec3::new(-hx, hy, -hz),
                Vec3::new(-hx, -hy, hz),
                Vec3::new(hx, -hy, hz),
                Vec3::new(hx, hy, hz),
                Vec3::new(-hx, hy, hz),
            ];

            lines.extend_from_slice(&[
                (v[0], v[1]),
                (v[1], v[2]),
                (v[2], v[3]),
                (v[3], v[0]),
                (v[4], v[5]),
                (v[5], v[6]),
                (v[6], v[7]),
                (v[7], v[4]),
                (v[0], v[4]),
                (v[1], v[5]),
                (v[2], v[6]),
                (v[3], v[7]),
            ]);
        }
        ecs::ColliderShapeType::Sphere => {
            let r = col.size[0].max(0.001);
            let segments = 24;

            for i in 0..segments {
                let theta0 = (i as f32 / segments as f32) * std::f32::consts::TAU;
                let theta1 = ((i + 1) as f32 / segments as f32) * std::f32::consts::TAU;

                let (s0, c0) = (theta0.sin() * r, theta0.cos() * r);
                let (s1, c1) = (theta1.sin() * r, theta1.cos() * r);

                lines.push((Vec3::new(c0, s0, 0.0), Vec3::new(c1, s1, 0.0)));
                lines.push((Vec3::new(c0, 0.0, s0), Vec3::new(c1, 0.0, s1)));
                lines.push((Vec3::new(0.0, c0, s0), Vec3::new(0.0, c1, s1)));
            }
        }
        ecs::ColliderShapeType::Capsule => {
            let r = col.size[0].max(0.001);
            let h = col.size[1].max(0.001);
            let segments = 20;

            for i in 0..segments {
                let theta0 = (i as f32 / segments as f32) * std::f32::consts::TAU;
                let theta1 = ((i + 1) as f32 / segments as f32) * std::f32::consts::TAU;

                let (s0, c0) = (theta0.sin() * r, theta0.cos() * r);
                let (s1, c1) = (theta1.sin() * r, theta1.cos() * r);

                lines.push((Vec3::new(c0, h, s0), Vec3::new(c1, h, s1)));
                lines.push((Vec3::new(c0, -h, s0), Vec3::new(c1, -h, s1)));

                let phi0 = (i as f32 / segments as f32) * std::f32::consts::PI;
                let phi1 = ((i + 1) as f32 / segments as f32) * std::f32::consts::PI;
                let (sp0, cp0) = (phi0.sin() * r, phi0.cos() * r);
                let (sp1, cp1) = (phi1.sin() * r, phi1.cos() * r);

                lines.push((Vec3::new(sp0, h + cp0, 0.0), Vec3::new(sp1, h + cp1, 0.0)));
                lines.push((Vec3::new(0.0, h + cp0, sp0), Vec3::new(0.0, h + cp1, sp1)));
                lines.push((
                    Vec3::new(sp0, -h - cp0, 0.0),
                    Vec3::new(sp1, -h - cp1, 0.0),
                ));
                lines.push((
                    Vec3::new(0.0, -h - cp0, sp0),
                    Vec3::new(0.0, -h - cp1, sp1),
                ));
            }

            lines.push((Vec3::new(r, h, 0.0), Vec3::new(r, -h, 0.0)));
            lines.push((Vec3::new(-r, h, 0.0), Vec3::new(-r, -h, 0.0)));
            lines.push((Vec3::new(0.0, h, r), Vec3::new(0.0, -h, r)));
            lines.push((Vec3::new(0.0, h, -r), Vec3::new(0.0, -h, -r)));
        }
        ecs::ColliderShapeType::Cylinder => {
            let r = col.size[0].max(0.001);
            let h = col.size[1].max(0.001);
            let segments = 24;

            for i in 0..segments {
                let theta0 = (i as f32 / segments as f32) * std::f32::consts::TAU;
                let theta1 = ((i + 1) as f32 / segments as f32) * std::f32::consts::TAU;

                let (s0, c0) = (theta0.sin() * r, theta0.cos() * r);
                let (s1, c1) = (theta1.sin() * r, theta1.cos() * r);

                lines.push((Vec3::new(c0, h, s0), Vec3::new(c1, h, s1)));
                lines.push((Vec3::new(c0, -h, s0), Vec3::new(c1, -h, s1)));
            }

            lines.push((Vec3::new(r, h, 0.0), Vec3::new(r, -h, 0.0)));
            lines.push((Vec3::new(-r, h, 0.0), Vec3::new(-r, -h, 0.0)));
            lines.push((Vec3::new(0.0, h, r), Vec3::new(0.0, -h, r)));
            lines.push((Vec3::new(0.0, h, -r), Vec3::new(0.0, -h, -r)));
        }
    }

    for (l0, l1) in lines {
        let p0 = model_mat.transform_point3(l0);
        let p1 = model_mat.transform_point3(l1);

        if let Some((s0, s1)) = project_3d_line(p0, p1, view_proj, viewport_rect) {
            painter.line_segment([s0, s1], stroke);
        }
    }
}