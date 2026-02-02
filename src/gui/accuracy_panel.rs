//! Accuracy panel for viewing estimation accuracy statistics

use eframe::egui::{self, Color32, RichText, Ui};
use tonsuu_checker::store::{AccuracySample, AccuracyStats, Store};

/// Panel for viewing accuracy statistics
pub struct AccuracyPanel {
    /// Group statistics by truck type
    group_by_truck: bool,
    /// Group statistics by material type
    group_by_material: bool,
    /// Show detailed sample table
    show_detailed: bool,
}

impl AccuracyPanel {
    /// Create a new accuracy panel
    pub fn new() -> Self {
        Self {
            group_by_truck: false,
            group_by_material: false,
            show_detailed: false,
        }
    }

    /// Render the panel UI
    pub fn ui(&mut self, ui: &mut Ui, store: &Store) {
        ui.heading("精度統計");
        ui.separator();

        let stats = store.accuracy_stats();

        // Check if there's any feedback data
        if stats.sample_count == 0 {
            ui.add_space(20.0);
            ui.vertical_centered(|ui| {
                ui.label(
                    RichText::new("フィードバックデータがありません")
                        .size(16.0)
                        .color(Color32::GRAY),
                );
                ui.add_space(8.0);
                ui.label("履歴タブで実測値を登録してください");
            });
            return;
        }

        // Overall statistics section
        ui.add_space(8.0);
        show_stats(ui, "全体統計", &stats);

        ui.add_space(16.0);
        ui.separator();

        // Toggle checkboxes
        ui.add_space(8.0);
        ui.horizontal(|ui| {
            ui.checkbox(&mut self.group_by_truck, "車種別");
            ui.add_space(16.0);
            ui.checkbox(&mut self.group_by_material, "材料別");
            ui.add_space(16.0);
            ui.checkbox(&mut self.show_detailed, "詳細表示");
        });

        ui.add_space(8.0);
        ui.separator();

        // Grouped statistics
        egui::ScrollArea::vertical()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                // Group by truck type
                if self.group_by_truck {
                    ui.add_space(8.0);
                    ui.heading("車種別統計");
                    ui.add_space(4.0);

                    let by_truck = stats.by_truck_type();
                    let mut truck_types: Vec<_> = by_truck.keys().collect();
                    truck_types.sort();

                    for truck_type in truck_types {
                        if let Some(truck_stats) = by_truck.get(truck_type) {
                            show_stats_compact(ui, truck_type, truck_stats);
                            ui.add_space(8.0);
                        }
                    }

                    ui.separator();
                }

                // Group by material type
                if self.group_by_material {
                    ui.add_space(8.0);
                    ui.heading("材料別統計");
                    ui.add_space(4.0);

                    let by_material = stats.by_material_type();
                    let mut material_types: Vec<_> = by_material.keys().collect();
                    material_types.sort();

                    for material_type in material_types {
                        if let Some(material_stats) = by_material.get(material_type) {
                            show_stats_compact(ui, material_type, material_stats);
                            ui.add_space(8.0);
                        }
                    }

                    ui.separator();
                }

                // Detailed sample table
                if self.show_detailed {
                    ui.add_space(8.0);
                    ui.heading("詳細データ");
                    ui.add_space(4.0);

                    show_sample_table(ui, &stats.samples);
                }
            });
    }
}

impl Default for AccuracyPanel {
    fn default() -> Self {
        Self::new()
    }
}

/// Display full accuracy statistics with a heading
fn show_stats(ui: &mut Ui, label: &str, stats: &AccuracyStats) {
    ui.heading(format!("{} (n={})", label, stats.sample_count));
    ui.add_space(4.0);

    egui::Grid::new(format!("stats_grid_{}", label))
        .num_columns(4)
        .spacing([20.0, 4.0])
        .show(ui, |ui| {
            // Row 1: Mean error and MAE
            ui.label("平均誤差:");
            ui.label(format_error(stats.mean_error, "t"));
            ui.label("平均絶対誤差:");
            ui.label(format_abs_error(stats.mean_abs_error, "t"));
            ui.end_row();

            // Row 2: RMSE and Mean % error
            ui.label("RMSE:");
            ui.label(format_abs_error(stats.rmse, "t"));
            ui.label("平均%誤差:");
            ui.label(format_percent_error(stats.mean_percent_error));
            ui.end_row();

            // Row 3: Min/Max error
            ui.label("最小誤差:");
            ui.label(format_error(stats.min_error, "t"));
            ui.label("最大誤差:");
            ui.label(format_error(stats.max_error, "t"));
            ui.end_row();
        });
}

/// Display compact statistics for grouped data
fn show_stats_compact(ui: &mut Ui, label: &str, stats: &AccuracyStats) {
    ui.horizontal(|ui| {
        ui.label(RichText::new(format!("{} (n={})", label, stats.sample_count)).strong());
    });

    egui::Grid::new(format!("compact_stats_grid_{}", label))
        .num_columns(6)
        .spacing([12.0, 2.0])
        .show(ui, |ui| {
            ui.label("平均誤差:");
            ui.label(format_error(stats.mean_error, "t"));
            ui.label("MAE:");
            ui.label(format_abs_error(stats.mean_abs_error, "t"));
            ui.label("%誤差:");
            ui.label(format_percent_error(stats.mean_percent_error));
            ui.end_row();
        });
}

/// Display detailed sample table
fn show_sample_table(ui: &mut Ui, samples: &[AccuracySample]) {
    egui::Grid::new("sample_table")
        .num_columns(6)
        .spacing([12.0, 4.0])
        .striped(true)
        .show(ui, |ui| {
            // Header row
            ui.label(RichText::new("推定(t)").strong());
            ui.label(RichText::new("実測(t)").strong());
            ui.label(RichText::new("誤差(t)").strong());
            ui.label(RichText::new("誤差%").strong());
            ui.label(RichText::new("車種").strong());
            ui.label(RichText::new("材料").strong());
            ui.end_row();

            // Data rows
            for sample in samples {
                ui.label(format!("{:.2}", sample.estimated));
                ui.label(format!("{:.2}", sample.actual));
                ui.label(format_error(sample.error(), ""));
                ui.label(format_percent_error(sample.percent_error().abs()));
                ui.label(&sample.truck_type);
                ui.label(&sample.material_type);
                ui.end_row();
            }
        });
}

/// Format error value with color coding
fn format_error(error: f64, unit: &str) -> RichText {
    let color = error_color(error.abs());
    let text = if unit.is_empty() {
        format!("{:+.3}", error)
    } else {
        format!("{:+.3} {}", error, unit)
    };
    RichText::new(text).color(color)
}

/// Format absolute error value with color coding
fn format_abs_error(error: f64, unit: &str) -> RichText {
    let color = error_color(error);
    let text = if unit.is_empty() {
        format!("{:.3}", error)
    } else {
        format!("{:.3} {}", error, unit)
    };
    RichText::new(text).color(color)
}

/// Format percent error with color coding
fn format_percent_error(percent: f64) -> RichText {
    let color = percent_error_color(percent);
    RichText::new(format!("{:.1}%", percent)).color(color)
}

/// Get color for error value (in tonnes)
/// Green for good accuracy (< 0.5t), yellow for moderate (0.5-1t), red for poor (> 1t)
fn error_color(abs_error: f64) -> Color32 {
    if abs_error < 0.5 {
        Color32::from_rgb(100, 200, 100) // Green - good
    } else if abs_error < 1.0 {
        Color32::from_rgb(220, 180, 50) // Yellow - moderate
    } else {
        Color32::from_rgb(220, 100, 100) // Red - poor
    }
}

/// Get color for percent error
/// Green for < 5%, yellow for 5-10%, red for > 10%
fn percent_error_color(percent: f64) -> Color32 {
    if percent < 5.0 {
        Color32::from_rgb(100, 200, 100) // Green - good
    } else if percent < 10.0 {
        Color32::from_rgb(220, 180, 50) // Yellow - moderate
    } else {
        Color32::from_rgb(220, 100, 100) // Red - poor
    }
}
