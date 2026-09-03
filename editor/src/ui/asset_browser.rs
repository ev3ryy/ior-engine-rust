use crate::ui::EditorContext;
use egui::{Color32, RichText, ScrollArea, Ui, Vec2};
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AssetCategoryFilter {
    All,
    Models,
    Textures,
    Scenes,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ViewMode {
    Grid,
    List,
}

#[derive(Debug, Clone)]
pub struct AssetModelInfo {
    pub mesh_count: usize,
    pub material_count: usize,
    pub vertex_count: usize,
    pub index_count: usize,
    pub textures: Vec<String>,
}

pub struct AssetBrowserState {
    pub root_dir: PathBuf,
    pub current_dir: PathBuf,
    pub selected_file: Option<PathBuf>,
    pub search_query: String,
    pub category_filter: AssetCategoryFilter,
    pub view_mode: ViewMode,
    pub icon_size: f32,
    pub status_message: Option<(String, bool)>,
    pub dragged_asset: Option<PathBuf>,
    pub cached_model_info: Option<AssetModelInfo>,
}

impl Default for AssetBrowserState {
    fn default() -> Self {
        let root = asset::resolve_asset_root();
        Self {
            root_dir: root.clone(),
            current_dir: root,
            selected_file: None,
            search_query: String::new(),
            category_filter: AssetCategoryFilter::All,
            view_mode: ViewMode::Grid,
            icon_size: 72.0,
            status_message: None,
            dragged_asset: None,
            cached_model_info: None,
        }
    }
}

pub fn draw_asset_browser(ui: &mut Ui, ctx: &mut EditorContext) {
    if !ctx.asset_browser_state.root_dir.exists() {
        ctx.asset_browser_state.root_dir = asset::resolve_asset_root();
        ctx.asset_browser_state.current_dir = ctx.asset_browser_state.root_dir.clone();
    }

    // top navigation and search toolbar
    draw_top_bar(ui, ctx);

    ui.separator();

    let show_preview = ctx.asset_browser_state.selected_file.is_some();
    let root_copy = ctx.asset_browser_state.root_dir.clone();

    // main layout with folder tree and content area
    ui.horizontal(|ui| {
        // left folder tree navigation
        ui.allocate_ui_with_layout(
            Vec2::new(180.0, ui.available_height()),
            egui::Layout::top_down(egui::Align::LEFT),
            |ui| {
                ui.heading(RichText::new("📁 Folders").size(14.0));
                ui.separator();
                ScrollArea::vertical().show(ui, |ui| {
                    draw_folder_node(ui, &root_copy, &mut ctx.asset_browser_state.current_dir);
                });
            },
        );

        ui.separator();

        // center file grid or list
        let available_w = ui.available_width() - if show_preview { 220.0 } else { 0.0 };
        ui.allocate_ui_with_layout(
            Vec2::new(available_w.max(200.0), ui.available_height()),
            egui::Layout::top_down(egui::Align::LEFT),
            |ui| {
                draw_file_view(ui, ctx);
            },
        );

        // right asset preview drawer
        if show_preview {
            ui.separator();
            ui.allocate_ui_with_layout(
                Vec2::new(220.0, ui.available_height()),
                egui::Layout::top_down(egui::Align::LEFT),
                |ui| {
                    draw_asset_preview_drawer(ui, ctx);
                },
            );
        }
    });
}

fn draw_top_bar(ui: &mut Ui, ctx: &mut EditorContext) {
    let state = &mut ctx.asset_browser_state;

    ui.horizontal(|ui| {
        // back / up directory button
        let can_go_up = state.current_dir != state.root_dir && state.current_dir.starts_with(&state.root_dir);
        if ui.add_enabled(can_go_up, egui::Button::new("⬆ Up")).clicked() {
            if let Some(parent) = state.current_dir.parent() {
                state.current_dir = parent.to_path_buf();
                state.selected_file = None;
                state.cached_model_info = None;
            }
        }

        if ui.button("↻ Refresh").clicked() {
            state.selected_file = None;
            state.cached_model_info = None;
        }

        ui.separator();

        // breadcrumbs path
        let rel_path = state
            .current_dir
            .strip_prefix(&state.root_dir)
            .unwrap_or(&state.current_dir);
        let path_str = if rel_path.as_os_str().is_empty() {
            "res/".to_string()
        } else {
            format!("res/{}", rel_path.to_string_lossy().replace('\\', "/"))
        };
        ui.label(RichText::new(path_str).strong().color(Color32::LIGHT_BLUE));

        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            // view mode switch
            ui.selectable_value(&mut state.view_mode, ViewMode::List, "☰ List");
            ui.selectable_value(&mut state.view_mode, ViewMode::Grid, "🗂 Grid");

            ui.separator();

            // search field
            ui.add(egui::TextEdit::singleline(&mut state.search_query).hint_text("🔍 Search..."));

            ui.separator();

            // category filter
            egui::ComboBox::from_id_salt("asset_category_filter")
                .selected_text(match state.category_filter {
                    AssetCategoryFilter::All => "All Assets",
                    AssetCategoryFilter::Models => "📐 Models (.bin, .glb)",
                    AssetCategoryFilter::Textures => "🎨 Textures (.dds, .png)",
                    AssetCategoryFilter::Scenes => "🎬 Scenes (.ron)",
                })
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut state.category_filter, AssetCategoryFilter::All, "All Assets");
                    ui.selectable_value(&mut state.category_filter, AssetCategoryFilter::Models, "📐 Models (.bin, .glb)");
                    ui.selectable_value(&mut state.category_filter, AssetCategoryFilter::Textures, "🎨 Textures (.dds, .png)");
                    ui.selectable_value(&mut state.category_filter, AssetCategoryFilter::Scenes, "🎬 Scenes (.ron)");
                });
        });
    });
}

fn draw_folder_node(ui: &mut Ui, dir: &Path, current_dir: &mut PathBuf) {
    let name = dir
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| "res".to_string());

    let is_active = dir == current_dir;
    let mut has_subdirs = false;

    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries.flatten() {
            if entry.path().is_dir() {
                has_subdirs = true;
                break;
            }
        }
    }

    if has_subdirs {
        let id = ui.make_persistent_id(dir);
        egui::collapsing_header::CollapsingState::load_with_default_open(ui.ctx(), id, false)
            .show_header(ui, |ui| {
                if ui.selectable_label(is_active, format!("📁 {}", name)).clicked() {
                    *current_dir = dir.to_path_buf();
                }
            })
            .body(|ui| {
                if let Ok(entries) = fs::read_dir(dir) {
                    let mut subdirs = Vec::new();
                    for entry in entries.flatten() {
                        let path = entry.path();
                        if path.is_dir() {
                            subdirs.push(path);
                        }
                    }
                    subdirs.sort();
                    for sub in subdirs {
                        draw_folder_node(ui, &sub, current_dir);
                    }
                }
            });
    } else if ui.selectable_label(is_active, format!("📁 {}", name)).clicked() {
        *current_dir = dir.to_path_buf();
    }
}

fn draw_file_view(ui: &mut Ui, ctx: &mut EditorContext) {
    let current_dir = ctx.asset_browser_state.current_dir.clone();
    let entries = match fs::read_dir(&current_dir) {
        Ok(e) => e,
        Err(_) => {
            ui.label(RichText::new("Failed to read directory").color(Color32::RED));
            return;
        }
    };

    let search_lower = ctx.asset_browser_state.search_query.to_lowercase();
    let filter = ctx.asset_browser_state.category_filter;
    let view_mode = ctx.asset_browser_state.view_mode;
    let icon_size = ctx.asset_browser_state.icon_size;

    let mut folders = Vec::new();
    let mut files = Vec::new();

    for entry in entries.flatten() {
        let path = entry.path();
        let name = path.file_name().unwrap_or_default().to_string_lossy().to_string();

        if !search_lower.is_empty() && !name.to_lowercase().contains(&search_lower) {
            continue;
        }

        if path.is_dir() {
            folders.push(path);
        } else {
            let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("").to_lowercase();
            let matches_category = match filter {
                AssetCategoryFilter::All => true,
                AssetCategoryFilter::Models => matches!(ext.as_str(), "bin" | "glb" | "gltf"),
                AssetCategoryFilter::Textures => matches!(ext.as_str(), "dds" | "png" | "jpg" | "jpeg" | "tga"),
                AssetCategoryFilter::Scenes => ext == "ron",
            };

            if matches_category {
                files.push(path);
            }
        }
    }

    folders.sort();
    files.sort();

    ScrollArea::vertical().show(ui, |ui| {
        if view_mode == ViewMode::Grid {
            let item_width = icon_size + 24.0;
            let available_w = ui.available_width();
            let cols = ((available_w / item_width).floor() as usize).max(1);

            egui::Grid::new("asset_grid_view")
                .min_col_width(item_width)
                .spacing([12.0, 12.0])
                .show(ui, |ui| {
                    let mut col_idx = 0;

                    // render folder items
                    for folder in &folders {
                        draw_folder_card(ui, folder, ctx);
                        col_idx += 1;
                        if col_idx >= cols {
                            ui.end_row();
                            col_idx = 0;
                        }
                    }

                    // render file items
                    for file in &files {
                        draw_file_card(ui, file, ctx);
                        col_idx += 1;
                        if col_idx >= cols {
                            ui.end_row();
                            col_idx = 0;
                        }
                    }
                });
        } else {
            // list view
            for folder in &folders {
                draw_folder_list_item(ui, folder, ctx);
            }
            for file in &files {
                draw_file_list_item(ui, file, ctx);
            }
        }
    });
}

fn draw_folder_card(ui: &mut Ui, path: &Path, ctx: &mut EditorContext) {
    let name = path.file_name().unwrap_or_default().to_string_lossy().to_string();
    let size = ctx.asset_browser_state.icon_size;

    ui.vertical_centered(|ui| {
        let (rect, resp) = ui.allocate_exact_size(Vec2::new(size + 16.0, size + 24.0), egui::Sense::click());

        let is_hovered = resp.hovered();
        let bg_color = if is_hovered {
            Color32::from_rgb(50, 55, 65)
        } else {
            Color32::from_rgb(35, 38, 45)
        };

        ui.painter().rect_filled(rect, 4.0, bg_color);
        ui.painter().text(
            rect.center_top() + Vec2::new(0.0, size * 0.4),
            egui::Align2::CENTER_CENTER,
            "📁",
            egui::FontId::proportional(size * 0.45),
            Color32::GOLD,
        );

        let truncated = truncate_string(&name, 12);
        ui.painter().text(
            rect.center_bottom() - Vec2::new(0.0, 6.0),
            egui::Align2::CENTER_BOTTOM,
            truncated,
            egui::FontId::proportional(11.0),
            Color32::WHITE,
        );

        if resp.double_clicked() {
            ctx.asset_browser_state.current_dir = path.to_path_buf();
            ctx.asset_browser_state.selected_file = None;
            ctx.asset_browser_state.cached_model_info = None;
        }

        resp.context_menu(|ui| {
            if ui.button("Open Folder").clicked() {
                ctx.asset_browser_state.current_dir = path.to_path_buf();
                ui.close();
            }
            if ui.button("Show in Explorer").clicked() {
                reveal_in_explorer(path);
                ui.close();
            }
        });
    });
}

fn draw_file_card(ui: &mut Ui, path: &Path, ctx: &mut EditorContext) {
    let name = path.file_name().unwrap_or_default().to_string_lossy().to_string();
    let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("").to_lowercase();
    let size = ctx.asset_browser_state.icon_size;

    let is_selected = ctx.asset_browser_state.selected_file.as_deref() == Some(path);

    let (icon, icon_color) = match ext.as_str() {
        "bin" => ("📐", Color32::from_rgb(100, 200, 255)),
        "glb" | "gltf" => ("📦", Color32::from_rgb(255, 180, 80)),
        "dds" | "png" | "jpg" => ("🎨", Color32::from_rgb(220, 100, 255)),
        "ron" => ("🎬", Color32::from_rgb(100, 255, 150)),
        _ => ("📄", Color32::LIGHT_GRAY),
    };

    ui.vertical_centered(|ui| {
        let (rect, resp) = ui.allocate_exact_size(Vec2::new(size + 16.0, size + 24.0), egui::Sense::click_and_drag());

        let bg_color = if is_selected {
            Color32::from_rgb(45, 80, 120)
        } else if resp.hovered() {
            Color32::from_rgb(45, 48, 56)
        } else {
            Color32::from_rgb(32, 34, 40)
        };

        ui.painter().rect_filled(rect, 4.0, bg_color);
        ui.painter().text(
            rect.center_top() + Vec2::new(0.0, size * 0.4),
            egui::Align2::CENTER_CENTER,
            icon,
            egui::FontId::proportional(size * 0.45),
            icon_color,
        );

        let truncated = truncate_string(&name, 12);
        ui.painter().text(
            rect.center_bottom() - Vec2::new(0.0, 6.0),
            egui::Align2::CENTER_BOTTOM,
            truncated,
            egui::FontId::proportional(11.0),
            Color32::WHITE,
        );

        if resp.clicked() {
            ctx.asset_browser_state.selected_file = Some(path.to_path_buf());
            if ext == "bin" {
                ctx.asset_browser_state.cached_model_info = read_model_info(path);
            } else {
                ctx.asset_browser_state.cached_model_info = None;
            }
        }

        // initiate drag & drop for .bin models
        if resp.drag_started() && ext == "bin" {
            ctx.asset_browser_state.dragged_asset = Some(path.to_path_buf());
        }

        // double click spawn
        if resp.double_clicked() && ext == "bin" {
            spawn_model_in_scene(path, ctx);
        }

        resp.context_menu(|ui| {
            if ext == "bin" {
                if ui.button("➕ Spawn in Scene").clicked() {
                    spawn_model_in_scene(path, ctx);
                    ui.close();
                }
            }

            if ext == "glb" || ext == "gltf" {
                if ui.button("🔥 Cook to .bin").clicked() {
                    cook_asset(path);
                    ui.close();
                }
            }

            if ui.button("📂 Show in Explorer").clicked() {
                reveal_in_explorer(path);
                ui.close();
            }

            ui.separator();
            if ui.button(RichText::new("🗑 Delete").color(Color32::RED)).clicked() {
                let _ = fs::remove_file(path);
                ctx.asset_browser_state.selected_file = None;
                ctx.asset_browser_state.cached_model_info = None;
                ui.close();
            }
        });
    });
}

fn draw_folder_list_item(ui: &mut Ui, path: &Path, ctx: &mut EditorContext) {
    let name = path.file_name().unwrap_or_default().to_string_lossy().to_string();
    ui.horizontal(|ui| {
        if ui.button(format!("📁 {}", name)).clicked() {
            ctx.asset_browser_state.current_dir = path.to_path_buf();
        }
    });
}

fn draw_file_list_item(ui: &mut Ui, path: &Path, ctx: &mut EditorContext) {
    let name = path.file_name().unwrap_or_default().to_string_lossy().to_string();
    let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("").to_lowercase();
    let is_selected = ctx.asset_browser_state.selected_file.as_deref() == Some(path);

    let icon = match ext.as_str() {
        "bin" => "📐",
        "glb" | "gltf" => "📦",
        "dds" | "png" | "jpg" => "🎨",
        "ron" => "🎬",
        _ => "📄",
    };

    ui.horizontal(|ui| {
        let resp = ui.selectable_label(is_selected, format!("{} {}", icon, name));
        if resp.clicked() {
            ctx.asset_browser_state.selected_file = Some(path.to_path_buf());
            if ext == "bin" {
                ctx.asset_browser_state.cached_model_info = read_model_info(path);
            } else {
                ctx.asset_browser_state.cached_model_info = None;
            }
        }
        if resp.drag_started() && ext == "bin" {
            ctx.asset_browser_state.dragged_asset = Some(path.to_path_buf());
        }
        if resp.double_clicked() && ext == "bin" {
            spawn_model_in_scene(path, ctx);
        }
    });
}

fn draw_asset_preview_drawer(ui: &mut Ui, ctx: &mut EditorContext) {
    let selected_path = match ctx.asset_browser_state.selected_file.clone() {
        Some(p) => p,
        None => return,
    };

    let name = selected_path.file_name().unwrap_or_default().to_string_lossy().to_string();
    let ext = selected_path.extension().and_then(|e| e.to_str()).unwrap_or("").to_lowercase();

    ui.heading(RichText::new("Inspector").size(14.0));
    ui.separator();

    ui.label(RichText::new(&name).strong());
    if let Ok(meta) = fs::metadata(&selected_path) {
        let size_kb = meta.len() as f32 / 1024.0;
        ui.label(format!("Size: {:.1} KB", size_kb));
    }

    ui.add_space(8.0);

    if ext == "bin" {
        if ui.button("➕ Spawn in Scene").clicked() {
            spawn_model_in_scene(&selected_path, ctx);
        }

        if let Some(ref info) = ctx.asset_browser_state.cached_model_info {
            ui.separator();
            ui.label(RichText::new("Mesh Metadata").strong().color(Color32::LIGHT_BLUE));
            ui.label(format!("Meshes: {}", info.mesh_count));
            ui.label(format!("Materials: {}", info.material_count));
            ui.label(format!("Vertices: {}", info.vertex_count));
            ui.label(format!("Triangles: {}", info.index_count / 3));

            if !info.textures.is_empty() {
                ui.add_space(6.0);
                ui.label(RichText::new("Textures:").strong());
                for tex in &info.textures {
                    ui.label(format!(" • {}", tex));
                }
            }
        }
    } else if ext == "glb" || ext == "gltf" {
        if ui.button("🔥 Cook to .bin").clicked() {
            cook_asset(&selected_path);
        }
    }

    ui.add_space(8.0);
    if ui.button("📂 Open in Explorer").clicked() {
        reveal_in_explorer(&selected_path);
    }
}

fn spawn_model_in_scene(path: &Path, ctx: &mut EditorContext) {
    if let Ok(data) = fs::read(path) {
        if let Ok(baked_model) = rkyv::from_bytes::<shared::BakedModel, rkyv::rancor::Error>(&data) {
            if let Some(name_str) = path.file_name().and_then(|n| n.to_str()) {
                let model_id = asset::AssetId::from_path(name_str);
                let pos = ctx.camera.target;
                let spawned_id = ecs::spawn_scene(ctx.world, model_id, &baked_model, pos);
                *ctx.selected_entity = Some(spawned_id);
                ctx.load_requests.push(name_str.to_string());
                log::info!("Spawned {} in ECS", name_str);
            }
        }
    }
}

fn read_model_info(path: &Path) -> Option<AssetModelInfo> {
    let data = fs::read(path).ok()?;
    let baked = rkyv::from_bytes::<shared::BakedModel, rkyv::rancor::Error>(&data).ok()?;

    let mut total_verts = 0;
    let mut total_indices = 0;
    for mesh in &baked.meshes {
        total_verts += mesh.vertices.len();
        total_indices += mesh.indices.len();
    }

    let mut textures = Vec::new();
    for mat in &baked.materials {
        if let Some(ref d) = mat.diffuse_texture { textures.push(d.clone()); }
        if let Some(ref n) = mat.normal_texture { textures.push(n.clone()); }
        if let Some(ref o) = mat.orm_texture { textures.push(o.clone()); }
    }
    textures.dedup();

    Some(AssetModelInfo {
        mesh_count: baked.meshes.len(),
        material_count: baked.materials.len(),
        vertex_count: total_verts,
        index_count: total_indices,
        textures,
    })
}

fn cook_asset(glb_path: &Path) {
    let Some(cooker_bin) = asset::find_cooker_executable() else {
        log::error!("Cooker executable not found!");
        return;
    };

    let bin_path = glb_path.with_extension("bin");
    let glb_owned = glb_path.to_path_buf();
    let bin_owned = bin_path.clone();

    std::thread::spawn(move || {
        log::info!("Cooking {:?} -> {:?}", glb_owned, bin_owned);
        let status = std::process::Command::new(cooker_bin)
            .arg(&glb_owned)
            .arg(&bin_owned)
            .status();

        match status {
            Ok(s) if s.success() => log::info!("Successfully cooked {:?}", bin_owned),
            Ok(s) => log::error!("Cooker exited with error code: {:?}", s.code()),
            Err(e) => log::error!("Failed to launch cooker: {:?}", e),
        }
    });
}

fn truncate_string(s: &str, max_len: usize) -> String {
    if s.len() > max_len {
        format!("{}...", &s[..max_len - 3])
    } else {
        s.to_string()
    }
}

pub fn reveal_in_explorer(path: &Path) {
    #[cfg(target_os = "windows")]
    {
        let _ = std::process::Command::new("explorer")
            .arg("/select,")
            .arg(path)
            .spawn();
    }
    #[cfg(target_os = "macos")]
    {
        let _ = std::process::Command::new("open")
            .arg("-R")
            .arg(path)
            .spawn();
    }
    #[cfg(target_os = "linux")]
    {
        if let Some(parent) = path.parent() {
            let _ = std::process::Command::new("xdg-open")
                .arg(parent)
                .spawn();
        }
    }
}