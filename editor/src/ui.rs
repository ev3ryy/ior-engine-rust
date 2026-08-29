use egui::{Context, Ui};
use ecs::{
    serialization, LightComponent, LightType, ModelComponent, Transform, World,
};
use engine::{camera::Camera, EngineMetrics};
use render::postprocess::{PostProcessSettings, TonemappingMode};
use transform_gizmo_egui::math::{DMat4, DQuat, DVec3, Transform as GizmoTransform};
use transform_gizmo_egui::prelude::*;
use crate::EditorGizmoMode;

fn draw_entity_node(
    ui: &mut Ui,
    world: &mut World,
    entity_id: u64,
    selected_entity: &mut Option<u64>,
) {
    if !world.is_alive(entity_id) {
        return;
    }

    let is_selected = *selected_entity == Some(entity_id);
    let name = world.get_entity_name(entity_id);
    let display_name = if name.is_empty() {
        format!("Entity {}", entity_id)
    } else {
        name
    };

    let has_light = world.has_light(entity_id);
    let has_model = world.has_model(entity_id);

    let icon = if has_light {
        "💡"
    } else if has_model {
        "🔷"
    } else {
        "📁"
    };

    let header_text = format!("{} {} ({})", icon, display_name, entity_id);
    let header_id = ui.make_persistent_id(entity_id);
    let has_children = world.has_children(entity_id);

    if has_children {
        egui::collapsing_header::CollapsingState::load_with_default_open(ui.ctx(), header_id, true)
            .show_header(ui, |ui| {
                if ui.selectable_label(is_selected, &header_text).clicked() {
                    if is_selected {
                        *selected_entity = None;
                    } else {
                        *selected_entity = Some(entity_id);
                    }
                }
            })
            .body(|ui| {
                let children = world.get_children(entity_id);
                for child_id in children {
                    draw_entity_node(ui, world, child_id, selected_entity);
                }
            });
    } else {
        if ui.selectable_label(is_selected, &header_text).clicked() {
            if is_selected {
                *selected_entity = None;
            } else {
                *selected_entity = Some(entity_id);
            }
        }
    }
}

pub fn draw_postprocess_window(ctx: &Context, pp: &mut PostProcessSettings) {
    egui::Window::new("Post-Processing")
        .default_pos([870.0, 10.0])
        .default_size([280.0, 400.0])
        .show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.checkbox(&mut pp.enabled, "Enable Post-Processing");
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.button("↺ Reset").on_hover_text("Reset to Defaults").clicked() {
                        *pp = PostProcessSettings::default();
                    }
                });
            });
            ui.separator();

            if !pp.enabled {
                ui.label(egui::RichText::new("Post-processing pipeline is bypassed.").italics());
                return;
            }

            egui::ScrollArea::vertical().show(ui, |ui| {
                // 1. Tonemapping & Exposure
                ui.group(|ui| {
                    ui.label(egui::RichText::new("📷 Tonemapping & Exposure").strong());

                    egui::ComboBox::from_label("Tonemap")
                        .selected_text(match pp.tonemapping_mode {
                            TonemappingMode::None => "None (Clamp)",
                            TonemappingMode::ACES => "ACES Filmic",
                            TonemappingMode::Reinhard => "Reinhard",
                        })
                        .show_ui(ui, |ui| {
                            ui.selectable_value(&mut pp.tonemapping_mode, TonemappingMode::ACES, "ACES Filmic");
                            ui.selectable_value(&mut pp.tonemapping_mode, TonemappingMode::Reinhard, "Reinhard");
                            ui.selectable_value(&mut pp.tonemapping_mode, TonemappingMode::None, "None (Clamp)");
                        });

                    ui.add(egui::Slider::new(&mut pp.exposure, 0.01..=5.0).text("Exposure").logarithmic(true));
                    ui.checkbox(&mut pp.manual_gamma_correction, "Manual Gamma (1 / 2.2)");
                });
                ui.add_space(5.0);

                // 2. Bloom
                ui.group(|ui| {
                    ui.horizontal(|ui| {
                        ui.label(egui::RichText::new("✨ Bloom (Dual Filtering)").strong());
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            ui.checkbox(&mut pp.bloom_enabled, "");
                        });
                    });

                    if pp.bloom_enabled {
                        ui.add(egui::Slider::new(&mut pp.bloom_intensity, 0.0..=0.5).text("Intensity"));
                        ui.add(egui::Slider::new(&mut pp.bloom_threshold, 0.0..=10.0).text("Threshold"));
                        ui.add(egui::Slider::new(&mut pp.bloom_knee, 0.0..=1.0).text("Knee (Softness)"));
                        ui.add(egui::Slider::new(&mut pp.bloom_scatter, 0.1..=2.0).text("Scatter / Radius"));
                    }
                });
                ui.add_space(5.0);

                // 3. Color Grading
                ui.group(|ui| {
                    ui.label(egui::RichText::new("🎨 Color Grading").strong());
                    ui.add(egui::Slider::new(&mut pp.contrast, 0.5..=2.0).text("Contrast"));
                    ui.add(egui::Slider::new(&mut pp.saturation, 0.0..=2.0).text("Saturation"));
                });
                ui.add_space(5.0);

                // 4. Vignette
                ui.group(|ui| {
                    ui.horizontal(|ui| {
                        ui.label(egui::RichText::new("⭕ Vignette").strong());
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            ui.checkbox(&mut pp.vignette_enabled, "");
                        });
                    });

                    if pp.vignette_enabled {
                        ui.add(egui::Slider::new(&mut pp.vignette_intensity, 0.0..=1.0).text("Intensity"));
                        ui.add(egui::Slider::new(&mut pp.vignette_radius, 0.1..=1.5).text("Radius"));
                        ui.add(egui::Slider::new(&mut pp.vignette_smoothness, 0.01..=1.0).text("Smoothness"));
                    }
                });
                ui.add_space(5.0);

                // 5. Film Grain
                ui.group(|ui| {
                    ui.horizontal(|ui| {
                        ui.label(egui::RichText::new("🎞 Film Grain").strong());
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            ui.checkbox(&mut pp.grain_enabled, "");
                        });
                    });

                    if pp.grain_enabled {
                        ui.add(egui::Slider::new(&mut pp.grain_intensity, 0.0..=0.1).text("Intensity"));
                    }
                });
            });
        });
}

pub fn draw_editor_ui(
    ctx: &Context,
    world: &mut World,
    camera: &mut Camera,
    postprocess: &mut PostProcessSettings,
    metrics: &EngineMetrics,
    load_requests: &mut Vec<String>,
    selected_entity: &mut Option<u64>,
    gizmo: &mut Gizmo,
    gizmo_mode: &mut EditorGizmoMode,
    gizmo_orientation: &mut GizmoOrientation,
) {
    let mut clear_requested = false;

    draw_postprocess_window(ctx, postprocess);

    // 3D Transform Gizmo Viewport
    if let Some(entity_id) = *selected_entity {
        if let Some(model_mat) = world.get_global_transform(entity_id) {
            let eye = DVec3::new(camera.eye.x as f64, camera.eye.y as f64, camera.eye.z as f64);
            let target = DVec3::new(camera.target.x as f64, camera.target.y as f64, camera.target.z as f64);
            let up = DVec3::new(camera.up.x as f64, camera.up.y as f64, camera.up.z as f64);

            let view_mat = DMat4::look_at_rh(eye, target, up);
            let proj_mat = DMat4::perspective_infinite_rh(
                (camera.fovy as f64).to_radians(),
                camera.aspect as f64,
                camera.znear as f64,
            );

            let modes: EnumSet<GizmoMode> = match gizmo_mode {
                EditorGizmoMode::Translate => GizmoMode::all().into_iter().filter(|m| m.is_translate()).collect(),
                EditorGizmoMode::Rotate => GizmoMode::all().into_iter().filter(|m| m.is_rotate()).collect(),
                EditorGizmoMode::Scale => GizmoMode::all().into_iter().filter(|m| m.is_scale()).collect(),
                EditorGizmoMode::All => GizmoMode::all(),
            };

            gizmo.update_config(GizmoConfig {
                view_matrix: view_mat.into(),
                projection_matrix: proj_mat.into(),
                modes,
                orientation: *gizmo_orientation,
                ..Default::default()
            });

            let (scale, rot, pos) = model_mat.to_scale_rotation_translation();
            let transform = GizmoTransform::from_scale_rotation_translation(
                DVec3::new(scale.x as f64, scale.y as f64, scale.z as f64),
                DQuat::from_xyzw(rot.x as f64, rot.y as f64, rot.z as f64, rot.w as f64),
                DVec3::new(pos.x as f64, pos.y as f64, pos.z as f64),
            );

            egui::Area::new(egui::Id::new("gizmo_viewport"))
                .order(egui::Order::Background)
                .fixed_pos([0.0, 0.0])
                .show(ctx, |ui| {
                    if let Some((_result, new_transforms)) = gizmo.interact(ui, &[transform]) {
                        if let Some(new_t) = new_transforms.first() {
                            let s = new_t.scale;
                            let r = new_t.rotation;
                            let p = new_t.translation;

                            let new_global = glam::Mat4::from_scale_rotation_translation(
                                glam::Vec3::new(s.x as f32, s.y as f32, s.z as f32),
                                glam::Quat::from_xyzw(r.v.x as f32, r.v.y as f32, r.v.z as f32, r.s as f32),
                                glam::Vec3::new(p.x as f32, p.y as f32, p.z as f32),
                            );

                            world.set_transform_from_global_matrix(entity_id, new_global);
                            world.update_transforms();
                        }
                    }
                });
        }
    }

    // Toolbar
    egui::Window::new("Toolbar")
        .default_pos([10.0, 10.0])
        .title_bar(false)
        .resizable(false)
        .show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.menu_button("File", |ui| {
                    if ui.button("Save Scene (F5)").clicked() {
                        if let Err(e) = serialization::save_world(world, "res/saved_scene.bin") {
                            log::error!("Failed to save scene: {e}");
                        } else {
                            log::info!("Scene saved to res/saved_scene.bin");
                        }
                    }
                    if ui.button("Load Scene (F9)").clicked() {
                        if let Err(e) = serialization::load_world(world, "res/saved_scene.bin") {
                            log::error!("Failed to load scene: {e}");
                        } else {
                            log::info!("Scene loaded from res/saved_scene.bin");
                        }
                    }
                    if ui.button("Clear Scene").clicked() {
                        clear_requested = true;
                    }
                    ui.separator();
                    if ui.button("Quit").clicked() {
                        std::process::exit(0);
                    }
                });

                ui.menu_button("➕ Spawn", |ui| {
                    if ui.button("Empty Entity").clicked() {
                        let id = world.spawn_empty("Empty Entity");
                        *selected_entity = Some(id);
                    }
                    if ui.button("💡 Point Light").clicked() {
                        let id = world.spawn_empty("Point Light");
                        let pos = camera.eye + (camera.target - camera.eye).normalize() * 3.0;
                        world.add_transform(id, Transform::from_translation(pos));
                        world.add_light(id, LightComponent {
                            light_type: LightType::Point,
                            color: [1.0, 1.0, 1.0],
                            intensity: 10.0,
                            range: 15.0,
                            inner_angle: 0.0,
                            outer_angle: 0.0,
                        });
                        *selected_entity = Some(id);
                    }
                    if ui.button("☀️ Directional Light").clicked() {
                        let id = world.spawn_empty("Directional Light");
                        let pos = glam::Vec3::new(0.0, 10.0, 0.0);
                        let rot = glam::Quat::from_rotation_x(-0.7);
                        world.add_transform(id, Transform {
                            position: pos,
                            rotation: rot,
                            scale: glam::Vec3::ONE,
                        });
                        world.add_light(id, LightComponent {
                            light_type: LightType::Directional,
                            color: [1.0, 0.95, 0.8],
                            intensity: 5.0,
                            range: 100.0,
                            inner_angle: 0.0,
                            outer_angle: 0.0,
                        });
                        *selected_entity = Some(id);
                    }
                });

                ui.separator();

                ui.selectable_value(gizmo_mode, EditorGizmoMode::Translate, "Translate (W)");
                ui.selectable_value(gizmo_mode, EditorGizmoMode::Rotate, "Rotate (E)");
                ui.selectable_value(gizmo_mode, EditorGizmoMode::Scale, "Scale (R)");
                ui.selectable_value(gizmo_mode, EditorGizmoMode::All, "All (T)");

                ui.separator();

                if ui.button(match gizmo_orientation {
                    GizmoOrientation::Global => "🌐 Global (Q)",
                    GizmoOrientation::Local => "📦 Local (Q)",
                }).clicked() {
                    *gizmo_orientation = match gizmo_orientation {
                        GizmoOrientation::Global => GizmoOrientation::Local,
                        GizmoOrientation::Local => GizmoOrientation::Global,
                    };
                }

                ui.separator();

                ui.menu_button("View", |ui| {
                    ui.checkbox(&mut true, "Show Inspector");
                    ui.checkbox(&mut true, "Show Metrics");
                });
            });
        });

    // Scene Hierarchy Window
    egui::Window::new("Scene Hierarchy")
        .default_pos([10.0, 50.0])
        .default_size([250.0, 450.0])
        .show(ctx, |ui| {
            ui.horizontal(|ui| {
                if ui.button("➕ Empty").clicked() {
                    let id = world.spawn_empty("New Entity");
                    *selected_entity = Some(id);
                }
                if let Some(parent_id) = *selected_entity {
                    if ui.button("➕ Child").clicked() {
                        let id = world.spawn_child(parent_id, "Child Entity");
                        *selected_entity = Some(id);
                    }
                }
            });
            ui.separator();

            egui::ScrollArea::vertical().show(ui, |ui| {
                let roots = world.get_root_entities();
                if roots.is_empty() {
                    ui.label(egui::RichText::new("Scene is empty.").italics());
                } else {
                    for root_id in roots {
                        draw_entity_node(ui, world, root_id, selected_entity);
                    }
                }
            });
        });

    // Inspector
    egui::Window::new("Inspector")
        .default_pos([270.0, 50.0])
        .default_size([300.0, 500.0])
        .show(ctx, |ui| {
            if let Some(entity_id) = *selected_entity {
                if !world.is_alive(entity_id) {
                    *selected_entity = None;
                    return;
                }

                // Header & Name
                ui.horizontal(|ui| {
                    let mut name = world.get_entity_name(entity_id);
                    if ui.text_edit_singleline(&mut name).changed() {
                        world.set_entity_name(entity_id, &name);
                    }
                    if ui.button("✖ Deselect").clicked() {
                        *selected_entity = None;
                        return;
                    }
                });
                ui.label(format!("Entity ID: {}", entity_id));
                ui.separator();

                egui::ScrollArea::vertical().show(ui, |ui| {
                    let has_transform = world.has_transform(entity_id);
                    let has_light = world.has_light(entity_id);
                    let has_model = world.has_model(entity_id);

                    // 1. TRANSFORM COMPONENT
                    if has_transform {
                        ui.group(|ui| {
                            ui.horizontal(|ui| {
                                ui.label(egui::RichText::new("📍 Transform").strong());
                                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                    if ui.button("❌").on_hover_text("Remove Transform").clicked() {
                                        world.remove_transform(entity_id);
                                    }
                                });
                            });

                            world.get_transform_mut(entity_id, |transform| {
                                ui.horizontal(|ui| {
                                    ui.add(egui::DragValue::new(&mut transform.position.x).speed(0.1).prefix("X: "));
                                    ui.add(egui::DragValue::new(&mut transform.position.y).speed(0.1).prefix("Y: "));
                                    ui.add(egui::DragValue::new(&mut transform.position.z).speed(0.1).prefix("Z: "));
                                });

                                let (rx, ry, rz) = transform.rotation.to_euler(glam::EulerRot::XYZ);
                                let mut deg_x = rx.to_degrees();
                                let mut deg_y = ry.to_degrees();
                                let mut deg_z = rz.to_degrees();

                                let mut rot_changed = false;
                                ui.horizontal(|ui| {
                                    rot_changed |= ui.add(egui::DragValue::new(&mut deg_x).speed(0.5).prefix("R_X: ").suffix("°")).changed();
                                    rot_changed |= ui.add(egui::DragValue::new(&mut deg_y).speed(0.5).prefix("R_Y: ").suffix("°")).changed();
                                    rot_changed |= ui.add(egui::DragValue::new(&mut deg_z).speed(0.5).prefix("R_Z: ").suffix("°")).changed();
                                });

                                if rot_changed {
                                    transform.rotation = glam::Quat::from_euler(
                                        glam::EulerRot::XYZ,
                                        deg_x.to_radians(),
                                        deg_y.to_radians(),
                                        deg_z.to_radians(),
                                    );
                                }

                                ui.horizontal(|ui| {
                                    ui.add(egui::DragValue::new(&mut transform.scale.x).speed(0.1).prefix("S_X: "));
                                    ui.add(egui::DragValue::new(&mut transform.scale.y).speed(0.1).prefix("S_Y: "));
                                    ui.add(egui::DragValue::new(&mut transform.scale.z).speed(0.1).prefix("S_Z: "));
                                });
                            });
                        });
                        ui.add_space(5.0);
                    }

                    // 2. LIGHT COMPONENT
                    if has_light {
                        ui.group(|ui| {
                            ui.horizontal(|ui| {
                                ui.label(egui::RichText::new("💡 Light Component").strong());
                                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                    if ui.button("❌").on_hover_text("Remove Light").clicked() {
                                        world.remove_light(entity_id);
                                    }
                                });
                            });

                            world.get_light_mut(entity_id, |light| {
                                egui::ComboBox::from_label("Type")
                                    .selected_text(match light.light_type {
                                        LightType::Directional => "Directional",
                                        LightType::Point => "Point",
                                        LightType::Spot => "Spot",
                                    })
                                    .show_ui(ui, |ui| {
                                        ui.selectable_value(&mut light.light_type, LightType::Point, "Point");
                                        ui.selectable_value(&mut light.light_type, LightType::Directional, "Directional");
                                        ui.selectable_value(&mut light.light_type, LightType::Spot, "Spot");
                                    });

                                ui.horizontal(|ui| {
                                    ui.label("Color:");
                                    let mut color = [light.color[0], light.color[1], light.color[2]];
                                    ui.color_edit_button_rgb(&mut color);
                                    light.color = color;
                                });

                                ui.add(egui::Slider::new(&mut light.intensity, 0.0..=1000.0).text("Intensity"));

                                match light.light_type {
                                    LightType::Point | LightType::Spot => {
                                        ui.add(egui::Slider::new(&mut light.range, 0.1..=200.0).text("Range"));
                                    }
                                    _ => {}
                                }

                                if let LightType::Spot = light.light_type {
                                    let mut inner_deg = light.inner_angle.to_degrees();
                                    let mut outer_deg = light.outer_angle.to_degrees();

                                    ui.add(egui::Slider::new(&mut inner_deg, 0.0..=90.0).text("Inner Cone"));
                                    ui.add(egui::Slider::new(&mut outer_deg, 0.0..=90.0).text("Outer Cone"));

                                    light.inner_angle = inner_deg.to_radians();
                                    light.outer_angle = outer_deg.to_radians().max(light.inner_angle + 0.01);
                                }
                            });
                        });
                        ui.add_space(5.0);
                    }

                    // 3. MESH COMPONENT
                    if has_model {
                        ui.group(|ui| {
                            ui.horizontal(|ui| {
                                ui.label(egui::RichText::new("🔷 Mesh Component").strong());
                                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                    if ui.button("❌").on_hover_text("Remove Mesh").clicked() {
                                        world.remove_model(entity_id);
                                    }
                                });
                            });

                            world.get_model(entity_id, |model| {
                                ui.label(format!("Model ID: {:?}", model.id.path()));
                                ui.label(format!("Mesh Index: {}", model.mesh_index));
                            });
                        });
                        ui.add_space(5.0);
                    }

                    // ➕ ADD COMPONENT MENU
                    ui.separator();
                    ui.menu_button("➕ Add Component", |ui| {
                        if !has_transform {
                            if ui.button("📍 Transform").clicked() {
                                world.add_transform(entity_id, Transform::default());
                                ui.close();
                            }
                        }
                        if !has_light {
                            if ui.button("💡 Light Component").clicked() {
                                world.add_light(entity_id, LightComponent {
                                    light_type: LightType::Point,
                                    color: [1.0, 1.0, 1.0],
                                    intensity: 10.0,
                                    range: 15.0,
                                    inner_angle: 0.0,
                                    outer_angle: 0.0,
                                });
                                ui.close();
                            }
                        }
                    });

                    ui.add_space(15.0);
                    if ui.button(egui::RichText::new("🗑 Delete Entity").color(egui::Color32::RED)).clicked() {
                        world.delete_entity(entity_id);
                        *selected_entity = None;
                    }
                });
            } else {
                ui.label(egui::RichText::new("Select an entity from Scene Hierarchy to inspect.").italics());
            }
        });

    egui::Window::new("Performance & Camera")
        .default_pos([600.0, 10.0])
        .default_size([250.0, 300.0])
        .show(ctx, |ui| {
            ui.heading("Performance");
            ui.separator();

            ui.label(format!("FPS: {:.1}", metrics.fps));
            ui.label(format!("Frame Time: {:.2} ms", metrics.frame_time_ms));
            ui.label(format!("Draw Calls: {}", metrics.draw_calls));
            ui.label(format!("Total Instances: {}", metrics.instance_count));

            ui.add_space(20.0);

            ui.heading("Camera");
            ui.separator();

            ui.label("Position");
            ui.horizontal(|ui| {
                ui.add(egui::DragValue::new(&mut camera.eye.x).speed(0.1));
                ui.add(egui::DragValue::new(&mut camera.eye.y).speed(0.1));
                ui.add(egui::DragValue::new(&mut camera.eye.z).speed(0.1));
            });

            ui.add_space(10.0);
            ui.add(egui::Slider::new(&mut camera.fovy, 10.0..=120.0).text("FOV"));
        });

    egui::Window::new("Asset Browser")
        .default_pos([10.0, 600.0])
        .default_size([300.0, 200.0])
        .show(ctx, |ui| {
            let res_path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../res");

            ui.heading("Models (.bin)");
            ui.separator();

            egui::ScrollArea::vertical().show(ui, |ui| {
                if let Ok(entries) = std::fs::read_dir(&res_path) {
                    for entry in entries.flatten() {
                        if let Some(name) = entry.file_name().to_str() {
                            if name.ends_with(".bin") {
                                let _ = asset::AssetId::from_path(name);

                                ui.horizontal(|ui| {
                                    ui.label(name);
                                    if ui.button("Spawn").clicked() {
                                        if let Ok(data) = std::fs::read(entry.path()) {
                                            if let Ok(baked_model) = rkyv::from_bytes::<shared::BakedModel, rkyv::rancor::Error>(&data) {
                                                let model_id = asset::AssetId::from_path(name);

                                                let spawned_id = ecs::spawn_scene(world, model_id, &baked_model, glam::Vec3::ZERO);
                                                *selected_entity = Some(spawned_id);

                                                load_requests.push(name.to_string());

                                                log::info!("Spawned {} in ECS", name);
                                            }
                                        }
                                    }
                                });
                            }
                        }
                    }
                } else {
                    ui.label(egui::RichText::new("Failed to read res/ directory").color(egui::Color32::RED));
                }
            });
        });

    if clear_requested {
        world.clear_scene();
        *selected_entity = None;

        log::info!("Scene cleared");
    }
}