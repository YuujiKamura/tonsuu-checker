//! History panel for viewing and managing analysis history

use eframe::egui::{self, Color32, ColorImage, RichText, ScrollArea, TextureHandle, Vec2};
use tonsuu_store::{Store, VehicleStore};

/// Context menu action to be executed
#[derive(Clone)]
#[allow(dead_code)]
pub enum ContextAction {
    /// Re-analyze the selected entry
    ReAnalyze { hash: String, image_path: String },
    /// Register feedback (opens dialog)
    RegisterFeedback { hash: String },
    /// Assign vehicle (opens dialog)
    AssignVehicle { hash: String },
}

/// Panel for viewing analysis history and providing feedback
pub struct HistoryPanel {
    /// Currently selected entry hash
    selected_hash: Option<String>,
    /// Input field for actual tonnage feedback
    feedback_input: String,
    /// Toggle to show only entries with feedback
    show_only_with_feedback: bool,
    /// Cached texture for preview
    preview_texture: Option<TextureHandle>,
    /// Path of currently loaded preview
    preview_path: Option<String>,
    /// Context menu action to execute (returned to parent)
    pending_action: Option<ContextAction>,
    /// Show feedback dialog
    show_feedback_dialog: bool,
    /// Show vehicle assign dialog
    show_vehicle_dialog: bool,
    /// Selected vehicle ID for assignment
    selected_vehicle_id: Option<String>,
}

impl HistoryPanel {
    /// Create a new history panel
    pub fn new() -> Self {
        Self {
            selected_hash: None,
            feedback_input: String::new(),
            show_only_with_feedback: false,
            preview_texture: None,
            preview_path: None,
            pending_action: None,
            show_feedback_dialog: false,
            show_vehicle_dialog: false,
            selected_vehicle_id: None,
        }
    }

    /// Take pending action (consumed by caller)
    pub fn take_pending_action(&mut self) -> Option<ContextAction> {
        self.pending_action.take()
    }

    /// Load image from base64 or file path and create texture
    fn load_preview_texture(
        &mut self,
        ctx: &egui::Context,
        image_path: &str,
        thumbnail_base64: Option<&str>,
    ) -> Option<&TextureHandle> {
        // Check if already loaded (cache hit)
        if self.preview_path.as_deref() == Some(image_path) {
            return self.preview_texture.as_ref();
        }

        // Mark as loading to prevent re-processing
        self.preview_path = Some(image_path.to_string());
        self.preview_texture = None;

        // Try base64 first (faster than filesystem access)
        if let Some(base64_data) = thumbnail_base64 {
            use base64::{engine::general_purpose::STANDARD, Engine};

            // Remove data URL prefix if present
            let data = if base64_data.contains(',') {
                base64_data.split(',').nth(1).unwrap_or(base64_data)
            } else {
                base64_data
            };

            if let Ok(bytes) = STANDARD.decode(data) {
                if let Ok(img) = image::load_from_memory(&bytes) {
                    let rgba = img.to_rgba8();
                    let size = [rgba.width() as usize, rgba.height() as usize];
                    let pixels = rgba.into_raw();

                    let color_image = ColorImage::from_rgba_unmultiplied(size, &pixels);

                    let texture = ctx.load_texture(
                        format!("preview_{}", image_path),
                        color_image,
                        egui::TextureOptions::LINEAR,
                    );

                    self.preview_texture = Some(texture);
                    return self.preview_texture.as_ref();
                }
            }
        }

        // Fall back to file (skip exists() check - just try to open)
        let path = std::path::Path::new(image_path);
        if let Ok(img) = image::open(path) {
            let rgba = img.to_rgba8();
            let size = [rgba.width() as usize, rgba.height() as usize];
            let pixels = rgba.into_raw();

            let color_image = ColorImage::from_rgba_unmultiplied(size, &pixels);

            let texture = ctx.load_texture(
                format!("preview_{}", image_path),
                color_image,
                egui::TextureOptions::LINEAR,
            );

            self.preview_texture = Some(texture);
            return self.preview_texture.as_ref();
        }

        None
    }

    /// Calculate scaled size to fit within max dimensions while preserving aspect ratio
    fn calc_preview_size(texture: &TextureHandle, max_width: f32, max_height: f32) -> Vec2 {
        let original_size = texture.size_vec2();
        let scale_x = max_width / original_size.x;
        let scale_y = max_height / original_size.y;
        let scale = scale_x.min(scale_y); // Allow upscale for small thumbnails
        Vec2::new(original_size.x * scale, original_size.y * scale)
    }

    /// Render the panel UI
    pub fn ui(&mut self, ui: &mut egui::Ui, store: &mut Store, vehicle_store: &VehicleStore) {
        ui.heading("履歴");
        ui.separator();

        // Top: Filter checkbox
        ui.horizontal(|ui| {
            ui.checkbox(&mut self.show_only_with_feedback, "フィードバック済みのみ表示");
            ui.add_space(16.0);
            ui.label(format!(
                "全{}件 / フィードバック済み{}件",
                store.count(),
                store.feedback_count()
            ));
        });

        ui.add_space(8.0);

        // Get entries based on filter
        let entries = if self.show_only_with_feedback {
            store.entries_with_feedback()
        } else {
            store.all_entries()
        };

        if entries.is_empty() {
            ui.vertical_centered(|ui| {
                ui.add_space(40.0);
                ui.label(RichText::new("履歴がありません").color(Color32::GRAY));
                if self.show_only_with_feedback {
                    ui.label(
                        RichText::new("(フィードバック済みのデータがありません)")
                            .small()
                            .color(Color32::GRAY),
                    );
                }
            });
            return;
        }

        // Table header (monospace for alignment)
        ui.label(
            RichText::new(format!(
                "{:<25} {:>6} {:>6} {:>6} {}",
                "画像", "推定", "実測", "誤差", "日時"
            ))
            .strong()
            .monospace(),
        );
        ui.separator();

        // Scrollable table body
        ScrollArea::vertical()
            .max_height(ui.available_height() * 0.4)
            .show(ui, |ui| {
                for entry in &entries {
                    let is_selected = self
                        .selected_hash
                        .as_ref()
                        .map_or(false, |h| h == &entry.image_hash);

                    let hash = entry.image_hash.clone();
                    let image_path = entry.image_path.clone();

                    // Build row text
                    let filename = truncate_filename(&entry.image_path, 25);
                    let estimated = format!("{:.2}", entry.estimation.estimated_tonnage);
                    let actual = entry.actual_tonnage.map_or("-".to_string(), |t| format!("{:.2}", t));
                    let error = if let Some(act) = entry.actual_tonnage {
                        format!("{:+.2}", entry.estimation.estimated_tonnage - act)
                    } else {
                        "-".to_string()
                    };
                    let datetime = entry.analyzed_at.format("%m/%d %H:%M").to_string();

                    let row_text = format!(
                        "{:<25} {:>6} {:>6} {:>6} {}",
                        filename, estimated, actual, error, datetime
                    );

                    // Selectable label with context menu
                    let response = ui.selectable_label(is_selected, row_text);

                    // Left click to select
                    if response.clicked() {
                        self.selected_hash = Some(hash.clone());
                        self.feedback_input = entry
                            .actual_tonnage
                            .map_or(String::new(), |t| format!("{:.2}", t));
                    }

                    // Right click context menu
                    response.context_menu(|ui| {
                        if ui.button("🔄 再解析").clicked() {
                            self.pending_action = Some(ContextAction::ReAnalyze {
                                hash: hash.clone(),
                                image_path: image_path.clone(),
                            });
                            ui.close_menu();
                        }
                        if ui.button("✏️ 実測値を登録").clicked() {
                            self.selected_hash = Some(hash.clone());
                            self.show_feedback_dialog = true;
                            self.feedback_input = entry
                                .actual_tonnage
                                .map_or(String::new(), |t| format!("{:.2}", t));
                            ui.close_menu();
                        }
                        if ui.button("🚛 車両をアサイン").clicked() {
                            self.selected_hash = Some(hash.clone());
                            self.show_vehicle_dialog = true;
                            ui.close_menu();
                        }
                        ui.separator();
                        if ui.button("❌ 削除").clicked() {
                            // TODO: Implement delete
                            ui.close_menu();
                        }
                    });
                }
            });

        ui.separator();

        // Bottom: Feedback section (scrollable both directions)
        let ctx = ui.ctx().clone();
        ScrollArea::both()
            .id_salt("feedback_section_scroll")
            .max_height(ui.available_height())
            .show(ui, |ui| {
                self.render_feedback_section(ui, store, &ctx);
            });

        // Render dialogs
        self.render_feedback_dialog(ui, store);
        self.render_vehicle_dialog(ui, store, vehicle_store);
    }

    /// Render feedback input dialog
    fn render_feedback_dialog(&mut self, ui: &mut egui::Ui, store: &mut Store) {
        if !self.show_feedback_dialog {
            return;
        }

        let selected_hash = match &self.selected_hash {
            Some(h) => h.clone(),
            None => {
                self.show_feedback_dialog = false;
                return;
            }
        };

        egui::Window::new("実測値を登録")
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .show(ui.ctx(), |ui| {
                ui.horizontal(|ui| {
                    ui.label("実測トン数:");
                    ui.add(
                        egui::TextEdit::singleline(&mut self.feedback_input)
                            .desired_width(100.0)
                            .hint_text("0.00"),
                    );
                    ui.label("t");
                });

                ui.add_space(8.0);

                ui.horizontal(|ui| {
                    if ui.button("登録").clicked() {
                        if let Ok(tonnage) = self.feedback_input.parse::<f64>() {
                            if tonnage >= 0.0 {
                                // Extract path first to avoid borrow conflict
                                let image_path = store
                                    .get_by_hash(&selected_hash)
                                    .map(|e| e.image_path.clone());
                                if let Some(path_str) = image_path {
                                    let path = std::path::Path::new(&path_str);
                                    if let Err(e) = store.add_feedback(path, tonnage, None) {
                                        eprintln!("フィードバック登録エラー: {}", e);
                                    }
                                }
                                self.feedback_input.clear();
                                self.show_feedback_dialog = false;
                            }
                        }
                    }
                    if ui.button("キャンセル").clicked() {
                        self.show_feedback_dialog = false;
                    }
                });
            });
    }

    /// Render vehicle assignment dialog
    fn render_vehicle_dialog(
        &mut self,
        ui: &mut egui::Ui,
        store: &mut Store,
        vehicle_store: &VehicleStore,
    ) {
        if !self.show_vehicle_dialog {
            return;
        }

        let selected_hash = match &self.selected_hash {
            Some(h) => h.clone(),
            None => {
                self.show_vehicle_dialog = false;
                return;
            }
        };

        egui::Window::new("車両をアサイン")
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .show(ui.ctx(), |ui| {
                let vehicles = vehicle_store.all_vehicles();

                if vehicles.is_empty() {
                    ui.label(
                        RichText::new("登録車両がありません")
                            .color(Color32::GRAY)
                            .italics(),
                    );
                } else {
                    ui.label("車両を選択:");
                    ScrollArea::vertical().max_height(200.0).show(ui, |ui| {
                        for vehicle in &vehicles {
                            let is_selected = self
                                .selected_vehicle_id
                                .as_ref()
                                .map_or(false, |id| id == &vehicle.id);

                            let label = format!(
                                "{} ({:.1}t) {}",
                                vehicle.name,
                                vehicle.max_capacity,
                                vehicle
                                    .license_plate
                                    .as_ref()
                                    .map_or("".to_string(), |p| format!("[{}]", p))
                            );

                            if ui.selectable_label(is_selected, label).clicked() {
                                self.selected_vehicle_id = Some(vehicle.id.clone());
                            }
                        }
                    });
                }

                ui.add_space(8.0);

                ui.horizontal(|ui| {
                    let can_assign = self.selected_vehicle_id.is_some();
                    if ui.add_enabled(can_assign, egui::Button::new("アサイン")).clicked() {
                        if let Some(vehicle_id) = &self.selected_vehicle_id {
                            if let Some(vehicle) = vehicle_store.get_vehicle(vehicle_id) {
                                // Extract data first to avoid borrow conflict
                                let entry_data = store.get_by_hash(&selected_hash).map(|e| {
                                    (e.image_path.clone(), e.actual_tonnage)
                                });
                                if let Some((path_str, actual)) = entry_data {
                                    if let Some(actual_tonnage) = actual {
                                        let path = std::path::Path::new(&path_str);
                                        let _ = store.add_feedback_with_capacity(
                                            path,
                                            actual_tonnage,
                                            Some(vehicle.max_capacity),
                                            None,
                                        );
                                    }
                                }
                            }
                        }
                        self.show_vehicle_dialog = false;
                        self.selected_vehicle_id = None;
                    }
                    if ui.button("キャンセル").clicked() {
                        self.show_vehicle_dialog = false;
                        self.selected_vehicle_id = None;
                    }
                });
            });
    }

    /// Render the feedback input section with image preview and estimation details
    fn render_feedback_section(
        &mut self,
        ui: &mut egui::Ui,
        store: &mut Store,
        ctx: &egui::Context,
    ) {
        if let Some(ref selected_hash) = self.selected_hash.clone() {
            if let Some(entry) = store.get_by_hash(selected_hash) {
                let filename = truncate_filename(&entry.image_path, 50);
                let image_path = entry.image_path.clone();
                let thumbnail_base64 = entry.thumbnail_base64.clone();
                let estimation = entry.estimation.clone();

                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    ui.label(RichText::new("選択中:").strong());
                    ui.label(&filename);
                });

                ui.add_space(4.0);
                ui.horizontal(|ui| {
                    ui.label("実測トン数:");
                    let text_edit = egui::TextEdit::singleline(&mut self.feedback_input)
                        .desired_width(80.0)
                        .hint_text("0.00");
                    ui.add(text_edit);
                    ui.label("t");

                    ui.add_space(16.0);

                    if ui.button("登録").clicked() {
                        if let Ok(tonnage) = self.feedback_input.parse::<f64>() {
                            if tonnage >= 0.0 {
                                // Add feedback using image path
                                let path = std::path::Path::new(&image_path);
                                if let Err(e) = store.add_feedback(path, tonnage, None) {
                                    eprintln!("フィードバック登録エラー: {}", e);
                                } else {
                                    // Clear input on success
                                    self.feedback_input.clear();
                                }
                            }
                        }
                    }
                });

                ui.add_space(8.0);
                ui.separator();

                // Horizontal layout: Image preview (left) | Estimation details (right)
                ui.horizontal(|ui| {
                    // Left side: Image preview
                    ui.vertical(|ui| {
                        ui.label(RichText::new("画像プレビュー").strong());
                        ui.add_space(4.0);

                        // Load and display image preview
                        self.load_preview_texture(ctx, &image_path, thumbnail_base64.as_deref());

                        if let Some(ref texture) = self.preview_texture {
                            let preview_size = Self::calc_preview_size(texture, 240.0, 180.0);
                            ui.add(egui::Image::new(texture).fit_to_exact_size(preview_size));
                        } else {
                            // File does not exist or failed to load - show placeholder
                            egui::Frame::new()
                                .fill(Color32::from_rgb(50, 50, 50))
                                .corner_radius(4.0)
                                .inner_margin(egui::Margin::same(16))
                                .show(ui, |ui| {
                                    ui.set_min_size(Vec2::new(200.0, 150.0));
                                    ui.vertical_centered(|ui| {
                                        ui.add_space(50.0);
                                        ui.label(
                                            RichText::new("画像なし")
                                                .color(Color32::GRAY)
                                                .size(16.0),
                                        );
                                        ui.label(
                                            RichText::new("(ファイルが存在しません)")
                                                .color(Color32::DARK_GRAY)
                                                .small(),
                                        );
                                    });
                                });
                        }
                    });

                    ui.add_space(16.0);

                    // Right side: Estimation details
                    ui.vertical(|ui| {
                        ui.label(RichText::new("推定詳細").strong());
                        ui.add_space(4.0);

                        // Use monospace font for aligned display
                        let mono_font = egui::FontId::monospace(13.0);

                        // Basic estimation info
                        ui.label(
                            RichText::new(format!(
                                "車種:    {}",
                                if estimation.truck_type.is_empty() {
                                    "-".to_string()
                                } else {
                                    estimation.truck_type.clone()
                                }
                            ))
                            .font(mono_font.clone()),
                        );
                        ui.label(
                            RichText::new(format!(
                                "素材:    {}",
                                if estimation.material_type.is_empty() {
                                    "-".to_string()
                                } else {
                                    estimation.material_type.clone()
                                }
                            ))
                            .font(mono_font.clone()),
                        );
                        ui.label(
                            RichText::new(format!(
                                "体積:    {:.2} m³",
                                estimation.estimated_volume_m3
                            ))
                            .font(mono_font.clone()),
                        );
                        ui.label(
                            RichText::new(format!(
                                "推定:    {:.2} t",
                                estimation.estimated_tonnage
                            ))
                            .font(mono_font.clone()),
                        );
                        ui.label(
                            RichText::new(format!(
                                "信頼度:  {:.0}%",
                                estimation.confidence_score * 100.0
                            ))
                            .font(mono_font.clone()),
                        );

                        ui.add_space(4.0);
                        ui.label(RichText::new("---").font(mono_font.clone()).color(Color32::GRAY));
                        ui.add_space(4.0);

                        // Detailed measurements
                        ui.label(
                            RichText::new(format!(
                                "高さ:    {} m",
                                estimation
                                    .height
                                    .map_or("-".to_string(), |v| format!("{:.2}", v))
                            ))
                            .font(mono_font.clone()),
                        );
                        ui.label(
                            RichText::new(format!(
                                "充填L:  {}",
                                estimation
                                    .fill_ratio_l
                                    .map_or("-".to_string(), |v| format!("{:.2}", v))
                            ))
                            .font(mono_font.clone()),
                        );
                        ui.label(
                            RichText::new(format!(
                                "充填W:  {}",
                                estimation
                                    .fill_ratio_w
                                    .map_or("-".to_string(), |v| format!("{:.2}", v))
                            ))
                            .font(mono_font.clone()),
                        );
                        ui.label(
                            RichText::new(format!(
                                "充填Z:  {}",
                                estimation
                                    .fill_ratio_z
                                    .map_or("-".to_string(), |v| format!("{:.2}", v))
                            ))
                            .font(mono_font.clone()),
                        );
                        ui.label(
                            RichText::new(format!(
                                "詰まり:  {}",
                                estimation
                                    .packing_density
                                    .map_or("-".to_string(), |v| format!("{:.2}", v))
                            ))
                            .font(mono_font.clone()),
                        );

                        ui.add_space(4.0);
                        ui.label(RichText::new("---").font(mono_font.clone()).color(Color32::GRAY));
                        ui.add_space(4.0);

                        // Reasoning section with scroll area
                        ui.label(RichText::new("推論:").font(mono_font.clone()));
                        ui.add_space(2.0);

                        ScrollArea::vertical()
                            .id_salt("reasoning_scroll")
                            .max_height(80.0)
                            .max_width(280.0)
                            .show(ui, |ui| {
                                if estimation.reasoning.is_empty() {
                                    ui.label(
                                        RichText::new("(推論情報なし)")
                                            .color(Color32::GRAY)
                                            .italics(),
                                    );
                                } else {
                                    ui.label(&estimation.reasoning);
                                }
                            });
                    });
                });
            } else {
                // Selected entry no longer exists
                self.selected_hash = None;
            }
        } else {
            ui.add_space(8.0);
            ui.label(
                RichText::new("行をクリックして詳細を表示・フィードバックを登録")
                    .color(Color32::GRAY)
                    .italics(),
            );
        }
    }
}

impl Default for HistoryPanel {
    fn default() -> Self {
        Self::new()
    }
}

/// Truncate a filename to fit in the display
fn truncate_filename(path: &str, max_len: usize) -> String {
    // Get just the filename from the path
    let filename = std::path::Path::new(path)
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or(path);

    if filename.len() <= max_len {
        filename.to_string()
    } else {
        // Keep extension visible
        if let Some(dot_pos) = filename.rfind('.') {
            let ext = &filename[dot_pos..];
            let name_len = max_len.saturating_sub(ext.len() + 3); // 3 for "..."
            if name_len > 0 {
                format!("{}...{}", &filename[..name_len], ext)
            } else {
                format!("{}...", &filename[..max_len.saturating_sub(3)])
            }
        } else {
            format!("{}...", &filename[..max_len.saturating_sub(3)])
        }
    }
}
