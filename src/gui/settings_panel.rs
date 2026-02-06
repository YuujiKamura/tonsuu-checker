//! Settings panel for tonsuu-checker GUI

use std::path::PathBuf;

use eframe::egui::{self, Color32, RichText, Ui};
use tonsuu_checker::config::Config;
use tonsuu_checker::infrastructure::legacy_importer::{
    import_legacy_data, load_legacy_export, summarize_legacy_export, ImportMode,
};
use tonsuu_checker::store::Store;

/// Available AI backends
const BACKENDS: &[&str] = &["gemini", "claude", "codex"];

/// Available usage modes (value, display label)
const USAGE_MODES: &[(&str, &str)] = &[
    ("time_based_quota", "時間ベース使用量制限"),
    ("pay_per_use", "従量課金"),
];

/// Preset models for each backend
const GEMINI_MODELS: &[&str] = &["gemini-2.5-pro-preview-06-05"];
const CLAUDE_MODELS: &[&str] = &["claude-opus-4-20250514"];
const CODEX_MODELS: &[&str] = &["codex-5.2"];

/// Import dialog state
#[derive(Debug, Clone)]
pub struct ImportDialogState {
    /// Selected JSON file path
    pub file_path: PathBuf,
    /// Preview summary
    pub preview_summary: String,
    /// Number of stock items in the file
    pub stock_count: usize,
    /// Selected import mode
    pub import_mode: ImportMode,
    /// Error message if loading failed
    pub error: Option<String>,
}

/// Settings panel
pub struct SettingsPanel {
    /// Backend selection
    selected_backend: String,
    /// Model input (can be custom)
    model_input: String,
    /// Usage mode selection
    selected_usage_mode: String,
    /// Whether config was modified
    modified: bool,
    /// Status message
    status_message: Option<(String, bool)>, // (message, is_error)
    /// Import dialog state (Some when dialog is open)
    import_dialog: Option<ImportDialogState>,
}

impl SettingsPanel {
    pub fn new(config: &Config) -> Self {
        Self {
            selected_backend: config.backend.clone(),
            model_input: config.model.clone().unwrap_or_default(),
            selected_usage_mode: config.usage_mode.clone(),
            modified: false,
            status_message: None,
            import_dialog: None,
        }
    }

    pub fn ui(&mut self, ui: &mut Ui, config: &mut Config, store: &mut Store) {
        egui::ScrollArea::vertical().show(ui, |ui| {
        ui.heading("設定");
        ui.add_space(10.0);

        // Backend selection
        ui.label(RichText::new("AIバックエンド").strong());
        ui.add_space(5.0);

        ui.horizontal(|ui| {
            for backend in BACKENDS {
                let selected = self.selected_backend == *backend;
                if ui.selectable_label(selected, *backend).clicked() {
                    self.selected_backend = backend.to_string();
                    self.modified = true;
                    // Clear model when backend changes
                    self.model_input.clear();
                }
            }
        });

        ui.add_space(15.0);
        ui.separator();
        ui.add_space(15.0);

        // Usage mode selection
        ui.label(RichText::new("課金モデル").strong());
        ui.add_space(5.0);

        ui.horizontal(|ui| {
            for &(value, label) in USAGE_MODES {
                let selected = self.selected_usage_mode == value;
                if ui.selectable_label(selected, label).clicked() {
                    self.selected_usage_mode = value.to_string();
                    self.modified = true;
                }
            }
        });

        ui.add_space(15.0);
        ui.separator();
        ui.add_space(15.0);

        // Model selection
        ui.label(RichText::new("モデル").strong());
        ui.add_space(5.0);

        // Preset models based on backend
        let presets = match self.selected_backend.as_str() {
            "gemini" => GEMINI_MODELS,
            "claude" => CLAUDE_MODELS,
            "codex" => CODEX_MODELS,
            _ => &[],
        };

        if !presets.is_empty() {
            ui.label("プリセット:");
            ui.horizontal_wrapped(|ui| {
                for model in presets {
                    if ui.small_button(*model).clicked() {
                        self.model_input = model.to_string();
                        self.modified = true;
                    }
                }
            });
            ui.add_space(5.0);
        }

        // Custom model input
        ui.horizontal(|ui| {
            ui.label("カスタム:");
            let response = ui.text_edit_singleline(&mut self.model_input);
            if response.changed() {
                self.modified = true;
            }
            if ui.button("クリア").clicked() {
                self.model_input.clear();
                self.modified = true;
            }
        });

        ui.add_space(5.0);
        ui.label(
            RichText::new("※ 空欄の場合はデフォルトモデルを使用")
                .color(Color32::GRAY)
                .small(),
        );

        ui.add_space(20.0);
        ui.separator();
        ui.add_space(15.0);

        // Current config display
        ui.label(RichText::new("現在の設定").strong());
        ui.add_space(5.0);

        egui::Frame::new()
            .fill(Color32::from_gray(30))
            .inner_margin(10.0)
            .corner_radius(4.0)
            .show(ui, |ui| {
                egui::Grid::new("current_config")
                    .num_columns(2)
                    .spacing([20.0, 6.0])
                    .show(ui, |ui| {
                        ui.label("バックエンド:");
                        ui.label(&config.backend);
                        ui.end_row();

                        ui.label("モデル:");
                        ui.label(config.model.as_deref().unwrap_or("(デフォルト)"));
                        ui.end_row();

                        ui.label("キャッシュ:");
                        ui.label(if config.cache_enabled { "有効" } else { "無効" });
                        ui.end_row();

                        ui.label("アンサンブル数:");
                        ui.label(format!("{}", config.ensemble_count));
                        ui.end_row();

                        ui.label("課金モデル:");
                        let usage_mode_display = USAGE_MODES.iter()
                            .find(|(v, _)| *v == config.usage_mode.as_str())
                            .map(|(_, l)| *l)
                            .unwrap_or("時間ベース使用量制限");
                        ui.label(usage_mode_display);
                        ui.end_row();
                    });
            });

        ui.add_space(20.0);

        // Save button
        ui.horizontal(|ui| {
            let save_enabled = self.modified;
            if ui.add_enabled(save_enabled, egui::Button::new(
                RichText::new("💾 保存").size(16.0)
            )).clicked() {
                self.save_config(config);
            }

            if ui.button("リセット").clicked() {
                self.selected_backend = config.backend.clone();
                self.model_input = config.model.clone().unwrap_or_default();
                self.selected_usage_mode = config.usage_mode.clone();
                self.modified = false;
                self.status_message = None;
            }

            if self.modified {
                ui.label(RichText::new("* 未保存の変更があります").color(Color32::YELLOW));
            }
        });

        // Status message
        if let Some((ref msg, is_error)) = self.status_message {
            ui.add_space(10.0);
            let color = if is_error { Color32::LIGHT_RED } else { Color32::LIGHT_GREEN };
            ui.label(RichText::new(msg).color(color));
        }

        ui.add_space(20.0);
        ui.separator();
        ui.add_space(15.0);

        // JSON Import section
        self.render_import_section(ui, store);
        }); // End ScrollArea
    }

    /// Render the JSON import section
    fn render_import_section(&mut self, ui: &mut Ui, store: &mut Store) {
        ui.label(RichText::new("データインポート").strong());
        ui.add_space(5.0);

        ui.label(
            RichText::new("旧バージョン (TonSuuChecker_local) からJSONバックアップをインポート")
                .color(Color32::GRAY)
                .small(),
        );
        ui.add_space(10.0);

        // Import button
        if ui
            .button(RichText::new("JSONファイルを選択...").size(14.0))
            .clicked()
        {
            self.open_file_dialog();
        }

        // Handle import dialog
        if self.import_dialog.is_some() {
            self.render_import_dialog(ui, store);
        }
    }

    /// Open file dialog to select JSON file
    fn open_file_dialog(&mut self) {
        let file = rfd::FileDialog::new()
            .add_filter("JSON", &["json"])
            .set_title("JSONバックアップファイルを選択")
            .pick_file();

        if let Some(path) = file {
            // Try to load and parse the file for preview
            match load_legacy_export(&path) {
                Ok(data) => {
                    let summary = summarize_legacy_export(&data);
                    let stock_count = data.stock.len();
                    self.import_dialog = Some(ImportDialogState {
                        file_path: path,
                        preview_summary: summary,
                        stock_count,
                        import_mode: ImportMode::Append,
                        error: None,
                    });
                }
                Err(e) => {
                    self.import_dialog = Some(ImportDialogState {
                        file_path: path,
                        preview_summary: String::new(),
                        stock_count: 0,
                        import_mode: ImportMode::Append,
                        error: Some(format!("ファイル読み込みエラー: {}", e)),
                    });
                }
            }
        }
    }

    /// Render the import dialog
    fn render_import_dialog(&mut self, ui: &mut Ui, store: &mut Store) {
        // Clone the dialog state to avoid borrow issues
        let dialog = self.import_dialog.clone().unwrap();
        let mut should_close = false;
        let mut should_import = false;
        let mut new_import_mode = dialog.import_mode;

        ui.add_space(15.0);

        egui::Frame::new()
            .fill(Color32::from_gray(40))
            .inner_margin(15.0)
            .corner_radius(8.0)
            .show(ui, |ui| {
                ui.label(RichText::new("インポート設定").strong().size(16.0));
                ui.add_space(10.0);

                // Show file path
                ui.horizontal(|ui| {
                    ui.label("ファイル:");
                    ui.label(
                        RichText::new(dialog.file_path.display().to_string())
                            .color(Color32::LIGHT_BLUE)
                            .small(),
                    );
                });

                ui.add_space(10.0);

                // Show error or preview
                if let Some(ref error) = dialog.error {
                    ui.label(RichText::new(error).color(Color32::LIGHT_RED));
                } else {
                    // Preview summary
                    ui.label(RichText::new("プレビュー:").strong());
                    egui::ScrollArea::vertical()
                        .max_height(150.0)
                        .show(ui, |ui| {
                            ui.label(
                                RichText::new(&dialog.preview_summary)
                                    .monospace()
                                    .small(),
                            );
                        });

                    ui.add_space(15.0);

                    // Import mode selection
                    ui.label(RichText::new("インポートモード:").strong());
                    ui.add_space(5.0);

                    ui.horizontal(|ui| {
                        if ui
                            .selectable_label(new_import_mode == ImportMode::Append, "追加 (Append)")
                            .clicked()
                        {
                            new_import_mode = ImportMode::Append;
                        }
                        ui.label(
                            RichText::new("(既存データを保持し、新規のみ追加)")
                                .color(Color32::GRAY)
                                .small(),
                        );
                    });

                    ui.horizontal(|ui| {
                        if ui
                            .selectable_label(
                                new_import_mode == ImportMode::Refresh,
                                "リフレッシュ",
                            )
                            .clicked()
                        {
                            new_import_mode = ImportMode::Refresh;
                        }
                        ui.label(
                            RichText::new("(既存データを削除し、完全に置き換え)")
                                .color(Color32::GRAY)
                                .small(),
                        );
                    });

                    if new_import_mode == ImportMode::Refresh {
                        ui.add_space(5.0);
                        ui.label(
                            RichText::new("警告: 既存の履歴データがすべて削除されます")
                                .color(Color32::YELLOW),
                        );
                    }
                }

                ui.add_space(15.0);

                // Action buttons
                ui.horizontal(|ui| {
                    let can_import = dialog.error.is_none() && dialog.stock_count > 0;

                    if ui
                        .add_enabled(can_import, egui::Button::new("インポート実行"))
                        .clicked()
                    {
                        should_import = true;
                    }

                    if ui.button("キャンセル").clicked() {
                        should_close = true;
                    }
                });
            });

        // Update import mode if changed
        if new_import_mode != dialog.import_mode {
            if let Some(ref mut d) = self.import_dialog {
                d.import_mode = new_import_mode;
            }
        }

        // Handle import action
        if should_import {
            // Re-read the file and execute import
            match load_legacy_export(&dialog.file_path) {
                Ok(export_data) => {
                    let result = import_legacy_data(&export_data, store, new_import_mode);

                    if result.is_success() {
                        let cleared_msg = if result.cleared > 0 {
                            format!(", {} 件削除", result.cleared)
                        } else {
                            String::new()
                        };
                        self.status_message = Some((
                            format!(
                                "インポート完了: {} 件追加, {} 件スキップ{}",
                                result.history_imported, result.skipped, cleared_msg
                            ),
                            false,
                        ));
                    } else {
                        self.status_message = Some((
                            format!(
                                "インポート完了 (エラーあり): {} 件追加, {} 件スキップ, エラー: {:?}",
                                result.history_imported, result.skipped, result.errors
                            ),
                            true,
                        ));
                    }
                }
                Err(e) => {
                    self.status_message = Some((
                        format!("インポートエラー: {}", e),
                        true,
                    ));
                }
            }
            self.import_dialog = None;
        } else if should_close {
            self.import_dialog = None;
        }
    }

    fn save_config(&mut self, config: &mut Config) {
        config.backend = self.selected_backend.clone();
        config.model = if self.model_input.is_empty() {
            None
        } else {
            Some(self.model_input.clone())
        };
        config.usage_mode = self.selected_usage_mode.clone();

        match config.save() {
            Ok(()) => {
                self.modified = false;
                self.status_message = Some(("設定を保存しました".to_string(), false));
            }
            Err(e) => {
                self.status_message = Some((format!("保存エラー: {}", e), true));
            }
        }
    }
}
