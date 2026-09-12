use crate::ui::visitor::EguiFieldVisitor;
use crate::ui::EditorContext;
use ecs::{CameraComponent, ColliderComponent, ColliderShapeType, LightComponent, LightType, RigidBodyComponent, CharacterControllerComponent, Transform};
use egui::{Color32, DragValue, RichText, ScrollArea, Slider, Ui};

pub fn draw_inspector(ui: &mut Ui, ctx: &mut EditorContext) {
    if let Some(entity_id) = *ctx.selected_entity {
        if !ctx.world.is_alive(entity_id) {
            *ctx.selected_entity = None;
            return;
        }

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

            if ui
                .button(RichText::new("🗑 Delete").color(Color32::RED))
                .on_hover_text("Delete Entity (Del)")
                .clicked()
            {
                ctx.world.delete_entity(entity_id);
                *ctx.selected_entity = None;
                return;
            }

            if ui.button("✖ Deselect").clicked() {
                *ctx.selected_entity = None;
                return;
            }
        });
        ui.label(format!("Entity ID: {}", entity_id));
        ui.separator();

        ScrollArea::vertical().show(ui, |ui| {
            let has_transform = ctx.world.has_transform(entity_id);
            let has_camera = ctx.world.has_camera(entity_id);
            let has_light = ctx.world.has_light(entity_id);
            let has_model = ctx.world.has_model(entity_id);
            let has_rigid_body = ctx.world.has_rigid_body(entity_id);
            let has_collider = ctx.world.has_collider(entity_id);
            let has_character_controller = ctx.world.has_character_controller(entity_id); 
            let has_prefab_ref = ctx.world.has_prefab_ref(entity_id);

            if has_prefab_ref {
                ui.group(|ui| {
                    ui.horizontal(|ui| {
                        ui.label(RichText::new("🧩 Prefab Instance").strong().color(Color32::from_rgb(255, 215, 0)));
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if ui.button("❌").on_hover_text("Unlink instance from prefab").clicked() {
                                ctx.world.remove_prefab_ref(entity_id);
                            }
                        });
                    });

                    ctx.world.get_prefab_ref(entity_id, |p| {
                        ui.label(RichText::new(format!("Source: {}", p.path)).weak());
                    });
                });
                ui.add_space(5.0);
            }
            
            // transform component
            if has_transform {
                ui.group(|ui| {
                    ui.horizontal(|ui| {
                        ui.label(RichText::new("📍 Transform").strong());
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if ui.button("❌").on_hover_text("Remove Transform").clicked() {
                                ctx.world.remove_transform(entity_id);
                            }
                        });
                    });

                    let mut transform_changed = false;

                    ctx.world.get_transform_mut(entity_id, |transform| {
                        ui.horizontal(|ui| {
                            transform_changed |= ui
                                .add(DragValue::new(&mut transform.position.x).speed(0.1).prefix("X: "))
                                .changed();
                            transform_changed |= ui
                                .add(DragValue::new(&mut transform.position.y).speed(0.1).prefix("Y: "))
                                .changed();
                            transform_changed |= ui
                                .add(DragValue::new(&mut transform.position.z).speed(0.1).prefix("Z: "))
                                .changed();
                        });

                        let (rx, ry, rz) = transform.rotation.to_euler(glam::EulerRot::XYZ);
                        let mut deg_x = rx.to_degrees();
                        let mut deg_y = ry.to_degrees();
                        let mut deg_z = rz.to_degrees();

                        let mut rot_changed = false;
                        ui.horizontal(|ui| {
                            rot_changed |= ui
                                .add(DragValue::new(&mut deg_x).speed(0.5).prefix("R_X: ").suffix("°"))
                                .changed();
                            rot_changed |= ui
                                .add(DragValue::new(&mut deg_y).speed(0.5).prefix("R_Y: ").suffix("°"))
                                .changed();
                            rot_changed |= ui
                                .add(DragValue::new(&mut deg_z).speed(0.5).prefix("R_Z: ").suffix("°"))
                                .changed();
                        });

                        if rot_changed {
                            transform.rotation = glam::Quat::from_euler(
                                glam::EulerRot::XYZ,
                                deg_x.to_radians(),
                                deg_y.to_radians(),
                                deg_z.to_radians(),
                            );
                            transform_changed = true;
                        }

                        ui.horizontal(|ui| {
                            transform_changed |= ui
                                .add(DragValue::new(&mut transform.scale.x).speed(0.1).prefix("S_X: "))
                                .changed();
                            transform_changed |= ui
                                .add(DragValue::new(&mut transform.scale.y).speed(0.1).prefix("S_Y: "))
                                .changed();
                            transform_changed |= ui
                                .add(DragValue::new(&mut transform.scale.z).speed(0.1).prefix("S_Z: "))
                                .changed();
                        });
                    });

                    if transform_changed {
                        ctx.world.update_transforms();
                    }
                });
                ui.add_space(5.0);
            }

            // camera component
            if has_camera {
                ui.group(|ui| {
                    ui.horizontal(|ui| {
                        ui.label(RichText::new("📷 Camera Component").strong());
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if ui.button("❌").on_hover_text("Remove Camera").clicked() {
                                ctx.world.remove_camera(entity_id);
                            }
                        });
                    });

                    ctx.world.get_camera_mut(entity_id, |camera| {
                        ui.checkbox(&mut camera.is_active, "Active");
                        ui.add(Slider::new(&mut camera.fovy, 10.0..=120.0).text("FOV"));
                        ui.add(Slider::new(&mut camera.znear, 0.01..=10.0).text("Z Near"));
                        ui.add(Slider::new(&mut camera.zfar, 10.0..=10000.0).text("Z Far"));
                        ui.add(DragValue::new(&mut camera.priority).prefix("Priority: "));
                        ui.add(DragValue::new(&mut camera.culling_mask).prefix("Culling Mask: "));
                        ui.checkbox(&mut camera.clear_depth, "Clear Depth (Overlay)");
                    });
                });
                ui.add_space(5.0);
            }

            // light component
            if has_light {
                ui.group(|ui| {
                    ui.horizontal(|ui| {
                        ui.label(RichText::new("💡 Light Component").strong());
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if ui.button("❌").on_hover_text("Remove Light").clicked() {
                                ctx.world.remove_light(entity_id);
                            }
                        });
                    });

                    ctx.world.get_light_mut(entity_id, |light| {
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

                        ui.add(Slider::new(&mut light.intensity, 0.0..=1000.0).text("Intensity"));

                        match light.light_type {
                            LightType::Point | LightType::Spot => {
                                ui.add(Slider::new(&mut light.range, 0.1..=200.0).text("Range"));
                            }
                            _ => {}
                        }

                        if let LightType::Spot = light.light_type {
                            let mut inner_deg = light.inner_angle.to_degrees();
                            let mut outer_deg = light.outer_angle.to_degrees();

                            ui.add(Slider::new(&mut inner_deg, 0.0..=90.0).text("Inner Cone"));
                            ui.add(Slider::new(&mut outer_deg, 0.0..=90.0).text("Outer Cone"));

                            light.inner_angle = inner_deg.to_radians();
                            light.outer_angle = outer_deg.to_radians().max(light.inner_angle + 0.01);
                        }
                    });
                });
                ui.add_space(5.0);
            }

            // mesh component
            if has_model {
                ui.group(|ui| {
                    ui.horizontal(|ui| {
                        ui.label(RichText::new("🔷 Mesh Component").strong());
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if ui.button("❌").on_hover_text("Remove Mesh").clicked() {
                                ctx.world.remove_model(entity_id);
                            }
                        });
                    });

                    ctx.world.get_model_mut(entity_id, |model| {
                        ui.label(format!("Model ID: {:?}", model.id.path()));
                        ui.label(format!("Mesh Index: {}", model.mesh_index));
                        ui.add(DragValue::new(&mut model.render_layer).prefix("Render Layer: "));
                    });
                });
                ui.add_space(5.0);
            }

            // rigidbody component
            if has_rigid_body {
                ui.group(|ui| {
                    ui.horizontal(|ui| {
                        ui.label(RichText::new("📦 RigidBody").strong());
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if ui.button("❌").on_hover_text("Remove RigidBody").clicked() {
                                ctx.world.remove_rigid_body(entity_id);
                            }
                        });
                    });

                    ctx.world.get_rigid_body_mut(entity_id, |rb| {
                        egui::ComboBox::from_label("Type")
                            .selected_text(match rb.body_type {
                                ecs::BodyType::Dynamic => "Dynamic",
                                ecs::BodyType::Fixed => "Fixed (Static)",
                                ecs::BodyType::KinematicPositionBased => "Kinematic (Position)",
                                ecs::BodyType::KinematicVelocityBased => "Kinematic (Velocity)",
                            })
                            .show_ui(ui, |ui| {
                                ui.selectable_value(&mut rb.body_type, ecs::BodyType::Dynamic, "Dynamic");
                                ui.selectable_value(&mut rb.body_type, ecs::BodyType::Fixed, "Fixed (Static)");
                                ui.selectable_value(
                                    &mut rb.body_type,
                                    ecs::BodyType::KinematicPositionBased,
                                    "Kinematic (Position)",
                                );
                                ui.selectable_value(
                                    &mut rb.body_type,
                                    ecs::BodyType::KinematicVelocityBased,
                                    "Kinematic (Velocity)",
                                );
                            });

                        if rb.body_type == ecs::BodyType::Dynamic {
                            ui.add(Slider::new(&mut rb.mass, 0.01..=1000.0).text("Mass (kg)"));
                            ui.add(Slider::new(&mut rb.gravity_scale, -10.0..=10.0).text("Gravity Scale"));
                            ui.add(Slider::new(&mut rb.linear_damping, 0.0..=10.0).text("Linear Damping"));
                            ui.add(Slider::new(&mut rb.angular_damping, 0.0..=10.0).text("Angular Damping"));
                            ui.checkbox(&mut rb.ccd_enabled, "CCD (Continuous Collision)");
                            ui.checkbox(&mut rb.lock_rotations, "Lock Rotations");
                        }
                    });
                });
                ui.add_space(5.0);
            }

            // collider component (обновлен под Спринт 2)
            if has_collider {
                ui.group(|ui| {
                    ui.horizontal(|ui| {
                        ui.label(RichText::new("🛡 Collider").strong());
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if ui.button("❌").on_hover_text("Remove Collider").clicked() {
                                ctx.world.remove_collider(entity_id);
                            }
                        });
                    });

                    ctx.world.get_collider_mut(entity_id, |col| {
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
                                ui.selectable_value(&mut col.shape_type, ColliderShapeType::Cuboid, "Cuboid (Box)");
                                ui.selectable_value(&mut col.shape_type, ColliderShapeType::Sphere, "Sphere");
                                ui.selectable_value(&mut col.shape_type, ColliderShapeType::Capsule, "Capsule");
                                ui.selectable_value(&mut col.shape_type, ColliderShapeType::Cylinder, "Cylinder");
                                ui.selectable_value(&mut col.shape_type, ColliderShapeType::TriMesh, "TriMesh (Model)");
                                ui.selectable_value(&mut col.shape_type, ColliderShapeType::ConvexHull, "Convex Hull (Model)");
                            });

                        match col.shape_type {
                            ColliderShapeType::Cuboid => {
                                ui.horizontal(|ui| {
                                    ui.label("Half Extents:");
                                    ui.add(DragValue::new(&mut col.size[0]).speed(0.05).prefix("X: "));
                                    ui.add(DragValue::new(&mut col.size[1]).speed(0.05).prefix("Y: "));
                                    ui.add(DragValue::new(&mut col.size[2]).speed(0.05).prefix("Z: "));
                                });
                            }
                            ColliderShapeType::Sphere => {
                                ui.add(DragValue::new(&mut col.size[0]).speed(0.05).prefix("Radius: "));
                            }
                            ColliderShapeType::Capsule | ColliderShapeType::Cylinder => {
                                ui.add(DragValue::new(&mut col.size[0]).speed(0.05).prefix("Radius: "));
                                ui.add(DragValue::new(&mut col.size[1]).speed(0.05).prefix("Half Height: "));
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
                                ui.add(DragValue::new(&mut col.offset_position[0]).speed(0.05).prefix("X: "));
                                ui.add(DragValue::new(&mut col.offset_position[1]).speed(0.05).prefix("Y: "));
                                ui.add(DragValue::new(&mut col.offset_position[2]).speed(0.05).prefix("Z: "));
                            });
                        });

                        ui.collapsing("🏷 Collision Filtering", |ui| {
                            ui.add(DragValue::new(&mut col.collision_memberships).prefix("Memberships: "));
                            ui.add(DragValue::new(&mut col.collision_filters).prefix("Filters: "));
                            ui.checkbox(&mut col.generate_collision_events, "Generate Collision Events");
                        });

                        ui.add(Slider::new(&mut col.friction, 0.0..=2.0).text("Friction"));
                        ui.add(Slider::new(&mut col.restitution, 0.0..=1.0).text("Restitution (Bounciness)"));
                        ui.checkbox(&mut col.is_sensor, "Is Trigger / Sensor");
                    });
                });
                ui.add_space(5.0);
            }

            // character controller component (KCC)
            if has_character_controller {
                ui.group(|ui| {
                    ui.horizontal(|ui| {
                        ui.label(RichText::new("🏃 Character Controller (KCC)").strong());
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if ui
                                .button("❌")
                                .on_hover_text("Remove Character Controller")
                                .clicked()
                            {
                                ctx.world.remove_character_controller(entity_id);
                            }
                        });
                    });

                    ctx.world.get_character_controller_mut(entity_id, |kcc| {
                        ui.checkbox(&mut kcc.slide, "Slide on Obstacles");

                        ui.collapsing("🪜 Autostep (Stairs)", |ui| {
                            ui.checkbox(&mut kcc.enable_autostep, "Enable Autostep");
                            if kcc.enable_autostep {
                                ui.add(
                                    DragValue::new(&mut kcc.autostep_max_height)
                                        .speed(0.01)
                                        .prefix("Max Step Height: ")
                                        .suffix(" m"),
                                );
                                ui.add(
                                    DragValue::new(&mut kcc.autostep_min_width)
                                        .speed(0.01)
                                        .prefix("Min Step Width: ")
                                        .suffix(" m"),
                                );
                            }
                        });

                        ui.collapsing("📐 Slopes & Angles", |ui| {
                            ui.add(
                                Slider::new(&mut kcc.max_slope_climb_angle, 0.0..=89.0)
                                    .text("Max Climb Angle (°)"),
                            );
                            ui.add(
                                Slider::new(&mut kcc.min_slope_slide_angle, 0.0..=89.0)
                                    .text("Min Slide Angle (°)"),
                            );
                        });

                        ui.collapsing("🧲 Snap to Ground", |ui| {
                            ui.checkbox(&mut kcc.enable_snap_to_ground, "Enable Snap to Ground");
                            if kcc.enable_snap_to_ground {
                                ui.add(
                                    DragValue::new(&mut kcc.snap_to_ground_distance)
                                        .speed(0.01)
                                        .prefix("Snap Distance: ")
                                        .suffix(" m"),
                                );
                            }
                        });
                    });
                });
                ui.add_space(5.0);
            }

            // dynamic components via visitor pattern
            let dynamic_entries = ctx.world.get_component_entries();
            for entry in &dynamic_entries {
                if (entry.has)(ctx.world, entity_id) {
                    ui.group(|ui| {
                        ui.horizontal(|ui| {
                            ui.label(RichText::new(format!("🧩 {}", entry.name)).strong());
                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                if ui
                                    .button("❌")
                                    .on_hover_text(format!("Remove {}", entry.name))
                                    .clicked()
                                {
                                    (entry.remove)(ctx.world, entity_id);
                                }
                            });
                        });

                        let mut visitor = EguiFieldVisitor { ui, changed: false };
                        (entry.inspect)(ctx.world, entity_id, &mut visitor);
                    });
                    ui.add_space(5.0);
                }
            }

            // add component menu
            ui.separator();
            ui.menu_button("➕ Add Component", |ui| {
                if !has_transform && ui.button("📍 Transform").clicked() {
                    ctx.world.add_transform(entity_id, Transform::default());
                    ui.close();
                }
                if !has_camera && ui.button("📷 Camera").clicked() {
                    ctx.world.add_camera(entity_id, CameraComponent::default());
                    ui.close();
                }
                if !has_light && ui.button("💡 Light Component").clicked() {
                    ctx.world.add_light(entity_id, LightComponent::default());
                    ui.close();
                }
                if !has_rigid_body && ui.button("📦 RigidBody").clicked() {
                    ctx.world.add_rigid_body(entity_id, RigidBodyComponent::default());
                    ui.close();
                }
                if !has_collider && ui.button("🛡 Collider").clicked() {
                    ctx.world.add_collider(entity_id, ColliderComponent::default());
                    ui.close();
                }
                if !has_character_controller && ui.button("🏃 Character Controller").clicked() {
                    ctx.world.add_character_controller(entity_id, CharacterControllerComponent::default());
                    ui.close();
                }

                let mut categories: std::collections::BTreeMap<&'static str, Vec<ecs::DynamicComponentEntry>> =
                    std::collections::BTreeMap::new();
                for entry in &dynamic_entries {
                    if !(entry.has)(ctx.world, entity_id) {
                        categories.entry(entry.category).or_default().push(entry.clone());
                    }
                }

                for (cat_name, entries_in_cat) in categories {
                    ui.separator();
                    ui.label(RichText::new(format!("📁 {}", cat_name)).weak());
                    for entry in entries_in_cat {
                        if ui.button(format!("  ➕ {}", entry.name)).clicked() {
                            (entry.add_default)(ctx.world, entity_id);
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
    } else {
        ui.label(RichText::new("Select an entity from Scene Hierarchy to inspect.").italics());
    }
}