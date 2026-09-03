use crate::ui::dock::TabKind;
use crate::ui::EditorContext;
use crate::EditorGizmoMode;
use ecs::serialization::{load_world, save_world};
use ecs::{CameraComponent, LightComponent, LightType, Transform};
use egui::{Color32, Context, RichText, Window};
use egui_dock::DockState;
use engine::game::{PlayControlAction, PlayState};
use transform_gizmo_egui::prelude::GizmoOrientation;

pub fn draw_toolbar(
    ctx: &Context,
    editor_ctx: &mut EditorContext,
    dock_state: &mut DockState<TabKind>,
) {
    Window::new("Toolbar")
        .default_pos([10.0, 10.0])
        .title_bar(false)
        .resizable(false)
        .show(ctx, |ui| {
            ui.horizontal(|ui| {
                // play mode controls
                let is_running = matches!(
                    editor_ctx.play_state,
                    PlayState::Playing | PlayState::Simulating
                );
                let is_paused = matches!(editor_ctx.play_state, PlayState::Paused { .. });

                if is_running || is_paused {
                    if ui
                        .button(RichText::new("⏹ Stop").color(Color32::LIGHT_RED))
                        .clicked()
                    {
                        *editor_ctx.play_action = PlayControlAction::Stop;
                    }
                    if ui
                        .button(if is_paused { "▶ Resume" } else { "⏸ Pause" })
                        .clicked()
                    {
                        *editor_ctx.play_action = if is_paused {
                            PlayControlAction::Resume
                        } else {
                            PlayControlAction::Pause
                        };
                    }
                    if is_paused && ui.button("⏭ Step").clicked() {
                        *editor_ctx.play_action = PlayControlAction::Step;
                    }

                    ui.separator();
                    if editor_ctx.play_state == PlayState::Simulating {
                        ui.label(
                            RichText::new("⚡ SIMULATING")
                                .color(Color32::from_rgb(100, 200, 255))
                                .strong(),
                        );
                    } else if editor_ctx.play_state == PlayState::Playing {
                        ui.label(
                            RichText::new("🎮 PLAYING")
                                .color(Color32::from_rgb(100, 255, 100))
                                .strong(),
                        );
                    }
                } else {
                    if ui
                        .button(RichText::new("▶ Play").color(Color32::GREEN))
                        .on_hover_text("Play mode with gameplay camera and controls")
                        .clicked()
                    {
                        *editor_ctx.play_action = PlayControlAction::Start;
                    }
                    if ui
                        .button(
                            RichText::new("⚡ Simulate").color(Color32::from_rgb(100, 200, 255)),
                        )
                        .on_hover_text("Run physics/scripts with editor camera")
                        .clicked()
                    {
                        *editor_ctx.play_action = PlayControlAction::Simulate;
                    }
                }

                ui.separator();

                // file menu
                ui.menu_button("File", |ui| {
                    if ui.button("Save Scene (F5)").clicked() {
                        if let Err(e) = save_world(editor_ctx.world, editor_ctx.world_save_path) {
                            log::error!("Failed to save scene: {e}");
                        } else {
                            log::info!("Scene saved to {}", editor_ctx.world_save_path);
                        }
                        TabKind::save_dock_state(dock_state);
                        ui.close();
                    }
                    if ui.button("Load Scene (F9)").clicked() {
                        *editor_ctx.selected_entity = None;
                        if let Err(e) = load_world(editor_ctx.world, editor_ctx.world_save_path) {
                            log::error!("Failed to load scene: {e}");
                        } else {
                            log::info!("Scene loaded from {}", editor_ctx.world_save_path);
                        }
                        ui.close();
                    }
                    ui.separator();
                    if ui.button("Clear Scene").clicked() {
                        editor_ctx.clear_requested = true;
                        ui.close();
                    }
                    ui.separator();
                    if ui.button("Quit").clicked() {
                        TabKind::save_dock_state(dock_state);
                        std::process::exit(0);
                    }
                });

                // spawn menu
                ui.menu_button("➕ Spawn", |ui| {
                    if ui.button("Empty Entity").clicked() {
                        let id = editor_ctx.world.spawn_empty("Empty Entity");
                        *editor_ctx.selected_entity = Some(id);
                        ui.close();
                    }
                    if ui.button("📷 Camera").clicked() {
                        let id = editor_ctx.world.spawn_empty("Camera");
                        let pos = editor_ctx.camera.eye;
                        editor_ctx
                            .world
                            .add_transform(id, Transform::from_translation(pos));
                        editor_ctx
                            .world
                            .add_camera(id, CameraComponent::default());
                        *editor_ctx.selected_entity = Some(id);
                        ui.close();
                    }
                    if ui.button("💡 Point Light").clicked() {
                        let id = editor_ctx.world.spawn_empty("Point Light");
                        let forward =
                            (editor_ctx.camera.target - editor_ctx.camera.eye).normalize();
                        let pos = editor_ctx.camera.eye + forward * 3.0;
                        editor_ctx
                            .world
                            .add_transform(id, Transform::from_translation(pos));
                        editor_ctx.world.add_light(
                            id,
                            LightComponent {
                                light_type: LightType::Point,
                                color: [1.0, 1.0, 1.0],
                                intensity: 10.0,
                                range: 15.0,
                                inner_angle: 0.0,
                                outer_angle: 0.0,
                            },
                        );
                        *editor_ctx.selected_entity = Some(id);
                        ui.close();
                    }
                    if ui.button("☀️ Directional Light").clicked() {
                        let id = editor_ctx.world.spawn_empty("Directional Light");
                        let pos = glam::Vec3::new(0.0, 10.0, 0.0);
                        let rot = glam::Quat::from_rotation_x(-0.7);
                        editor_ctx.world.add_transform(
                            id,
                            Transform {
                                position: pos,
                                rotation: rot,
                                scale: glam::Vec3::ONE,
                            },
                        );
                        editor_ctx.world.add_light(
                            id,
                            LightComponent {
                                light_type: LightType::Directional,
                                color: [1.0, 0.95, 0.8],
                                intensity: 5.0,
                                range: 100.0,
                                inner_angle: 0.0,
                                outer_angle: 0.0,
                            },
                        );
                        *editor_ctx.selected_entity = Some(id);
                        ui.close();
                    }
                });

                ui.separator();

                // layout menu
                ui.menu_button("Layout", |ui| {
                    if ui.button("💾 Save Layout").clicked() {
                        TabKind::save_dock_state(dock_state);
                        ui.close();
                    }
                    if ui.button("↺ Reset to Defaults").clicked() {
                        *dock_state = TabKind::default_dock_state();
                        TabKind::save_dock_state(dock_state);
                        ui.close();
                    }
                });

                ui.separator();

                // gizmo mode selectors
                ui.selectable_value(
                    editor_ctx.gizmo_mode,
                    EditorGizmoMode::Translate,
                    "Translate (1)",
                );
                ui.selectable_value(
                    editor_ctx.gizmo_mode,
                    EditorGizmoMode::Rotate,
                    "Rotate (2)",
                );
                ui.selectable_value(
                    editor_ctx.gizmo_mode,
                    EditorGizmoMode::Scale,
                    "Scale (3)",
                );
                ui.selectable_value(editor_ctx.gizmo_mode, EditorGizmoMode::All, "All (4)");

                ui.separator();

                if ui
                    .button(match editor_ctx.gizmo_orientation {
                        GizmoOrientation::Global => "🌐 Global (Q)",
                        GizmoOrientation::Local => "📦 Local (Q)",
                    })
                    .clicked()
                {
                    *editor_ctx.gizmo_orientation = match editor_ctx.gizmo_orientation {
                        GizmoOrientation::Global => GizmoOrientation::Local,
                        GizmoOrientation::Local => GizmoOrientation::Global,
                    };
                }
            });
        });
}