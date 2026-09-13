use crate::ui::EditorContext;
use egui::{Color32, RichText, ScrollArea, Sense, Ui, Vec2};

pub fn draw_hierarchy(ui: &mut Ui, ctx: &mut EditorContext) {
    if let Some(ref path) = ctx.asset_browser_state.editing_prefab_path.clone() {
        let file_name = path.file_name().unwrap_or_default().to_string_lossy();
        ui.group(|ui| {
            ui.horizontal(|ui| {
                ui.label(RichText::new(format!("🧩 Editing Prefab: {}", file_name)).strong().color(Color32::GOLD));
            });
            ui.horizontal(|ui| {
                if ui.button("💾 Save").on_hover_text("Save changes to prefab file").clicked() {
                    crate::ui::asset_browser::save_and_close_prefab_editing(ctx, true);
                }
                if ui.button("❌ Close").on_hover_text("Exit without saving").clicked() {
                    crate::ui::asset_browser::save_and_close_prefab_editing(ctx, false);
                }
            });
        });
        ui.separator();
    }

    ui.horizontal(|ui| {
        if ui.button("➕ Empty").clicked() {
            let id = ctx.world.spawn_empty("New Entity");
            *ctx.selected_entity = Some(id.0);
        }
        if let Some(parent_id) = *ctx.selected_entity {
            if ui.button("➕ Child").clicked() {
                let id = ctx.world.spawn_child(parent_id, "Child Entity");
                *ctx.selected_entity = Some(id.0);
            }
        }
    });
    ui.separator();

    ScrollArea::vertical().show(ui, |ui| {
        let roots = ctx.world.get_root_entities();
        if roots.is_empty() {
            ui.label(RichText::new("Scene is empty.").italics());
        } else {
            for root_id in roots {
                let name = ctx.world.get_entity_name(root_id);
                if name.starts_with("__") {
                    continue;
                }

                draw_entity_node(ui, ctx, root_id.0);
            }
        }
    });

    if let Some(dragged_id) = ctx.asset_browser_state.dragged_entity {
        ui.ctx().set_cursor_icon(egui::CursorIcon::Grabbing);
        if let Some(pos) = ui.input(|i| i.pointer.hover_pos().or(i.pointer.interact_pos())) {
            let name = ctx.world.get_entity_name(dragged_id);
            let display_name = if name.is_empty() { format!("Entity {}", dragged_id) } else { name };

            egui::Area::new(ui.id().with("entity_drag_tooltip"))
                .interactable(false)
                .order(egui::Order::Tooltip)
                .fixed_pos(pos + Vec2::new(14.0, 14.0))
                .show(ui.ctx(), |ui| {
                    egui::Frame::popup(ui.style()).show(ui, |ui| {
                        ui.label(RichText::new(format!("🧩 Create Prefab: {}", display_name)).color(Color32::GOLD).strong());
                    });
                });
        }
    }
}

fn draw_label_and_handle_drag(
    ui: &mut Ui,
    ctx: &mut EditorContext,
    entity_id: u64,
    is_selected: bool,
    header_text: &str,
) {
    let bg_fill = if is_selected {
        ui.visuals().selection.bg_fill
    } else {
        Color32::TRANSPARENT
    };
    let text_color = if is_selected {
        Color32::WHITE
    } else {
        ui.visuals().text_color()
    };
    let stroke = if is_selected {
        ui.visuals().selection.stroke
    } else {
        egui::Stroke::NONE
    };

    let resp = ui.add(
        egui::Button::new(RichText::new(header_text).color(text_color))
            .selected(is_selected)
            .fill(bg_fill)
            .stroke(stroke)
            .sense(Sense::click_and_drag())
    );

    if resp.clicked() {
        if is_selected {
            *ctx.selected_entity = None;
        } else {
            *ctx.selected_entity = Some(entity_id);
        }
    }

    if resp.drag_started() {
        ctx.asset_browser_state.dragged_entity = Some(entity_id);
    }

    resp.context_menu(|ui| {
        if ui.button("🧩 Save as Prefab").clicked() {
            crate::ui::asset_browser::save_entity_to_current_folder(ctx, entity_id);
            ui.close();
        }
        if ui.button("➕ Add Child").clicked() {
            let child_id = ctx.world.spawn_child(entity_id, "New Child");
            *ctx.selected_entity = Some(child_id.0);
            ui.close();
        }
        ui.separator();
        if ui.button(RichText::new("🗑 Delete").color(Color32::RED)).clicked() {
            ctx.world.delete_entity(entity_id);
            if *ctx.selected_entity == Some(entity_id) {
                *ctx.selected_entity = None;
            }
            ui.close();
        }
    });
}

fn draw_entity_node(ui: &mut Ui, ctx: &mut EditorContext, entity_id: u64) {
    if !ctx.world.is_alive(entity_id) {
        return;
    }

    let is_selected = *ctx.selected_entity == Some(entity_id);
    let name = ctx.world.get_entity_name(entity_id);
    let display_name = if name.is_empty() {
        format!("Entity {}", entity_id)
    } else {
        name
    };

    let has_light = ctx.world.has_light(entity_id);
    let has_model = ctx.world.has_model(entity_id);
    let has_camera = ctx.world.has_camera(entity_id);

    let icon = if has_light {
        "💡"
    } else if has_model {
        "🔷"
    } else if has_camera {
        "📷"
    } else {
        "📁"
    };

    let header_text = format!("{} {} ({})", icon, display_name, entity_id);
    let header_id = ui.make_persistent_id(entity_id);
    let has_children = ctx.world.has_children(entity_id);

    if has_children {
        egui::collapsing_header::CollapsingState::load_with_default_open(ui.ctx(), header_id, true)
            .show_header(ui, |ui| {
                draw_label_and_handle_drag(ui, ctx, entity_id, is_selected, &header_text);
            })
            .body(|ui| {
                let children = ctx.world.get_children(entity_id);
                for child_id in children {
                    let child_name = ctx.world.get_entity_name(child_id);
                    if !child_name.starts_with("__") {
                        draw_entity_node(ui, ctx, child_id.0);
                    }
                }
            });
    } else {
        draw_label_and_handle_drag(ui, ctx, entity_id, is_selected, &header_text);
    }
}