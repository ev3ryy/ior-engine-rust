use egui::Context;
use ecs::{World, LightType};
use engine::{EngineMetrics, camera::Camera};

pub fn draw_editor_ui(ctx: &Context, world: &mut World, camera: &mut Camera, metrics: &EngineMetrics, load_requests: &mut Vec<String>) {
    let mut clear_requested = false;

    // toolbar
    egui::Window::new("Toolbar")
        .default_pos([10.0, 10.0]) 
        .title_bar(false) 
        .resizable(false)
        .show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.menu_button("File", |ui| {
                    if ui.button("Save Scene (F5)").clicked() {
                        // TODO: saving scene
                    }
                    if ui.button("Load Scene (F9)").clicked() {
                        // TODO: loading scene
                    }
                    if ui.button("Clear Scene").clicked() {
                        clear_requested = true;
                    }
                    ui.separator();
                    if ui.button("Quit").clicked() {
                        std::process::exit(0);
                    }
                });
                
                ui.menu_button("View", |ui| {
                    ui.checkbox(&mut true, "Show Inspector"); 
                    ui.checkbox(&mut true, "Show Metrics");
                });
            });
        });

    // inspector
    egui::Window::new("Inspector")
        .default_pos([10.0, 50.0])
        .default_size([300.0, 500.0])
        .show(ctx, |ui| {
            egui::ScrollArea::vertical().show(ui, |ui| {
                let mut entity_count = 0;
                world.each_transform_mut(|entity_id, pos, _rot, scale| {
                    
                    let has_model = world.has_model(entity_id);
                    let has_light = world.has_light(entity_id);
                    
                    if !has_model && !has_light { return; }

                    let header_text = format!("Entity {}", entity_id);
                    
                    egui::CollapsingHeader::new(header_text)
                        .default_open(true)
                        .show(ui, |ui| {
                            
                            ui.label(egui::RichText::new("Transform").strong());
                            ui.horizontal(|ui| {
                                ui.add(egui::DragValue::new(&mut pos.0.x).speed(0.1).prefix("X: "));
                                ui.add(egui::DragValue::new(&mut pos.0.y).speed(0.1).prefix("Y: "));
                                ui.add(egui::DragValue::new(&mut pos.0.z).speed(0.1).prefix("Z: "));
                            });
                            
                            ui.horizontal(|ui| {
                                ui.add(egui::DragValue::new(&mut scale.0.x).speed(0.1).prefix("S_X: "));
                                ui.add(egui::DragValue::new(&mut scale.0.y).speed(0.1).prefix("S_Y: "));
                                ui.add(egui::DragValue::new(&mut scale.0.z).speed(0.1).prefix("S_Z: "));
                            });

                            if has_model {
                                world.get_model(entity_id, |model| {
                                    ui.separator();
                                    ui.label(egui::RichText::new("Mesh Component").strong());
                                    ui.horizontal(|ui| {
                                        ui.label("Model ID:");
                                        ui.label(format!("{:?}", model.id.path()));
                                    });
                                    ui.label(format!("Mesh Index: {}", model.mesh_index));
                                });
                            }

                            if has_light {
                                world.get_light_mut(entity_id, |light| {
                                    ui.separator();
                                    ui.label(egui::RichText::new("Light Settings").strong());
                                    
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
                                        },
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
                            }
                        });
                    
                    ui.add_space(5.0);
                    entity_count += 1;
                });
                
                if entity_count == 0 {
                    ui.label(egui::RichText::new("No renderable entities in the world.").italics());
                }
            });
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
                                                
                                                ecs::spawn_scene(world, model_id, &baked_model, glam::Vec3::ZERO);
                                            
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

        log::info!("Scene cleared");
    }
}