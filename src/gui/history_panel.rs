//! History panel for viewing and managing analysis history

use eframe::egui::{self, Color32, RichText, ScrollArea, Vec2};
use tonsuu_checker::store::Store;

/// Panel for viewing analysis history and providing feedback
pub struct HistoryPanel {
    /// Currently selected entry hash
    selected_hash: Option<String>,
    /// Input field for actual tonnage feedback
    feedback_input: String,
    /// Toggle to show only entries with feedback
    show_only_with_feedback: bool,
}

impl HistoryPanel {
    /// Create a new history panel
    pub fn new() -> Self {
        Self {
            selected_hash: None,
            feedback_input: String::new(),
            show_only_with_feedback: false,
        }
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
        self.render_feedback_section(ui, store);
    }

    /// Render the feedback input section
    fn render_feedback_section(&mut self, ui: &mut egui::Ui, store: &mut Store) {
        if let Some(ref selected_hash) = self.selected_hash.clone() {
            if let Some(entry) = store.get_by_hash(selected_hash) {
                let filename = truncate_filename(&entry.image_path, 50);
                let image_path = entry.image_path.clone();

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

                // Show entry details in a collapsible section
                ui.add_space(8.0);
                ui.collapsing("詳細情報", |ui| {
                    if let Some(entry) = store.get_by_hash(selected_hash) {
                        egui::Grid::new("entry_details")
                            .num_columns(2)
                            .spacing([20.0, 4.0])
                            .show(ui, |ui| {
                                ui.label("車両タイプ:");
                                ui.label(&entry.estimation.truck_type);
                                ui.end_row();

                                ui.label("積載物:");
                                ui.label(&entry.estimation.material_type);
                                ui.end_row();

                                ui.label("推定体積:");
                                ui.label(format!("{:.2} m3", entry.estimation.estimated_volume_m3));
                                ui.end_row();

                                ui.label("信頼度:");
                                ui.label(format!(
                                    "{:.0}%",
                                    entry.estimation.confidence_score * 100.0
                                ));
                                ui.end_row();

                                if let Some(actual) = entry.actual_tonnage {
                                    ui.label("実測トン数:");
                                    ui.label(format!("{:.2} t", actual));
                                    ui.end_row();

                                    let error = entry.estimation.estimated_tonnage - actual;
                                    let error_pct = if actual > 0.0 {
                                        (error / actual) * 100.0
                                    } else {
                                        0.0
                                    };
                                    ui.label("誤差:");
                                    ui.label(format!("{:+.2} t ({:+.1}%)", error, error_pct));
                                    ui.end_row();
                                }

                                if let Some(ref feedback_at) = entry.feedback_at {
                                    ui.label("フィードバック日時:");
                                    ui.label(feedback_at.format("%Y/%m/%d %H:%M").to_string());
                                    ui.end_row();
                                }

                                ui.label("解析日時:");
                                ui.label(entry.analyzed_at.format("%Y/%m/%d %H:%M:%S").to_string());
                                ui.end_row();
                            });

                        // Show reasoning in expandable section
                        if !entry.estimation.reasoning.is_empty() {
                            ui.add_space(8.0);
                            ui.collapsing("AIの推定根拠", |ui| {
                                ScrollArea::vertical().max_height(150.0).show(ui, |ui| {
                                    ui.label(&entry.estimation.reasoning);
                                });
                            });
                        }
                    }
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
