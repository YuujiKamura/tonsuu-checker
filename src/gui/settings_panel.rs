//! Settings panel for tonsuu-checker GUI

use eframe::egui::{self, Color32, RichText, Ui};
use tonsuu_checker::config::Config;

/// Available AI backends
const BACKENDS: &[&str] = &["gemini", "claude", "codex"];

/// Preset models for each backend
const GEMINI_MODELS: &[&str] = &["gemini-2.5-pro-preview-06-05"];
const CLAUDE_MODELS: &[&str] = &["claude-opus-4-20250514"];
const CODEX_MODELS: &[&str] = &["codex-5.2"];

/// Settings panel
pub struct SettingsPanel {
    /// Backend selection
    selected_backend: String,
    /// Model input (can be custom)
    model_input: String,
    /// Whether config was modified
    modified: bool,
    /// Status message
    status_message: Option<(String, bool)>, // (message, is_error)
}

impl SettingsPanel {
    pub fn new(config: &Config) -> Self {
        Self {
            selected_backend: config.backend.clone(),
            model_input: config.model.clone().unwrap_or_default(),
            modified: false,
            status_message: None,
        }
    }

    pub fn ui(&mut self, ui: &mut Ui, config: &mut Config) {
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
    }

    fn save_config(&mut self, config: &mut Config) {
        config.backend = self.selected_backend.clone();
        config.model = if self.model_input.is_empty() {
            None
        } else {
            Some(self.model_input.clone())
        };

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
