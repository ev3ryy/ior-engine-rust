use crate::ui::EditorContext;
use egui::{Color32, RichText, ScrollArea, Sense, Ui, Vec2};
use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AssetCategoryFilter {
    All,
    Models,
    Textures,
    Scenes,
    Prefabs,
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

#[derive(Debug, Clone)]
pub struct CachedDirEntry {
    pub path: PathBuf,
    pub name: String,
    pub is_dir: bool,
    pub ext: String,
}

#[derive(Debug, Clone)]
pub struct FolderTree {
    pub path: PathBuf,
    pub name: String,
    pub subdirs: Vec<FolderTree>,
}

pub struct AssetBrowserState {
    pub root_dir: PathBuf,
    pub current_dir: PathBuf,
    pub folder_panel_width: f32,
    pub selected_file: Option<PathBuf>,
    pub search_query: String,
    pub category_filter: AssetCategoryFilter,
    pub view_mode: ViewMode,
    pub icon_size: f32,
    pub status_message: Option<(String, bool)>,
    pub dragged_asset: Option<PathBuf>,
    pub dragged_entity: Option<u64>,

    pub editing_prefab_root: Option<u64>,
    pub editing_prefab_path: Option<PathBuf>,
    pub prefab_scene_backup: Option<Vec<u8>>,
    
    pub cached_model_info: Option<AssetModelInfo>,
    pub selected_file_size_kb: Option<f32>,

    pub current_dir_entries: Vec<CachedDirEntry>,
    pub folder_tree: Option<FolderTree>,
    pub needs_refresh: bool,

    pub cooking_assets: Arc<Mutex<HashSet<PathBuf>>>,

    pub renaming_path: Option<PathBuf>,
    pub rename_buffer: String,
}

impl Default for AssetBrowserState {
    fn default() -> Self {
        let root = asset::resolve_asset_root();
        Self {
            root_dir: root.clone(),
            current_dir: root,
            folder_panel_width: 180.0,
            selected_file: None,
            search_query: String::new(),
            category_filter: AssetCategoryFilter::All,
            view_mode: ViewMode::Grid,
            icon_size: 72.0,
            status_message: None,
            dragged_asset: None,
            dragged_entity: None,
            editing_prefab_root: None,
            editing_prefab_path: None,
            prefab_scene_backup: None,
            cached_model_info: None,
            selected_file_size_kb: None,
            current_dir_entries: Vec::new(),
            folder_tree: None,
            needs_refresh: true,
            cooking_assets: Arc::new(Mutex::new(HashSet::new())),
            renaming_path: None,
            rename_buffer: String::new(),
        }
    }
}

pub fn open_prefab_for_editing(path: &Path, ctx: &mut EditorContext) {
    if let Ok(snapshot) = ctx.world.create_snapshot() {
        ctx.asset_browser_state.prefab_scene_backup = Some(snapshot);
        ctx.world.clear_scene();

        if let Ok(prefab) = ecs::serialization::Prefab::load_from_file(path) {
            if let Ok(root_id) = prefab.instantiate(
                ctx.world,
                Some(glam::Vec3::ZERO),
                Some(glam::Quat::IDENTITY),
                None::<ecs::Entity>,
            ) {
                *ctx.selected_entity = Some(root_id.0);
                ctx.asset_browser_state.editing_prefab_path = Some(path.to_path_buf());
                ctx.asset_browser_state.editing_prefab_root = Some(root_id.0);

                let key_light = ctx.world.spawn_empty("__PrefabPreviewKeyLight");
                ctx.world.add_transform(key_light, ecs::Transform {
                    position: glam::Vec3::new(4.0, 8.0, 5.0),
                    rotation: glam::Quat::from_euler(glam::EulerRot::XYZ, -50.0f32.to_radians(), 35.0f32.to_radians(), 0.0),
                    scale: glam::Vec3::ONE,
                });
                ctx.world.add_light(key_light, ecs::LightComponent {
                    light_type: ecs::LightType::Directional,
                    color: [1.0, 0.98, 0.95],
                    intensity: 3.5,
                    range: 100.0,
                    ..Default::default()
                });

                let fill_light = ctx.world.spawn_empty("__PrefabPreviewFillLight");
                ctx.world.add_transform(fill_light, ecs::Transform {
                    position: glam::Vec3::new(-4.0, -4.0, -5.0),
                    rotation: glam::Quat::from_euler(glam::EulerRot::XYZ, 40.0f32.to_radians(), -145.0f32.to_radians(), 0.0),
                    scale: glam::Vec3::ONE,
                });
                ctx.world.add_light(fill_light, ecs::LightComponent {
                    light_type: ecs::LightType::Directional,
                    color: [0.6, 0.75, 1.0],
                    intensity: 2.0,
                    range: 100.0,
                    ..Default::default()
                });

                ctx.camera.eye = glam::Vec3::new(2.0, 2.0, 3.5);
                ctx.camera.target = glam::Vec3::new(0.0, 0.5, 0.0);
                ctx.camera.up = glam::Vec3::Y;

                log::info!("Opened clean prefab template {:?} for editing", path);
            }
        }
    }
}

pub fn save_and_close_prefab_editing(ctx: &mut EditorContext, save: bool) {
    if let Some(path) = ctx.asset_browser_state.editing_prefab_path.take() {
        let path_str = path.to_string_lossy().replace('\\', "/");

        if save {
            if let Some(root_id) = ctx.asset_browser_state.editing_prefab_root.take() {
                if ctx.world.is_alive(root_id) {
                    if let Err(e) = ctx.world.save_entity_as_prefab(root_id, &path_str) {
                        log::error!("Failed to save prefab: {e}");
                    } else {
                        log::info!("Prefab saved: {:?}", path);
                    }
                }
            }
        }

        ctx.world.clear_scene();
        if let Some(backup) = ctx.asset_browser_state.prefab_scene_backup.take() {
            let _ = ctx.world.restore_snapshot(&backup);
        }

        if save {
            ctx.world.reload_prefab_instances(&path_str);
        }

        *ctx.selected_entity = None;
        ctx.asset_browser_state.editing_prefab_root = None;
        ctx.asset_browser_state.needs_refresh = true;
    }
}

pub fn save_entity_to_current_folder(ctx: &mut EditorContext, entity_id: u64) {
    let name = ctx.world.get_entity_name(entity_id);
    let base_name = if name.is_empty() { format!("Entity_{}", entity_id) } else { name };

    let mut filename = format!("{}.prefab", base_name);
    let mut target_path = ctx.asset_browser_state.current_dir.join(&filename);
    let mut counter = 1;
    while target_path.exists() {
        filename = format!("{} ({}).prefab", base_name, counter);
        target_path = ctx.asset_browser_state.current_dir.join(&filename);
        counter += 1;
    }

    let path_str = target_path.to_string_lossy().replace('\\', "/");

    match ctx.world.save_entity_as_prefab(entity_id, &path_str) {
        Ok(_) => {
            log::info!("[PREFAB] Created prefab at {:?}", target_path);

            ctx.world.add_prefab_ref(entity_id, ecs::PrefabRef {
                path: path_str,
            });

            ctx.asset_browser_state.needs_refresh = true;
            ctx.asset_browser_state.selected_file = Some(target_path);
        }
        Err(e) => {
            log::error!("[PREFAB] Failed to create prefab: {e}");
        }
    }
}

fn create_new_folder(ctx: &mut EditorContext) {
    let current_dir = ctx.asset_browser_state.current_dir.clone();
    let mut folder_name = "New Folder".to_string();
    let mut counter = 1;
    let mut target_path = current_dir.join(&folder_name);

    while target_path.exists() {
        folder_name = format!("New Folder ({})", counter);
        target_path = current_dir.join(&folder_name);
        counter += 1;
    }

    if let Ok(_) = fs::create_dir(&target_path) {
        ctx.asset_browser_state.needs_refresh = true;
        ctx.asset_browser_state.selected_file = Some(target_path.clone());
        ctx.asset_browser_state.renaming_path = Some(target_path);
        ctx.asset_browser_state.rename_buffer = folder_name;
    }
}

fn commit_rename(ctx: &mut EditorContext, old_path: &Path) {
    let new_name = ctx.asset_browser_state.rename_buffer.trim();
    if !new_name.is_empty() {
        let new_path = old_path.with_file_name(new_name);
        if new_path != old_path && !new_path.exists() {
            if fs::rename(old_path, &new_path).is_ok() {
                if ctx.asset_browser_state.selected_file.as_deref() == Some(old_path) {
                    ctx.asset_browser_state.selected_file = Some(new_path.clone());
                }
                if ctx.asset_browser_state.current_dir == old_path {
                    ctx.asset_browser_state.current_dir = new_path;
                }
                ctx.asset_browser_state.needs_refresh = true;
            }
        }
    }
    ctx.asset_browser_state.renaming_path = None;
    ctx.asset_browser_state.rename_buffer.clear();
}

fn cancel_rename(ctx: &mut EditorContext) {
    ctx.asset_browser_state.renaming_path = None;
    ctx.asset_browser_state.rename_buffer.clear();
}

pub fn draw_asset_browser(ui: &mut Ui, ctx: &mut EditorContext) {
    if !ctx.asset_browser_state.root_dir.exists() {
        ctx.asset_browser_state.root_dir = asset::resolve_asset_root();
        ctx.asset_browser_state.current_dir = ctx.asset_browser_state.root_dir.clone();
        ctx.asset_browser_state.needs_refresh = true;
    }

    if ctx.asset_browser_state.needs_refresh {
        refresh_cache(ctx);
        ctx.asset_browser_state.needs_refresh = false;
    }

    let is_dragging_entity = ctx.asset_browser_state.dragged_entity.is_some();

    let browser_rect = ui.available_rect_before_wrap();

    egui::Frame::new().inner_margin(4.0).show(ui, |ui| {
        draw_top_bar(ui, ctx);
    });

    ui.separator();

    let show_preview = ctx.asset_browser_state.selected_file.is_some();
    let available_h = ui.available_height();

    ui.with_layout(
        egui::Layout::left_to_right(egui::Align::TOP).with_cross_justify(true),
        |ui| {
            let folder_w = ctx.asset_browser_state.folder_panel_width;

            ui.allocate_ui_with_layout(
                Vec2::new(folder_w, available_h),
                egui::Layout::top_down(egui::Align::LEFT),
                |ui| {
                    ui.set_min_height(available_h);
                    ui.heading(RichText::new("📁 Folders").size(14.0));
                    ui.separator();
                    ScrollArea::vertical()
                        .id_salt("folder_tree_scroll")
                        .auto_shrink([false, false])
                        .show(ui, |ui| {
                            let tree_opt = ctx.asset_browser_state.folder_tree.clone();
                            if let Some(tree) = tree_opt {
                                draw_cached_folder_tree(ui, &tree, ctx);
                            }
                        });
                },
            );

            let (sep_rect, _) = ui.allocate_exact_size(Vec2::new(8.0, available_h), Sense::hover());
            let sep_resp = ui.interact(sep_rect, ui.id().with("folders_splitter"), Sense::drag());

            if sep_resp.dragged() {
                let delta = sep_resp.drag_delta().x;
                let max_w = (ui.max_rect().width() - 200.0).max(120.0);
                ctx.asset_browser_state.folder_panel_width =
                    (ctx.asset_browser_state.folder_panel_width + delta).clamp(100.0, max_w);
                ui.ctx().request_repaint();
            }

            if sep_resp.hovered() || sep_resp.dragged() {
                ui.ctx().set_cursor_icon(egui::CursorIcon::ResizeHorizontal);
            }

            let line_color = if sep_resp.dragged() {
                ui.visuals().widgets.active.bg_fill
            } else if sep_resp.hovered() {
                ui.visuals().widgets.hovered.bg_fill
            } else {
                ui.visuals().widgets.noninteractive.bg_stroke.color
            };
            ui.painter().vline(sep_rect.center().x, sep_rect.y_range(), egui::Stroke::new(1.0, line_color));

            let available_w = ui.available_width() - if show_preview { 220.0 } else { 0.0 };
            ui.allocate_ui_with_layout(
                Vec2::new(available_w.max(200.0), available_h),
                egui::Layout::top_down(egui::Align::LEFT),
                |ui| {
                    ui.set_min_height(available_h);
                    draw_file_view(ui, ctx);
                },
            );

            if show_preview {
                ui.separator();
                ui.allocate_ui_with_layout(
                    Vec2::new(220.0, available_h),
                    egui::Layout::top_down(egui::Align::LEFT),
                    |ui| {
                        ui.set_min_height(available_h);
                        draw_asset_preview_drawer(ui, ctx);
                    },
                );
            }
        },
    );

    if is_dragging_entity {
        ui.painter().rect_stroke(
            browser_rect,
            4.0,
            egui::Stroke::new(2.0, Color32::from_rgb(255, 215, 0)),
            egui::StrokeKind::Inside,
        );
    }

    if ui.input(|i| i.pointer.any_released()) {
        if let Some(dragged_entity_id) = ctx.asset_browser_state.dragged_entity.take() {
            let pointer_pos = ui.input(|i| i.pointer.interact_pos().or(i.pointer.hover_pos()));
            if let Some(pos) = pointer_pos {
                if browser_rect.contains(pos) {
                    save_entity_to_current_folder(ctx, dragged_entity_id);
                }
            }
        }
        ctx.asset_browser_state.dragged_asset = None;
    }
}

fn refresh_cache(ctx: &mut EditorContext) {
    let current_dir = ctx.asset_browser_state.current_dir.clone();
    let root_dir = ctx.asset_browser_state.root_dir.clone();

    ctx.asset_browser_state.current_dir_entries.clear();
    
    if let Ok(entries) = fs::read_dir(&current_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            let name = path.file_name().unwrap_or_default().to_string_lossy().to_string();
            let is_dir = path.is_dir();
            let ext = path.extension().unwrap_or_default().to_string_lossy().to_lowercase();
            
            ctx.asset_browser_state.current_dir_entries.push(CachedDirEntry {
                path,
                name,
                is_dir,
                ext,
            });
        }
    }
    ctx.asset_browser_state.current_dir_entries.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    ctx.asset_browser_state.folder_tree = Some(build_folder_tree(&root_dir));
}

fn build_folder_tree(dir: &Path) -> FolderTree {
    let name = dir.file_name().unwrap_or_default().to_string_lossy().to_string();
    let mut subdirs = Vec::new();
    
    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                subdirs.push(build_folder_tree(&path));
            }
        }
    }
    subdirs.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    
    FolderTree {
        path: dir.to_path_buf(),
        name,
        subdirs,
    }
}

fn draw_top_bar(ui: &mut Ui, ctx: &mut EditorContext) {
    ui.horizontal(|ui| {
        let current_dir = ctx.asset_browser_state.current_dir.clone();
        let root_dir = ctx.asset_browser_state.root_dir.clone();
        
        let can_go_up = current_dir != root_dir && current_dir.starts_with(&root_dir);
        if ui.add_enabled(can_go_up, egui::Button::new("⬆ Up")).clicked() {
            if let Some(parent) = current_dir.parent() {
                ctx.asset_browser_state.current_dir = parent.to_path_buf();
                ctx.asset_browser_state.needs_refresh = true;
                ctx.asset_browser_state.selected_file = None;
                ctx.asset_browser_state.cached_model_info = None;
            }
        }

        if ui.button("↻ Refresh").clicked() {
            ctx.asset_browser_state.needs_refresh = true;
            ctx.asset_browser_state.selected_file = None;
            ctx.asset_browser_state.cached_model_info = None;
        }

        if ui.button("➕ Folder").on_hover_text("Create new folder").clicked() {
            create_new_folder(ctx);
        }

        ui.separator();

        let rel_path = current_dir.strip_prefix(&root_dir).unwrap_or(&current_dir);
        let path_str = if rel_path.as_os_str().is_empty() {
            "res/".to_string()
        } else {
            format!("res/{}", rel_path.to_string_lossy().replace('\\', "/"))
        };
        ui.label(RichText::new(path_str).strong().color(Color32::LIGHT_BLUE));

        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if ctx.asset_browser_state.view_mode == ViewMode::Grid {
                ui.add(egui::Slider::new(&mut ctx.asset_browser_state.icon_size, 32.0..=128.0).show_value(false).text("🔍"));
                ui.separator();
            }

            ui.selectable_value(&mut ctx.asset_browser_state.view_mode, ViewMode::List, "☰ List");
            ui.selectable_value(&mut ctx.asset_browser_state.view_mode, ViewMode::Grid, "🗂 Grid");

            ui.separator();

            ui.add(egui::TextEdit::singleline(&mut ctx.asset_browser_state.search_query).hint_text("🔍 Search..."));

            ui.separator();

            egui::ComboBox::from_id_salt("asset_category_filter")
                .selected_text(match ctx.asset_browser_state.category_filter {
                    AssetCategoryFilter::All => "All Assets",
                    AssetCategoryFilter::Models => "📐 Models",
                    AssetCategoryFilter::Textures => "🎨 Textures",
                    AssetCategoryFilter::Scenes => "🎬 Scenes",
                    AssetCategoryFilter::Prefabs => "🧩 Prefabs",
                })
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut ctx.asset_browser_state.category_filter, AssetCategoryFilter::All, "All Assets");
                    ui.selectable_value(&mut ctx.asset_browser_state.category_filter, AssetCategoryFilter::Models, "📐 Models");
                    ui.selectable_value(&mut ctx.asset_browser_state.category_filter, AssetCategoryFilter::Textures, "🎨 Textures");
                    ui.selectable_value(&mut ctx.asset_browser_state.category_filter, AssetCategoryFilter::Scenes, "🎬 Scenes");
                });
        });
    });
}

fn draw_cached_folder_tree(ui: &mut Ui, node: &FolderTree, ctx: &mut EditorContext) {
    let is_active = node.path == ctx.asset_browser_state.current_dir;
    
    if node.subdirs.is_empty() {
        if ui.selectable_label(is_active, format!("📁 {}", node.name)).clicked() {
            if ctx.asset_browser_state.current_dir != node.path {
                ctx.asset_browser_state.current_dir = node.path.clone();
                ctx.asset_browser_state.needs_refresh = true;
            }
        }
    } else {
        let id = ui.make_persistent_id(&node.path);
        egui::collapsing_header::CollapsingState::load_with_default_open(ui.ctx(), id, false)
            .show_header(ui, |ui| {
                if ui.selectable_label(is_active, format!("📁 {}", node.name)).clicked() {
                    if ctx.asset_browser_state.current_dir != node.path {
                        ctx.asset_browser_state.current_dir = node.path.clone();
                        ctx.asset_browser_state.needs_refresh = true;
                    }
                }
            })
            .body(|ui| {
                for sub in &node.subdirs {
                    draw_cached_folder_tree(ui, sub, ctx);
                }
            });
    }
}

fn draw_file_view(ui: &mut Ui, ctx: &mut EditorContext) {
    if !ui.ctx().egui_wants_keyboard_input() {
        if let Some(selected_path) = ctx.asset_browser_state.selected_file.clone() {
            if ui.input(|i| i.key_pressed(egui::Key::F2)) {
                let name = selected_path.file_name().unwrap_or_default().to_string_lossy().to_string();
                ctx.asset_browser_state.renaming_path = Some(selected_path.clone());
                ctx.asset_browser_state.rename_buffer = name;
            }

            if ui.input(|i| i.key_pressed(egui::Key::Delete)) {
                if selected_path.is_dir() {
                    let _ = fs::remove_dir_all(&selected_path);
                } else {
                    let _ = fs::remove_file(&selected_path);
                }
                ctx.asset_browser_state.selected_file = None;
                ctx.asset_browser_state.cached_model_info = None;
                ctx.asset_browser_state.selected_file_size_kb = None;
                ctx.asset_browser_state.needs_refresh = true;
            }
        }
    }

    let search_lower = ctx.asset_browser_state.search_query.to_lowercase();
    let filter = ctx.asset_browser_state.category_filter;
    let view_mode = ctx.asset_browser_state.view_mode;
    let icon_size = ctx.asset_browser_state.icon_size;
    
    let entries = ctx.asset_browser_state.current_dir_entries.clone();

    let mut folders = Vec::new();
    let mut files = Vec::new();

    for entry in &entries {
        if !search_lower.is_empty() && !entry.name.to_lowercase().contains(&search_lower) {
            continue;
        }

        if entry.is_dir {
            folders.push(entry);
        } else {
            let matches_category = match filter {
                AssetCategoryFilter::All => true,
                AssetCategoryFilter::Models => matches!(entry.ext.as_str(), "bin" | "glb" | "gltf"),
                AssetCategoryFilter::Textures => matches!(entry.ext.as_str(), "dds" | "png" | "jpg" | "jpeg" | "tga"),
                AssetCategoryFilter::Scenes => entry.ext == "ron",
                AssetCategoryFilter::Prefabs => entry.ext == "prefab",
            };

            if matches_category {
                files.push(entry);
            }
        }
    }

    ScrollArea::vertical()
        .id_salt("main_file_scroll")
        .auto_shrink([false, false])
        .show(ui, |ui| {
            if view_mode == ViewMode::Grid {
                let item_width = icon_size + 16.0;
                let spacing = 12.0;

                let available_w = (ui.available_width() - 8.0).max(item_width);
                let cols = ((available_w + spacing) / (item_width + spacing)).floor() as usize;
                let cols = cols.max(1);

                let mut all_items: Vec<&CachedDirEntry> = Vec::with_capacity(folders.len() + files.len());
                all_items.extend(folders);
                all_items.extend(files);

                ui.vertical(|ui| {
                    ui.spacing_mut().item_spacing = Vec2::new(spacing, spacing);
                    
                    for row in all_items.chunks(cols) {
                        ui.horizontal(|ui| {
                            ui.spacing_mut().item_spacing = Vec2::new(spacing, spacing);
                            for item in row {
                                if item.is_dir {
                                    draw_folder_card(ui, item, ctx);
                                } else {
                                    draw_file_card(ui, item, ctx);
                                }
                            }
                        });
                    }
                });

                let remaining_size = ui.available_size();
                let (_bg_rect, bg_resp) = ui.allocate_exact_size(
                    Vec2::new(ui.available_width(), remaining_size.y.max(60.0)),
                    Sense::click(),
                );

                if bg_resp.clicked() {
                    ctx.asset_browser_state.selected_file = None;
                    ctx.asset_browser_state.cached_model_info = None;
                    ctx.asset_browser_state.selected_file_size_kb = None;
                }

                bg_resp.context_menu(|ui| {
                    if ui.button("📁 New Folder").clicked() {
                        create_new_folder(ctx);
                        ui.close();
                    }
                    ui.separator();
                    if ui.button("↻ Refresh").clicked() {
                        ctx.asset_browser_state.needs_refresh = true;
                        ui.close();
                    }
                    if ui.button("📂 Show in Explorer").clicked() {
                        reveal_in_explorer(&ctx.asset_browser_state.current_dir);
                        ui.close();
                    }
                });
            } else {
                for folder in folders {
                    draw_folder_list_item(ui, folder, ctx);
                }
                for file in files {
                    draw_file_list_item(ui, file, ctx);
                }
            }
        });

    let drop_area = ui.max_rect();
    if ui.input(|i| i.pointer.any_released()) {
        if let Some(entity_id) = ctx.asset_browser_state.dragged_entity.take() {
            if let Some(mouse_pos) = ui.ctx().pointer_hover_pos() {
                if drop_area.contains(mouse_pos) {
                    let name = ctx.world.get_entity_name(entity_id);
                    let clean_name = if name.is_empty() { format!("Entity_{}", entity_id) } else { name };
                    
                    let mut filename = format!("{}.prefab", clean_name);
                    let mut target_path = ctx.asset_browser_state.current_dir.join(&filename);
                    let mut counter = 1;
                    while target_path.exists() {
                        filename = format!("{} ({}).prefab", clean_name, counter);
                        target_path = ctx.asset_browser_state.current_dir.join(&filename);
                        counter += 1;
                    }

                    if let Err(e) = ctx.world.save_entity_as_prefab(entity_id, &target_path.to_string_lossy()) {
                        log::error!("Failed to create prefab: {e}");
                    } else {
                        log::info!("Created prefab: {:?}", target_path);
                        ctx.asset_browser_state.needs_refresh = true;
                        ctx.asset_browser_state.selected_file = Some(target_path);
                    }
                }
            }
        }
    }
}

fn draw_folder_card(ui: &mut Ui, entry: &CachedDirEntry, ctx: &mut EditorContext) {
    let size = ctx.asset_browser_state.icon_size;
    let card_size = Vec2::new(size + 16.0, size + 28.0);
    
    let is_selected = ctx.asset_browser_state.selected_file.as_deref() == Some(&entry.path);
    let is_renaming = ctx.asset_browser_state.renaming_path.as_deref() == Some(&entry.path);

    let (rect, resp) = ui.allocate_exact_size(card_size, Sense::click());

    let bg_color = if is_selected {
        Color32::from_rgb(45, 80, 120)
    } else if resp.hovered() {
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

    if is_renaming {
        let text_rect = egui::Rect::from_min_size(
            rect.center_bottom() - Vec2::new((rect.width() * 0.9) * 0.5, 22.0),
            Vec2::new(rect.width() * 0.9, 18.0),
        );
        let text_res = ui.put(
            text_rect,
            egui::TextEdit::singleline(&mut ctx.asset_browser_state.rename_buffer)
                .font(egui::FontId::proportional(11.0)),
        );

        if text_res.lost_focus() || ui.input(|i| i.key_pressed(egui::Key::Enter)) {
            commit_rename(ctx, &entry.path);
        } else if ui.input(|i| i.key_pressed(egui::Key::Escape)) {
            cancel_rename(ctx);
        } else if !text_res.has_focus() {
            text_res.request_focus();
        }
    } else {
        let truncated = truncate_string(&entry.name, 12);
        ui.painter().text(
            rect.center_bottom() - Vec2::new(0.0, 6.0),
            egui::Align2::CENTER_BOTTOM,
            truncated,
            egui::FontId::proportional(11.0),
            Color32::WHITE,
        );
    }

    if resp.clicked() || resp.secondary_clicked() {
        ctx.asset_browser_state.selected_file = Some(entry.path.clone());
        ctx.asset_browser_state.cached_model_info = None;
        ctx.asset_browser_state.selected_file_size_kb = None;
    }

    if resp.double_clicked() {
        ctx.asset_browser_state.current_dir = entry.path.clone();
        ctx.asset_browser_state.needs_refresh = true;
        ctx.asset_browser_state.selected_file = None;
        ctx.asset_browser_state.cached_model_info = None;
    }

    resp.context_menu(|ui| {
        if ui.button("📂 Open Folder").clicked() {
            ctx.asset_browser_state.current_dir = entry.path.clone();
            ctx.asset_browser_state.needs_refresh = true;
            ctx.asset_browser_state.selected_file = None;
            ui.close();
        }
        if ui.button("📁 New Folder Inside").clicked() {
            ctx.asset_browser_state.current_dir = entry.path.clone();
            create_new_folder(ctx);
            ui.close();
        }
        if ui.button("✏ Rename (F2)").clicked() {
            ctx.asset_browser_state.renaming_path = Some(entry.path.clone());
            ctx.asset_browser_state.rename_buffer = entry.name.clone();
            ui.close();
        }
        if ui.button("📂 Show in Explorer").clicked() {
            reveal_in_explorer(&entry.path);
            ui.close();
        }
        ui.separator();
        if ui.button(RichText::new("🗑 Delete (Del)").color(Color32::RED)).clicked() {
            let _ = fs::remove_dir_all(&entry.path);
            ctx.asset_browser_state.needs_refresh = true;
            ctx.asset_browser_state.selected_file = None;
            ui.close();
        }
    });
}

fn draw_file_card(ui: &mut Ui, entry: &CachedDirEntry, ctx: &mut EditorContext) {
    let size = ctx.asset_browser_state.icon_size;
    let card_size = Vec2::new(size + 16.0, size + 28.0);
    
    let is_selected = ctx.asset_browser_state.selected_file.as_deref() == Some(&entry.path);
    let is_renaming = ctx.asset_browser_state.renaming_path.as_deref() == Some(&entry.path);

    let (icon, icon_color) = match entry.ext.as_str() {
        "bin" => ("📐", Color32::from_rgb(100, 200, 255)),
        "glb" | "gltf" => ("📦", Color32::from_rgb(255, 180, 80)),
        "dds" | "png" | "jpg" => ("🎨", Color32::from_rgb(220, 100, 255)),
        "ron" => ("🎬", Color32::from_rgb(100, 255, 150)),
        "prefab" => ("🧩", Color32::from_rgb(255, 215, 0)),
        _ => ("📄", Color32::LIGHT_GRAY),
    };

    let (rect, resp) = ui.allocate_exact_size(card_size, Sense::click_and_drag());

    let bg_color = if is_selected {
        Color32::from_rgb(45, 80, 120)
    } else if resp.hovered() {
        Color32::from_rgb(45, 48, 56)
    } else {
        Color32::from_rgb(32, 34, 40)
    };

    ui.painter().rect_filled(rect, 4.0, bg_color);

    let is_cooking = {
        let lock = ctx.asset_browser_state.cooking_assets.lock().unwrap();
        lock.contains(&entry.path)
    };

    if is_cooking {
        ui.put(
            egui::Rect::from_center_size(rect.center_top() + Vec2::new(0.0, size * 0.4), Vec2::splat(size * 0.45)),
            egui::Spinner::new().size(size * 0.45),
        );
    } else {
        ui.painter().text(
            rect.center_top() + Vec2::new(0.0, size * 0.4),
            egui::Align2::CENTER_CENTER,
            icon,
            egui::FontId::proportional(size * 0.45),
            icon_color,
        );
    }

    if resp.drag_started() && (entry.ext == "bin" || entry.ext == "prefab") {
        ctx.asset_browser_state.dragged_asset = Some(entry.path.clone());
    }

    if resp.double_clicked() {
        if entry.ext == "bin" {
            spawn_model_in_scene(&entry.path, ctx);
        } else if entry.ext == "prefab" {
            open_prefab_for_editing(&entry.path, ctx);
        } else if entry.ext == "scene" || entry.ext == "ron" {
            ctx.scene_manager.queue_load(&entry.path);
            *ctx.selected_entity = None;
            log::info!("Switched to scene: {:?}", entry.path);
        }
    }

    resp.context_menu(|ui| {
        if entry.ext == "prefab" {
            if ui.button("➕ Spawn in Scene").clicked() {
                let pos = ctx.camera.target;
                let _ = ctx.world.spawn_prefab(&entry.path.to_string_lossy(), pos, glam::Quat::IDENTITY);
                ui.close();
            }
            if ui.button("✏ Edit Prefab").clicked() {
                open_prefab_for_editing(&entry.path, ctx);
                ui.close();
            }
            ui.separator();
        }
    });

    if is_renaming {
        let text_rect = egui::Rect::from_min_size(
            rect.center_bottom() - Vec2::new((rect.width() * 0.9) * 0.5, 22.0),
            Vec2::new(rect.width() * 0.9, 18.0),
        );
        let text_res = ui.put(
            text_rect,
            egui::TextEdit::singleline(&mut ctx.asset_browser_state.rename_buffer)
                .font(egui::FontId::proportional(11.0)),
        );

        if text_res.lost_focus() || ui.input(|i| i.key_pressed(egui::Key::Enter)) {
            commit_rename(ctx, &entry.path);
        } else if ui.input(|i| i.key_pressed(egui::Key::Escape)) {
            cancel_rename(ctx);
        } else if !text_res.has_focus() {
            text_res.request_focus();
        }
    } else {
        let truncated = truncate_string(&entry.name, 12);
        ui.painter().text(
            rect.center_bottom() - Vec2::new(0.0, 6.0),
            egui::Align2::CENTER_BOTTOM,
            truncated,
            egui::FontId::proportional(11.0),
            Color32::WHITE,
        );
    }

    if resp.clicked() || resp.secondary_clicked() {
        ctx.asset_browser_state.selected_file = Some(entry.path.clone());
        ctx.asset_browser_state.selected_file_size_kb = fs::metadata(&entry.path).ok().map(|m| m.len() as f32 / 1024.0);

        if entry.ext == "bin" {
            ctx.asset_browser_state.cached_model_info = read_model_info(&entry.path);
        } else {
            ctx.asset_browser_state.cached_model_info = None;
        }
    }

    if resp.drag_started() && (entry.ext == "bin" || entry.ext == "prefab") {
        ctx.asset_browser_state.dragged_asset = Some(entry.path.clone());
    }

    if resp.double_clicked() && entry.ext == "bin" {
        spawn_model_in_scene(&entry.path, ctx);
    }

    resp.context_menu(|ui| {
        if entry.ext == "bin" {
            if ui.button("➕ Spawn in Scene").clicked() {
                spawn_model_in_scene(&entry.path, ctx);
                ui.close();
            }
        }

        if entry.ext == "glb" || entry.ext == "gltf" {
            if ui.button("🔥 Cook to .bin").clicked() {
                let cooking_tasks = ctx.asset_browser_state.cooking_assets.clone();
                cook_asset(&entry.path, cooking_tasks);
                ui.close();
            }
        }

        if ui.button("✏ Rename (F2)").clicked() {
            ctx.asset_browser_state.renaming_path = Some(entry.path.clone());
            ctx.asset_browser_state.rename_buffer = entry.name.clone();
            ui.close();
        }

        if ui.button("📂 Show in Explorer").clicked() {
            reveal_in_explorer(&entry.path);
            ui.close();
        }

        ui.separator();
        if ui.button(RichText::new("🗑 Delete (Del)").color(Color32::RED)).clicked() {
            let _ = fs::remove_file(&entry.path);
            ctx.asset_browser_state.needs_refresh = true;
            ctx.asset_browser_state.selected_file = None;
            ctx.asset_browser_state.cached_model_info = None;
            ctx.asset_browser_state.selected_file_size_kb = None;
            ui.close();
        }
    });
}

fn draw_folder_list_item(ui: &mut Ui, entry: &CachedDirEntry, ctx: &mut EditorContext) {
    let is_selected = ctx.asset_browser_state.selected_file.as_deref() == Some(&entry.path);
    let is_renaming = ctx.asset_browser_state.renaming_path.as_deref() == Some(&entry.path);

    ui.horizontal(|ui| {
        if is_renaming {
            ui.label("📁");
            let text_res = ui.add(
                egui::TextEdit::singleline(&mut ctx.asset_browser_state.rename_buffer).desired_width(180.0),
            );
            if text_res.lost_focus() || ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                commit_rename(ctx, &entry.path);
            } else if ui.input(|i| i.key_pressed(egui::Key::Escape)) {
                cancel_rename(ctx);
            } else if !text_res.has_focus() {
                text_res.request_focus();
            }
        } else {
            let resp = ui.selectable_label(is_selected, format!("📁 {}", entry.name));

            if resp.clicked() || resp.secondary_clicked() {
                ctx.asset_browser_state.selected_file = Some(entry.path.clone());
                ctx.asset_browser_state.cached_model_info = None;
                ctx.asset_browser_state.selected_file_size_kb = None;
            }

            if resp.double_clicked() {
                ctx.asset_browser_state.current_dir = entry.path.clone();
                ctx.asset_browser_state.needs_refresh = true;
                ctx.asset_browser_state.selected_file = None;
            }

            resp.context_menu(|ui| {
                if ui.button("📂 Open Folder").clicked() {
                    ctx.asset_browser_state.current_dir = entry.path.clone();
                    ctx.asset_browser_state.needs_refresh = true;
                    ctx.asset_browser_state.selected_file = None;
                    ui.close();
                }
                if ui.button("📁 New Folder Inside").clicked() {
                    ctx.asset_browser_state.current_dir = entry.path.clone();
                    create_new_folder(ctx);
                    ui.close();
                }
                if ui.button("✏ Rename (F2)").clicked() {
                    ctx.asset_browser_state.renaming_path = Some(entry.path.clone());
                    ctx.asset_browser_state.rename_buffer = entry.name.clone();
                    ui.close();
                }
                if ui.button("📂 Show in Explorer").clicked() {
                    reveal_in_explorer(&entry.path);
                    ui.close();
                }
                ui.separator();
                if ui.button(RichText::new("🗑 Delete (Del)").color(Color32::RED)).clicked() {
                    let _ = fs::remove_dir_all(&entry.path);
                    ctx.asset_browser_state.needs_refresh = true;
                    ctx.asset_browser_state.selected_file = None;
                    ui.close();
                }
            });
        }
    });
}

fn draw_file_list_item(ui: &mut Ui, entry: &CachedDirEntry, ctx: &mut EditorContext) {
    let is_selected = ctx.asset_browser_state.selected_file.as_deref() == Some(&entry.path);
    let is_renaming = ctx.asset_browser_state.renaming_path.as_deref() == Some(&entry.path);

    let icon = match entry.ext.as_str() {
        "bin" => "📐",
        "glb" | "gltf" => "📦",
        "dds" | "png" | "jpg" => "🎨",
        "ron" => "🎬",
        _ => "📄",
    };

    ui.horizontal(|ui| {
        let is_cooking = {
            let lock = ctx.asset_browser_state.cooking_assets.lock().unwrap();
            lock.contains(&entry.path)
        };

        if is_cooking {
            ui.spinner();
        }

        if is_renaming {
            ui.label(icon);
            let text_res = ui.add(
                egui::TextEdit::singleline(&mut ctx.asset_browser_state.rename_buffer).desired_width(180.0),
            );
            if text_res.lost_focus() || ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                commit_rename(ctx, &entry.path);
            } else if ui.input(|i| i.key_pressed(egui::Key::Escape)) {
                cancel_rename(ctx);
            } else if !text_res.has_focus() {
                text_res.request_focus();
            }
        } else {
            let resp = ui.selectable_label(is_selected, format!("{} {}", icon, entry.name))
                .interact(Sense::click_and_drag());

            if resp.clicked() || resp.secondary_clicked() {
                ctx.asset_browser_state.selected_file = Some(entry.path.clone());
                ctx.asset_browser_state.selected_file_size_kb = fs::metadata(&entry.path).ok().map(|m| m.len() as f32 / 1024.0);

                if entry.ext == "bin" {
                    ctx.asset_browser_state.cached_model_info = read_model_info(&entry.path);
                } else {
                    ctx.asset_browser_state.cached_model_info = None;
                }
            }

            if resp.drag_started() && (entry.ext == "bin" || entry.ext == "prefab") {
                ctx.asset_browser_state.dragged_asset = Some(entry.path.clone());
            }

            if resp.double_clicked() && entry.ext == "bin" {
                spawn_model_in_scene(&entry.path, ctx);
            }

            resp.context_menu(|ui| {
                if entry.ext == "bin" {
                    if ui.button("➕ Spawn in Scene").clicked() {
                        spawn_model_in_scene(&entry.path, ctx);
                        ui.close();
                    }
                }
                if entry.ext == "glb" || entry.ext == "gltf" {
                    if ui.button("🔥 Cook to .bin").clicked() {
                        let cooking_tasks = ctx.asset_browser_state.cooking_assets.clone();
                        cook_asset(&entry.path, cooking_tasks);
                        ui.close();
                    }
                }
                if ui.button("✏ Rename (F2)").clicked() {
                    ctx.asset_browser_state.renaming_path = Some(entry.path.clone());
                    ctx.asset_browser_state.rename_buffer = entry.name.clone();
                    ui.close();
                }
                if ui.button("📂 Show in Explorer").clicked() {
                    reveal_in_explorer(&entry.path);
                    ui.close();
                }
                ui.separator();
                if ui.button(RichText::new("🗑 Delete (Del)").color(Color32::RED)).clicked() {
                    let _ = fs::remove_file(&entry.path);
                    ctx.asset_browser_state.needs_refresh = true;
                    ctx.asset_browser_state.selected_file = None;
                    ctx.asset_browser_state.cached_model_info = None;
                    ctx.asset_browser_state.selected_file_size_kb = None;
                    ui.close();
                }
            });
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
    
    if let Some(size) = ctx.asset_browser_state.selected_file_size_kb {
        ui.label(format!("Size: {:.1} KB", size));
    }

    ui.add_space(8.0);

    if ext == "bin" {
        if ui.button("➕ Spawn in Scene").clicked() {
            spawn_model_in_scene(&selected_path, ctx);
        }

        let model_info = ctx.asset_browser_state.cached_model_info.clone();
        if let Some(info) = model_info {
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
        let is_cooking = {
            let lock = ctx.asset_browser_state.cooking_assets.lock().unwrap();
            lock.contains(&selected_path)
        };

        if is_cooking {
            ui.horizontal(|ui| {
                ui.spinner();
                ui.label("Cooking...");
            });
        } else if ui.button("🔥 Cook to .bin").clicked() {
            let cooking_tasks = ctx.asset_browser_state.cooking_assets.clone();
            cook_asset(&selected_path, cooking_tasks);
        }
    } else if ext == "prefab" {
        if ui.button("➕ Spawn in Scene").clicked() {
            let pos = ctx.camera.target;
            let _ = ctx.world.spawn_prefab(&selected_path.to_string_lossy(), pos, glam::Quat::IDENTITY);
        }
        if ui.button("✏ Edit Prefab").clicked() {
            open_prefab_for_editing(&selected_path, ctx);
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
                *ctx.selected_entity = Some(spawned_id.0);
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

fn cook_asset(glb_path: &Path, cooking_tasks: Arc<Mutex<HashSet<PathBuf>>>) {
    let Some(cooker_bin) = asset::find_cooker_executable() else {
        log::error!("Cooker executable not found!");
        return;
    };

    let bin_path = glb_path.with_extension("bin");
    let glb_owned = glb_path.to_path_buf();
    let bin_owned = bin_path.clone();

    cooking_tasks.lock().unwrap().insert(glb_owned.clone());

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

        cooking_tasks.lock().unwrap().remove(&glb_owned);
    });
}

fn truncate_string(s: &str, max_len: usize) -> String {
    if s.chars().count() > max_len {
        let truncated: String = s.chars().take(max_len.saturating_sub(3)).collect();
        format!("{}...", truncated)
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