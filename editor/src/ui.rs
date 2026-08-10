use egui::Context;
use ecs::{World, Position, Rotation, Scale, ModelComponent};
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
                let mut entity_id = 0;
                
                world.inner.each::<(&mut Position, &mut Rotation, &mut Scale, &ModelComponent)>(
                    |(pos, _rot, scale, model)| {
                        let header_text = format!("Entity {} (Mesh {})", entity_id, model.mesh_index);
                        
                        egui::CollapsingHeader::new(header_text)
                            .default_open(true)
                            .show(ui, |ui| {
                                // Transform (Position)
                                ui.label(egui::RichText::new("Position").strong());
                                ui.horizontal(|ui| {
                                    ui.add(egui::DragValue::new(&mut pos.0.x).speed(0.1).prefix("X: "));
                                    ui.add(egui::DragValue::new(&mut pos.0.y).speed(0.1).prefix("Y: "));
                                    ui.add(egui::DragValue::new(&mut pos.0.z).speed(0.1).prefix("Z: "));
                                });
                                
                                // Transform (Scale)
                                ui.label(egui::RichText::new("Scale").strong());
                                ui.horizontal(|ui| {
                                    ui.add(egui::DragValue::new(&mut scale.0.x).speed(0.1).prefix("X: "));
                                    ui.add(egui::DragValue::new(&mut scale.0.y).speed(0.1).prefix("Y: "));
                                    ui.add(egui::DragValue::new(&mut scale.0.z).speed(0.1).prefix("Z: "));
                                });
                                
                                // Asset ID
                                ui.horizontal(|ui| {
                                    ui.label(egui::RichText::new("Model ID:").strong());
                                    ui.label(format!("{:?}", model.id.path()));
                                });
                            });
                        
                        ui.add_space(5.0);
                        entity_id += 1;
                    }
                );
                
                if entity_id == 0 {
                    ui.label(egui::RichText::new("No entities in the world.").italics());
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