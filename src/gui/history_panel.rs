//! History panel for viewing and managing analysis history

use eframe::egui::{self, Color32, ColorImage, RichText, ScrollArea, TextureHandle, Vec2};
use tonsuu_checker::store::Store;

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
        }
    }

    /// Load image from path and create texture
    fn load_preview_texture(
        &mut self,
        ctx: &egui::Context,
        image_path: &str,
    ) -> Option<&TextureHandle> {
        // Check if already loaded
        if self.preview_path.as_deref() == Some(image_path) {
            return self.preview_texture.as_ref();
        }

        // Check if file exists
        let path = std::path::Path::new(image_path);
        if !path.exists() {
            self.preview_texture = None;
            self.preview_path = Some(image_path.to_string());
            return None;
        }

        // Load image using image crate
        match image::open(path) {
            Ok(img) => {
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
                self.preview_path = Some(image_path.to_string());
                self.preview_texture.as_ref()
            }
            Err(e) => {
                eprintln!("画像読み込みエラー: {} - {}", image_path, e);
                self.preview_texture = None;
                self.preview_path = Some(image_path.to_string());
                None
            }
        }
    }

    /// Calculate scaled size to fit within max dimensions while preserving aspect ratio
    fn calc_preview_size(texture: &TextureHandle, max_width: f32, max_height: f32) -> Vec2 {
        let original_size = texture.size_vec2();
        let scale_x = max_width / original_size.x;
        let scale_y = max_height / original_size.y;
        let scale = scale_x.min(scale_y).min(1.0); // Don't upscale
        Vec2::new(original_size.x * scale, original_size.y * scale)
    }

    /// Render the panel UI
    pub fn ui(&mut self, ui: &mut egui::Ui, store: &mut Store) {
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

        // Table header
        let available_width = ui.available_width();
        let col_widths = TableColumnWidths::new(available_width);

        ui.horizontal(|ui| {
            ui.add_space(4.0);
            ui.add_sized(
                Vec2::new(col_widths.image, 20.0),
                egui::Label::new(RichText::new("画像").strong()),
            );
            ui.add_sized(
                Vec2::new(col_widths.estimated, 20.0),
                egui::Label::new(RichText::new("推定(t)").strong()),
            );
            ui.add_sized(
                Vec2::new(col_widths.actual, 20.0),
                egui::Label::new(RichText::new("実測(t)").strong()),
            );
            ui.add_sized(
                Vec2::new(col_widths.error, 20.0),
                egui::Label::new(RichText::new("誤差(t)").strong()),
            );
            ui.add_sized(
                Vec2::new(col_widths.datetime, 20.0),
                egui::Label::new(RichText::new("日時").strong()),
            );
        });

        ui.separator();

        // Scrollable table body
        ScrollArea::vertical()
            .max_height(ui.available_height() - 120.0)
            .show(ui, |ui| {
                for entry in &entries {
                    let is_selected = self
                        .selected_hash
                        .as_ref()
                        .map_or(false, |h| h == &entry.image_hash);

                    let hash = entry.image_hash.clone();

                    // Create a clickable row
                    let response = ui
                        .horizontal(|ui| {
                            // Background color based on selection/hover
                            let rect = ui.available_rect_before_wrap();
                            let row_rect = egui::Rect::from_min_size(
                                rect.min,
                                Vec2::new(ui.available_width(), 26.0),
                            );

                            // Handle interaction first
                            let response = ui.allocate_rect(row_rect, egui::Sense::click());

                            // Draw background
                            if is_selected {
                                ui.painter().rect_filled(
                                    row_rect,
                                    2.0,
                                    Color32::from_rgba_unmultiplied(100, 149, 237, 50),
                                );
                            } else if response.hovered() {
                                ui.painter().rect_filled(
                                    row_rect,
                                    2.0,
                                    Color32::from_rgba_unmultiplied(128, 128, 128, 30),
                                );
                            }

                            // Draw content
                            ui.allocate_new_ui(egui::UiBuilder::new().max_rect(row_rect), |ui| {
                                ui.horizontal_centered(|ui| {
                                    ui.add_space(4.0);

                                    // Image filename (truncated)
                                    let filename = truncate_filename(&entry.image_path, 30);
                                    ui.add_sized(
                                        Vec2::new(col_widths.image, 20.0),
                                        egui::Label::new(filename).truncate(),
                                    );

                                    // Estimated tonnage
                                    ui.add_sized(
                                        Vec2::new(col_widths.estimated, 20.0),
                                        egui::Label::new(format!(
                                            "{:.2}",
                                            entry.estimation.estimated_tonnage
                                        )),
                                    );

                                    // Actual tonnage
                                    let actual_text = entry
                                        .actual_tonnage
                                        .map_or("-".to_string(), |t| format!("{:.2}", t));
                                    ui.add_sized(
                                        Vec2::new(col_widths.actual, 20.0),
                                        egui::Label::new(actual_text),
                                    );

                                    // Error (with color coding)
                                    let (error_text, error_color) =
                                        if let Some(actual) = entry.actual_tonnage {
                                            let error = entry.estimation.estimated_tonnage - actual;
                                            let color = if error.abs() < 0.5 {
                                                Color32::from_rgb(100, 200, 100)
                                            } else if error.abs() < 1.0 {
                                                Color32::from_rgb(200, 200, 100)
                                            } else {
                                                Color32::from_rgb(255, 100, 100)
                                            };
                                            (format!("{:+.2}", error), color)
                                        } else {
                                            ("-".to_string(), Color32::GRAY)
                                        };
                                    ui.add_sized(
                                        Vec2::new(col_widths.error, 20.0),
                                        egui::Label::new(
                                            RichText::new(error_text).color(error_color),
                                        ),
                                    );

                                    // Date/time
                                    let datetime =
                                        entry.analyzed_at.format("%Y/%m/%d %H:%M").to_string();
                                    ui.add_sized(
                                        Vec2::new(col_widths.datetime, 20.0),
                                        egui::Label::new(datetime),
                                    );
                                });
                            });

                            response
                        })
                        .inner;

                    // Handle click to select row
                    if response.clicked() {
                        self.selected_hash = Some(hash.clone());
                        // Pre-fill feedback input with actual tonnage if exists
                        self.feedback_input = entry
                            .actual_tonnage
                            .map_or(String::new(), |t| format!("{:.2}", t));
                    }
                }
            });

        ui.separator();

        // Bottom: Feedback section
        let ctx = ui.ctx().clone();
        self.render_feedback_section(ui, store, &ctx);
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
                        self.load_preview_texture(ctx, &image_path);

                        if let Some(ref texture) = self.preview_texture {
                            let preview_size = Self::calc_preview_size(texture, 280.0, 220.0);
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
                                "上面積:  {} m²",
                                estimation
                                    .upper_area
                                    .map_or("-".to_string(), |v| format!("{:.2}", v))
                            ))
                            .font(mono_font.clone()),
                        );
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
                                "空隙率:  {}%",
                                estimation
                                    .void_ratio
                                    .map_or("-".to_string(), |v| format!("{:.0}", v * 100.0))
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

/// Column widths for the history table
struct TableColumnWidths {
    image: f32,
    estimated: f32,
    actual: f32,
    error: f32,
    datetime: f32,
}

impl TableColumnWidths {
    fn new(available_width: f32) -> Self {
        // Distribute width proportionally
        // image:estimated:actual:error:datetime = 3:1:1:1:2
        let total_ratio = 3.0 + 1.0 + 1.0 + 1.0 + 2.0;
        let unit = (available_width - 20.0) / total_ratio; // -20 for padding

        Self {
            image: unit * 3.0,
            estimated: unit * 1.0,
            actual: unit * 1.0,
            error: unit * 1.0,
            datetime: unit * 2.0,
        }
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
