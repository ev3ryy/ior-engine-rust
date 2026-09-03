pub mod asset_browser;
pub mod dock;
pub mod hierarchy;
pub mod inspector;
pub mod metrics;
pub mod postprocess;
pub mod toolbar;
pub mod viewport;
pub mod visitor;

use crate::EditorGizmoMode;
use asset_browser::AssetBrowserState;
use dock::{EditorTabViewer, TabKind};
use ecs::World;
use egui::Context;
use egui_dock::{DockArea, DockState, Style};
use engine::camera::Camera;
use engine::game::{PlayControlAction, PlayState};
use engine::EngineMetrics;
use render::postprocess::PostProcessSettings;
use std::sync::Arc;
use transform_gizmo_egui::prelude::*;

// editor execution context passed down to tabs
pub struct EditorContext<'a> {
    pub world: &'a mut World,
    pub camera: &'a mut Camera,
    pub postprocess: &'a mut PostProcessSettings,
    pub metrics: &'a EngineMetrics,
    pub load_requests: &'a mut Vec<String>,
    pub selected_entity: &'a mut Option<u64>,
    pub gizmo: &'a mut Gizmo,
    pub gizmo_mode: &'a mut EditorGizmoMode,
    pub gizmo_orientation: &'a mut GizmoOrientation,
    pub play_state: PlayState,
    pub play_action: &'a mut PlayControlAction,
    pub asset_server: &'a Arc<asset::AssetServer>,
    pub clear_requested: bool,
    pub world_save_path: &'a str,
    pub viewport_texture_id: Option<egui::TextureId>,
    pub desired_viewport_size: &'a mut Option<(u32, u32)>,
    pub asset_browser_state: &'a mut AssetBrowserState,
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
    play_state: PlayState,
    play_action: &mut PlayControlAction,
    asset_server: &Arc<asset::AssetServer>,
    dock_state: &mut DockState<TabKind>,
    world_save_path: &str,
    viewport_texture_id: Option<egui::TextureId>,
    desired_viewport_size: &mut Option<(u32, u32)>,
    asset_browser_state: &mut AssetBrowserState,
) {
    let mut clear_requested = false;

    let mut editor_ctx = EditorContext {
        world,
        camera,
        postprocess,
        metrics,
        load_requests,
        selected_entity,
        gizmo,
        gizmo_mode,
        gizmo_orientation,
        play_state,
        play_action,
        asset_server,
        clear_requested: false,
        world_save_path,
        viewport_texture_id,
        desired_viewport_size,
        asset_browser_state,
    };

    let screen_rect = ctx
        .input(|i| i.raw.screen_rect)
        .unwrap_or_else(|| egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1280.0, 720.0)));

    // docking workspace layout
    let mut tab_viewer = EditorTabViewer {
        ctx: &mut editor_ctx,
    };

    egui::Area::new(egui::Id::new("dock_workspace"))
        .order(egui::Order::Background)
        .fixed_pos(screen_rect.min)
        .show(ctx, |ui| {
            ui.set_min_size(screen_rect.size());
            DockArea::new(dock_state)
                .style(Style::from_egui(ctx.global_style().as_ref()))
                .show_inside(ui, &mut tab_viewer);
        });

    // floating top toolbar
    toolbar::draw_toolbar(ctx, &mut editor_ctx, dock_state);

    clear_requested |= editor_ctx.clear_requested;

    if clear_requested {
        world.clear_scene();
        *selected_entity = None;
        log::info!("Scene cleared.");
    }
}