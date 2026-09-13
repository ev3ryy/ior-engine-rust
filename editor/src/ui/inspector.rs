use crate::ui::EditorContext;
use ecs::*;
use egui::{Color32, DragValue, RichText, ScrollArea, Slider, Ui};
use std::collections::BTreeMap;

fn draw_ron_field(ui: &mut Ui, val: &mut ron::Value) -> bool {
    let mut changed = false;
    match val {
        ron::Value::Bool(b) => {
            changed |= ui.checkbox(b, "").changed();
        }
        ron::Value::Number(_) => {
            if let Ok(mut f) = val.clone().into_rust::<f64>() {
                let is_integer = val.clone().into_rust::<i64>().is_ok()
                    && !ron::to_string(val).map(|s| s.contains('.')).unwrap_or(false);

                if is_integer {
                    let mut i = f as i64;
                    if ui.add(DragValue::new(&mut i).speed(1.0)).changed() {
                        *val = ron::Value::from(i);
                        changed = true;
                    }
                } else {
                    if ui.add(DragValue::new(&mut f).speed(0.1)).changed() {
                        *val = ron::Value::from(f);
                        changed = true;
                    }
                }
            }
        }
        ron::Value::String(s) => {
            changed |= ui.text_edit_singleline(s).changed();
        }
        ron::Value::Map(map) => {
            egui::Grid::new(ui.next_auto_id())
                .num_columns(2)
                .spacing([12.0, 4.0])
                .show(ui, |ui| {
                    for (k, v) in map.iter_mut() {
                        let field_name = match k {
                            ron::Value::String(s) => s.as_str(),
                            _ => "field",
                        };

                        ui.label(RichText::new(field_name).color(Color32::from_gray(200)));
                        changed |= draw_ron_field(ui, v);
                        ui.end_row();
                    }
                });
        }
        ron::Value::Seq(seq) => {
            ui.vertical(|ui| {
                for (idx, item) in seq.iter_mut().enumerate() {
                    ui.horizontal(|ui| {
                        ui.label(RichText::new(format!("[{}]", idx)).weak());
                        changed |= draw_ron_field(ui, item);
                    });
                }
            });
        }
        ron::Value::Option(opt) => {
            if let Some(inner) = opt {
                changed |= draw_ron_field(ui, inner);
            } else {
                ui.label(RichText::new("None").weak());
            }
        }
        ron::Value::Unit => {
            ui.label(RichText::new("Tag (Active)").weak().italics());
        }
        ron::Value::Char(c) => {
            let mut s = c.to_string();
            if ui.add(egui::TextEdit::singleline(&mut s).desired_width(20.0)).changed() {
                if let Some(first_char) = s.chars().next() {
                    *c = first_char;
                    changed = true;
                }
            }
        }
        ron::Value::Bytes(bytes) => {
            ui.label(RichText::new(format!("Bytes ({} B)", bytes.len())).weak());
        }
    }
    changed
}

pub struct ComponentInspector {
    pub id: &'static str,
    pub display_name: &'static str,
    pub icon: &'static str,
    pub category: &'static str,
    pub has: fn(&World, u64) -> bool,
    pub remove: fn(&World, u64),
    pub add_default: fn(&World, u64),
    pub draw: fn(&mut Ui, &World, u64) -> bool,
}

pub fn get_builtin_inspectors() -> &'static [ComponentInspector] {
    &[
        ComponentInspector {
            id: "PrefabRef",
            display_name: "Prefab Instance",
            icon: "🧩",
            category: "Core",
            has: |w, id| w.has_prefab_ref(id),
            remove: |w, id| w.remove_prefab_ref(id),
            add_default: |_w, _id| {},
            draw: draw_prefab_inspector,
        },
        ComponentInspector {
            id: "Transform",
            display_name: "Transform",
            icon: "📍",
            category: "Spatial",
            has: |w, id| w.has_transform(id),
            remove: |w, id| w.remove_transform(id),
            add_default: |w, id| w.add_transform(id, Transform::default()),
            draw: draw_transform_inspector,
        },
        ComponentInspector {
            id: "CameraComponent",
            display_name: "Camera Component",
            icon: "📷",
            category: "Rendering",
            has: |w, id| w.has_camera(id),
            remove: |w, id| w.remove_camera(id),
            add_default: |w, id| w.add_camera(id, CameraComponent::default()),
            draw: draw_camera_inspector,
        },
        ComponentInspector {
            id: "LightComponent",
            display_name: "Light Component",
            icon: "💡",
            category: "Rendering",
            has: |w, id| w.has_light(id),
            remove: |w, id| w.remove_light(id),
            add_default: |w, id| w.add_light(id, LightComponent::default()),
            draw: draw_light_inspector,
        },
        ComponentInspector {
            id: "ModelComponent",
            display_name: "Mesh Component",
            icon: "🔷",
            category: "Rendering",
            has: |w, id| w.has_model(id),
            remove: |w, id| w.remove_model(id),
            add_default: |_w, _id| {},
            draw: draw_model_inspector,
        },
        ComponentInspector {
            id: "RigidBodyComponent",
            display_name: "RigidBody",
            icon: "📦",
            category: "Physics",
            has: |w, id| w.has_rigid_body(id),
            remove: |w, id| w.remove_rigid_body(id),
            add_default: |w, id| w.add_rigid_body(id, RigidBodyComponent::default()),
            draw: draw_rigidbody_inspector,
        },
        ComponentInspector {
            id: "ColliderComponent",
            display_name: "Collider",
            icon: "🛡",
            category: "Physics",
            has: |w, id| w.has_collider(id),
            remove: |w, id| w.remove_collider(id),
            add_default: |w, id| w.add_collider(id, ColliderComponent::default()),
            draw: draw_collider_inspector,
        },
        ComponentInspector {
            id: "CharacterControllerComponent",
            display_name: "Character Controller (KCC)",
            icon: "🏃",
            category: "Physics",
            has: |w, id| w.has_character_controller(id),
            remove: |w, id| w.remove_character_controller(id),
            add_default: |w, id| w.add_character_controller(id, CharacterControllerComponent::default()),
            draw: draw_kcc_inspector,
        },
    ]
}

pub fn draw_inspector(ui: &mut Ui, ctx: &mut EditorContext) {
    let Some(entity_id) = *ctx.selected_entity else {
        ui.label(RichText::new("Select an entity from Scene Hierarchy to inspect.").italics());
        return;
    };

    if !ctx.world.is_alive(entity_id) {
        *ctx.selected_entity = None;
        return;
    }

    draw_entity_header(ui, ctx, entity_id);
    let e = ecs::Entity(entity_id);
    ui.label(
        RichText::new(format!("Entity: #{} (raw: {})", e.format_short(), entity_id))
            .weak()
            .size(11.0),
    );
    ui.separator();

    ScrollArea::vertical().show(ui, |ui| {
        let inspectors = get_builtin_inspectors();
        let mut need_transform_update = false;

        for inspector in inspectors {
            if (inspector.has)(ctx.world, entity_id) {
                ui.group(|ui| {
                    ui.horizontal(|ui| {
                        ui.label(RichText::new(format!("{} {}", inspector.icon, inspector.display_name)).strong());
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if inspector.id != "PrefabRef" {
                                if ui.button("❌").on_hover_text("Remove Component").clicked() {
                                    (inspector.remove)(ctx.world, entity_id);
                                }
                            } else if ui.button("❌").on_hover_text("Unlink from prefab").clicked() {
                                (inspector.remove)(ctx.world, entity_id);
                            }
                        });
                    });

                    let changed = (inspector.draw)(ui, ctx.world, entity_id);
                    if changed && inspector.id == "Transform" {
                        need_transform_update = true;
                    }
                });
                ui.add_space(5.0);
            }
        }

        if need_transform_update {
            ctx.world.update_transforms();
        }

        let serde_registry = ctx.world.get_serialization_registry();
        for entry in serde_registry.iter() {
            if inspectors.iter().any(|i| i.id == entry.name) || entry.name == "Static" || entry.name == "Dynamic" {
                continue;
            }

            if (entry.has)(ctx.world, entity_id.into()) {
                ui.group(|ui| {
                    ui.horizontal(|ui| {
                        ui.label(RichText::new(format!("🎮 {}", entry.name)).strong().color(Color32::LIGHT_GREEN));
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if ui.button("❌").on_hover_text(format!("Remove {}", entry.name)).clicked() {
                                (entry.remove)(ctx.world, entity_id.into());
                            }
                        });
                    });

                    if let Some(ron_str) = (entry.serialize)(ctx.world, entity_id.into()) {
                        if let Ok(mut ron_val) = ron::from_str::<ron::Value>(&ron_str) {
                            ui.add_space(2.0);
                            
                            let changed = draw_ron_field(ui, &mut ron_val);

                            if changed {
                                if let Ok(new_ron_str) = ron::to_string(&ron_val) {
                                    let _ = (entry.deserialize)(ctx.world, entity_id.into(), &new_ron_str);
                                }
                            }
                        }
                    }
                });
                ui.add_space(5.0);
            }
        }

        ui.separator();
        ui.menu_button("➕ Add Component", |ui| {
            let mut categories: BTreeMap<&'static str, Vec<&ComponentInspector>> = BTreeMap::new();
            for inspector in inspectors {
                if inspector.id == "PrefabRef" || inspector.id == "ModelComponent" {
                    continue;
                }
                if !(inspector.has)(ctx.world, entity_id) {
                    categories.entry(inspector.category).or_default().push(inspector);
                }
            }

            for (cat_name, entries) in categories {
                ui.label(RichText::new(format!("📁 {}", cat_name)).weak());
                for inspector in entries {
                    if ui.button(format!("{} {}", inspector.icon, inspector.display_name)).clicked() {
                        (inspector.add_default)(ctx.world, entity_id);
                        ui.close();
                    }
                }
                ui.separator();
            }

            let mut game_components = Vec::new();
            for entry in serde_registry.iter() {
                if !inspectors.iter().any(|i| i.id == entry.name) 
                    && entry.name != "Static" 
                    && entry.name != "Dynamic" 
                    && !(entry.has)(ctx.world, entity_id.into()) 
                {
                    game_components.push(entry);
                }
            }

            if !game_components.is_empty() {
                ui.label(RichText::new("📁 Gameplay (Auto-Discovered)").weak());
                for entry in game_components {
                    if ui.button(format!("🎮 {}", entry.name)).clicked() {
                        (entry.add_default)(ctx.world, entity_id.into());
                        ui.close();
                    }
                }
            }
        });

        ui.add_space(15.0);
        if ui.button(RichText::new("🗑 Delete Entity (Del)").color(Color32::RED)).clicked() {
            ctx.world.delete_entity(entity_id);
            *ctx.selected_entity = None;
        }
    });
}

fn draw_entity_header(ui: &mut Ui, ctx: &mut EditorContext, entity_id: u64) {
    let name_edit_id = ui.make_persistent_id("inspector_entity_name_edit");
    let mut request_focus_name = false;

    if !ui.ctx().egui_wants_keyboard_input() {
        if ui.input(|i| i.key_pressed(egui::Key::Delete)) {
            ctx.world.delete_entity(entity_id);
            *ctx.selected_entity = None;
            return;
        }
        if ui.input(|i| i.key_pressed(egui::Key::F2)) {
            request_focus_name = true;
        }
    }

    ui.horizontal(|ui| {
        let mut name = ctx.world.get_entity_name(entity_id);
        let name_edit_resp = ui.add(
            egui::TextEdit::singleline(&mut name)
                .id(name_edit_id)
                .hint_text("Entity name (F2)"),
        );

        if request_focus_name {
            name_edit_resp.request_focus();
        }

        if name_edit_resp.changed() {
            ctx.world.set_entity_name(entity_id, &name);
        }

        if ui.button(RichText::new("🗑 Delete").color(Color32::RED)).on_hover_text("Delete Entity (Del)").clicked() {
            ctx.world.delete_entity(entity_id);
            *ctx.selected_entity = None;
            return;
        }

        if ui.button("✖ Deselect").clicked() {
            *ctx.selected_entity = None;
        }
    });
}

fn draw_prefab_inspector(ui: &mut Ui, world: &World, entity_id: u64) -> bool {
    world.get_prefab_ref(entity_id, |p| {
        ui.label(RichText::new(format!("Source: {}", p.path)).weak());
    });
    false
}

fn draw_transform_inspector(ui: &mut Ui, world: &World, entity_id: u64) -> bool {
    let mut changed = false;
    world.get_transform_mut(entity_id, |transform| {
        ui.horizontal(|ui| {
            changed |= ui.add(DragValue::new(&mut transform.position.x).speed(0.1).prefix("X: ")).changed();
            changed |= ui.add(DragValue::new(&mut transform.position.y).speed(0.1).prefix("Y: ")).changed();
            changed |= ui.add(DragValue::new(&mut transform.position.z).speed(0.1).prefix("Z: ")).changed();
        });

        let (rx, ry, rz) = transform.rotation.to_euler(glam::EulerRot::XYZ);
        let mut deg_x = rx.to_degrees();
        let mut deg_y = ry.to_degrees();
        let mut deg_z = rz.to_degrees();

        let mut rot_changed = false;
        ui.horizontal(|ui| {
            rot_changed |= ui.add(DragValue::new(&mut deg_x).speed(0.5).prefix("R_X: ").suffix("°")).changed();
            rot_changed |= ui.add(DragValue::new(&mut deg_y).speed(0.5).prefix("R_Y: ").suffix("°")).changed();
            rot_changed |= ui.add(DragValue::new(&mut deg_z).speed(0.5).prefix("R_Z: ").suffix("°")).changed();
        });

        if rot_changed {
            transform.rotation = glam::Quat::from_euler(
                glam::EulerRot::XYZ,
                deg_x.to_radians(),
                deg_y.to_radians(),
                deg_z.to_radians(),
            );
            changed = true;
        }

        ui.horizontal(|ui| {
            changed |= ui.add(DragValue::new(&mut transform.scale.x).speed(0.1).prefix("S_X: ")).changed();
            changed |= ui.add(DragValue::new(&mut transform.scale.y).speed(0.1).prefix("S_Y: ")).changed();
            changed |= ui.add(DragValue::new(&mut transform.scale.z).speed(0.1).prefix("S_Z: ")).changed();
        });
    });
    changed
}

fn draw_camera_inspector(ui: &mut Ui, world: &World, entity_id: u64) -> bool {
    let mut changed = false;
    world.get_camera_mut(entity_id, |camera| {
        changed |= ui.checkbox(&mut camera.is_active, "Active").changed();
        changed |= ui.add(Slider::new(&mut camera.fovy, 10.0..=120.0).text("FOV")).changed();
        changed |= ui.add(Slider::new(&mut camera.znear, 0.01..=10.0).text("Z Near")).changed();
        changed |= ui.add(Slider::new(&mut camera.zfar, 10.0..=10000.0).text("Z Far")).changed();
        changed |= ui.add(DragValue::new(&mut camera.priority).prefix("Priority: ")).changed();
        changed |= ui.add(DragValue::new(&mut camera.culling_mask).prefix("Culling Mask: ")).changed();
        changed |= ui.checkbox(&mut camera.clear_depth, "Clear Depth (Overlay)").changed();
    });
    changed
}

fn draw_light_inspector(ui: &mut Ui, world: &World, entity_id: u64) -> bool {
    let mut changed = false;
    world.get_light_mut(entity_id, |light| {
        egui::ComboBox::from_label("Type")
            .selected_text(match light.light_type {
                LightType::Directional => "Directional",
                LightType::Point => "Point",
                LightType::Spot => "Spot",
            })
            .show_ui(ui, |ui| {
                changed |= ui.selectable_value(&mut light.light_type, LightType::Point, "Point").changed();
                changed |= ui.selectable_value(&mut light.light_type, LightType::Directional, "Directional").changed();
                changed |= ui.selectable_value(&mut light.light_type, LightType::Spot, "Spot").changed();
            });

        ui.horizontal(|ui| {
            ui.label("Color:");
            let mut color = [light.color[0], light.color[1], light.color[2]];
            if ui.color_edit_button_rgb(&mut color).changed() {
                light.color = color;
                changed = true;
            }
        });

        changed |= ui.add(Slider::new(&mut light.intensity, 0.0..=1000.0).text("Intensity")).changed();

        match light.light_type {
            LightType::Point | LightType::Spot => {
                changed |= ui.add(Slider::new(&mut light.range, 0.1..=200.0).text("Range")).changed();
            }
            _ => {}
        }

        if let LightType::Spot = light.light_type {
            let mut inner_deg = light.inner_angle.to_degrees();
            let mut outer_deg = light.outer_angle.to_degrees();

            let mut angle_changed = false;
            angle_changed |= ui.add(Slider::new(&mut inner_deg, 0.0..=90.0).text("Inner Cone")).changed();
            angle_changed |= ui.add(Slider::new(&mut outer_deg, 0.0..=90.0).text("Outer Cone")).changed();

            if angle_changed {
                light.inner_angle = inner_deg.to_radians();
                light.outer_angle = outer_deg.to_radians().max(light.inner_angle + 0.01);
                changed = true;
            }
        }
    });
    changed
}

fn draw_model_inspector(ui: &mut Ui, world: &World, entity_id: u64) -> bool {
    let mut changed = false;
    world.get_model_mut(entity_id, |model| {
        ui.label(format!("Model ID: {:?}", model.id.path()));
        ui.label(format!("Mesh Index: {}", model.mesh_index));
        changed |= ui.add(DragValue::new(&mut model.render_layer).prefix("Render Layer: ")).changed();
    });
    changed
}

fn draw_rigidbody_inspector(ui: &mut Ui, world: &World, entity_id: u64) -> bool {
    let mut changed = false;
    world.get_rigid_body_mut(entity_id, |rb| {
        egui::ComboBox::from_label("Type")
            .selected_text(match rb.body_type {
                BodyType::Dynamic => "Dynamic",
                BodyType::Fixed => "Fixed (Static)",
                BodyType::KinematicPositionBased => "Kinematic (Position)",
                BodyType::KinematicVelocityBased => "Kinematic (Velocity)",
            })
            .show_ui(ui, |ui| {
                changed |= ui.selectable_value(&mut rb.body_type, BodyType::Dynamic, "Dynamic").changed();
                changed |= ui.selectable_value(&mut rb.body_type, BodyType::Fixed, "Fixed (Static)").changed();
                changed |= ui.selectable_value(&mut rb.body_type, BodyType::KinematicPositionBased, "Kinematic (Position)").changed();
                changed |= ui.selectable_value(&mut rb.body_type, BodyType::KinematicVelocityBased, "Kinematic (Velocity)").changed();
            });

        if rb.body_type == BodyType::Dynamic {
            changed |= ui.add(Slider::new(&mut rb.mass, 0.01..=1000.0).text("Mass (kg)")).changed();
            changed |= ui.add(Slider::new(&mut rb.gravity_scale, -10.0..=10.0).text("Gravity Scale")).changed();
            changed |= ui.add(Slider::new(&mut rb.linear_damping, 0.0..=10.0).text("Linear Damping")).changed();
            changed |= ui.add(Slider::new(&mut rb.angular_damping, 0.0..=10.0).text("Angular Damping")).changed();
            changed |= ui.checkbox(&mut rb.ccd_enabled, "CCD (Continuous Collision)").changed();
            changed |= ui.checkbox(&mut rb.lock_rotations, "Lock Rotations").changed();
        }
    });
    changed
}

fn draw_collider_inspector(ui: &mut Ui, world: &World, entity_id: u64) -> bool {
    let mut changed = false;
    world.get_collider_mut(entity_id, |col| {
        egui::ComboBox::from_label("Shape")
            .selected_text(match col.shape_type {
                ColliderShapeType::Cuboid => "Cuboid (Box)",
                ColliderShapeType::Sphere => "Sphere",
                ColliderShapeType::Capsule => "Capsule",
                ColliderShapeType::Cylinder => "Cylinder",
                ColliderShapeType::TriMesh => "TriMesh (Model)",
                ColliderShapeType::ConvexHull => "Convex Hull (Model)",
            })
            .show_ui(ui, |ui| {
                changed |= ui.selectable_value(&mut col.shape_type, ColliderShapeType::Cuboid, "Cuboid (Box)").changed();
                changed |= ui.selectable_value(&mut col.shape_type, ColliderShapeType::Sphere, "Sphere").changed();
                changed |= ui.selectable_value(&mut col.shape_type, ColliderShapeType::Capsule, "Capsule").changed();
                changed |= ui.selectable_value(&mut col.shape_type, ColliderShapeType::Cylinder, "Cylinder").changed();
                changed |= ui.selectable_value(&mut col.shape_type, ColliderShapeType::TriMesh, "TriMesh (Model)").changed();
                changed |= ui.selectable_value(&mut col.shape_type, ColliderShapeType::ConvexHull, "Convex Hull (Model)").changed();
            });

        match col.shape_type {
            ColliderShapeType::Cuboid => {
                ui.horizontal(|ui| {
                    ui.label("Half Extents:");
                    changed |= ui.add(DragValue::new(&mut col.size[0]).speed(0.05).prefix("X: ")).changed();
                    changed |= ui.add(DragValue::new(&mut col.size[1]).speed(0.05).prefix("Y: ")).changed();
                    changed |= ui.add(DragValue::new(&mut col.size[2]).speed(0.05).prefix("Z: ")).changed();
                });
            }
            ColliderShapeType::Sphere => {
                changed |= ui.add(DragValue::new(&mut col.size[0]).speed(0.05).prefix("Radius: ")).changed();
            }
            ColliderShapeType::Capsule | ColliderShapeType::Cylinder => {
                changed |= ui.add(DragValue::new(&mut col.size[0]).speed(0.05).prefix("Radius: ")).changed();
                changed |= ui.add(DragValue::new(&mut col.size[1]).speed(0.05).prefix("Half Height: ")).changed();
            }
            ColliderShapeType::TriMesh => {
                ui.label(RichText::new("ℹ Uses mesh vertices from MeshComponent (Static)").weak());
            }
            ColliderShapeType::ConvexHull => {
                ui.label(RichText::new("ℹ Computes convex hull from MeshComponent").weak());
            }
        }

        ui.collapsing("📐 Local Offset", |ui| {
            ui.horizontal(|ui| {
                ui.label("Pos:");
                changed |= ui.add(DragValue::new(&mut col.offset_position[0]).speed(0.05).prefix("X: ")).changed();
                changed |= ui.add(DragValue::new(&mut col.offset_position[1]).speed(0.05).prefix("Y: ")).changed();
                changed |= ui.add(DragValue::new(&mut col.offset_position[2]).speed(0.05).prefix("Z: ")).changed();
            });
        });

        ui.collapsing("🏷 Collision Filtering", |ui| {
            changed |= ui.add(DragValue::new(&mut col.collision_memberships).prefix("Memberships: ")).changed();
            changed |= ui.add(DragValue::new(&mut col.collision_filters).prefix("Filters: ")).changed();
            changed |= ui.checkbox(&mut col.generate_collision_events, "Generate Collision Events").changed();
        });

        changed |= ui.add(Slider::new(&mut col.friction, 0.0..=2.0).text("Friction")).changed();
        changed |= ui.add(Slider::new(&mut col.restitution, 0.0..=1.0).text("Restitution (Bounciness)")).changed();
        changed |= ui.checkbox(&mut col.is_sensor, "Is Trigger / Sensor").changed();
    });
    changed
}

fn draw_kcc_inspector(ui: &mut Ui, world: &World, entity_id: u64) -> bool {
    let mut changed = false;
    world.get_character_controller_mut(entity_id, |kcc| {
        changed |= ui.checkbox(&mut kcc.slide, "Slide on Obstacles").changed();

        ui.collapsing("🪜 Autostep (Stairs)", |ui| {
            changed |= ui.checkbox(&mut kcc.enable_autostep, "Enable Autostep").changed();
            if kcc.enable_autostep {
                changed |= ui.add(DragValue::new(&mut kcc.autostep_max_height).speed(0.01).prefix("Max Step Height: ").suffix(" m")).changed();
                changed |= ui.add(DragValue::new(&mut kcc.autostep_min_width).speed(0.01).prefix("Min Step Width: ").suffix(" m")).changed();
            }
        });

        ui.collapsing("📐 Slopes & Angles", |ui| {
            changed |= ui.add(Slider::new(&mut kcc.max_slope_climb_angle, 0.0..=89.0).text("Max Climb Angle (°)")).changed();
            changed |= ui.add(Slider::new(&mut kcc.min_slope_slide_angle, 0.0..=89.0).text("Min Slide Angle (°)")).changed();
        });

        ui.collapsing("🧲 Snap to Ground", |ui| {
            changed |= ui.checkbox(&mut kcc.enable_snap_to_ground, "Enable Snap to Ground").changed();
            if kcc.enable_snap_to_ground {
                changed |= ui.add(DragValue::new(&mut kcc.snap_to_ground_distance).speed(0.01).prefix("Snap Distance: ").suffix(" m")).changed();
            }
        });
    });
    changed
}