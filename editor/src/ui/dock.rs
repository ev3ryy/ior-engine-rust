use crate::ui::asset_browser;
use crate::ui::hierarchy;
use crate::ui::inspector;
use crate::ui::metrics;
use crate::ui::postprocess;
use crate::ui::viewport;
use crate::ui::EditorContext;
use egui::WidgetText;
use egui_dock::{DockState, NodeIndex, TabViewer};
use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum TabKind {
    Viewport,
    SceneHierarchy,
    Inspector,
    AssetBrowser,
    PostProcess,
    Performance,
}

impl TabKind {
    pub const LAYOUT_PATH: &'static str = "editor_layout.ron";

    // default docking layout
    pub fn default_dock_state() -> DockState<TabKind> {
        let mut dock_state = DockState::new(vec![TabKind::Viewport]);

        let [viewport_node, _right_node] = dock_state.main_surface_mut().split_right(
            NodeIndex::root(),
            0.75,
            vec![TabKind::Inspector, TabKind::PostProcess],
        );

        let [_left_node, viewport_node] = dock_state.main_surface_mut().split_left(
            viewport_node,
            0.20,
            vec![TabKind::SceneHierarchy],
        );

        let [_viewport_node, _bottom_node] = dock_state.main_surface_mut().split_below(
            viewport_node,
            0.70,
            vec![TabKind::AssetBrowser, TabKind::Performance],
        );

        dock_state
    }

    // load saved docking layout or fallback to default
    pub fn load_dock_state() -> DockState<TabKind> {
        if Path::new(Self::LAYOUT_PATH).exists() {
            if let Ok(content) = std::fs::read_to_string(Self::LAYOUT_PATH) {
                if let Ok(dock_state) = ron::from_str::<DockState<TabKind>>(&content) {
                    log::info!("Loaded editor layout from {}", Self::LAYOUT_PATH);
                    return dock_state;
                }
                log::warn!(
                    "Failed to parse layout from {}, resetting to defaults",
                    Self::LAYOUT_PATH
                );
            }
        }
        Self::default_dock_state()
    }

    // save current layout to file
    pub fn save_dock_state(dock_state: &DockState<TabKind>) {
        if let Ok(content) =
            ron::ser::to_string_pretty(dock_state, ron::ser::PrettyConfig::default())
        {
            if let Err(e) = std::fs::write(Self::LAYOUT_PATH, content) {
                log::error!("Failed to save layout to {}: {}", Self::LAYOUT_PATH, e);
            } else {
                log::info!("Editor layout saved to {}", Self::LAYOUT_PATH);
            }
        }
    }
}

pub struct EditorTabViewer<'a, 'b> {
    pub ctx: &'a mut EditorContext<'b>,
}

impl<'a, 'b> TabViewer for EditorTabViewer<'a, 'b> {
    type Tab = TabKind;

    fn id(&mut self, tab: &mut Self::Tab) -> egui::Id {
        egui::Id::new(*tab)
    }

    fn title(&mut self, tab: &mut Self::Tab) -> WidgetText {
        match tab {
            TabKind::Viewport => "🎥 Viewport".into(),
            TabKind::SceneHierarchy => "🗂 Scene Hierarchy".into(),
            TabKind::Inspector => "🔍 Inspector".into(),
            TabKind::AssetBrowser => "📦 Asset Browser".into(),
            TabKind::PostProcess => "✨ Post-Processing".into(),
            TabKind::Performance => "📊 Performance & Camera".into(),
        }
    }

    fn ui(&mut self, ui: &mut egui::Ui, tab: &mut Self::Tab) {
        match tab {
            TabKind::Viewport => {
                viewport::draw_viewport(ui, self.ctx);
            }
            TabKind::SceneHierarchy => {
                hierarchy::draw_hierarchy(ui, self.ctx);
            }
            TabKind::Inspector => {
                inspector::draw_inspector(ui, self.ctx);
            }
            TabKind::AssetBrowser => {
                asset_browser::draw_asset_browser(ui, self.ctx);
            }
            TabKind::PostProcess => {
                postprocess::draw_postprocess(ui, self.ctx);
            }
            TabKind::Performance => {
                metrics::draw_metrics(ui, self.ctx);
            }
        }
    }

    fn clear_background(&self, tab: &Self::Tab) -> bool {
        !matches!(tab, TabKind::Viewport)
    }

    fn is_closeable(&self, tab: &Self::Tab) -> bool {
        !matches!(tab, TabKind::Viewport)
    }
}