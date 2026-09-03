use crate::ui::EditorContext;
use ecs::World;
use egui::{RichText, ScrollArea, Ui};

pub fn draw_hierarchy(ui: &mut Ui, ctx: &mut EditorContext) {
    ui.horizontal(|ui| {
        if ui.button("➕ Empty").clicked() {
            let id = ctx.world.spawn_empty("New Entity");
            *ctx.selected_entity = Some(id);
        }
        if let Some(parent_id) = *ctx.selected_entity {
            if ui.button("➕ Child").clicked() {
                let id = ctx.world.spawn_child(parent_id, "Child Entity");
                *ctx.selected_entity = Some(id);
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
                draw_entity_node(ui, ctx.world, root_id, ctx.selected_entity);
            }
        }
    });
}

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
    let has_camera = world.has_camera(entity_id);

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
    } else if ui.selectable_label(is_selected, &header_text).clicked() {
        if is_selected {
            *selected_entity = None;
        } else {
            *selected_entity = Some(entity_id);
        }
    }
}