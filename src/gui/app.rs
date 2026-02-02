//! Main application structure with tab navigation

use eframe::egui;
use tonsuu_checker::config::Config;
use tonsuu_checker::store::{Store, VehicleStore};

use crate::analyze_panel::AnalyzePanel;
use crate::history_panel::HistoryPanel;
use crate::accuracy_panel::AccuracyPanel;
use crate::settings_panel::SettingsPanel;
use crate::vehicle_panel::VehiclePanel;

/// Application tab selection
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Tab {
    #[default]
    Analyze,
    Vehicle,
    History,
    Accuracy,
    Settings,
}

impl Tab {
    /// Get the Japanese label for this tab
    pub fn label(&self) -> &'static str {
        match self {
            Tab::Analyze => "解析",
            Tab::Vehicle => "車両",
            Tab::History => "履歴",
            Tab::Accuracy => "精度",
            Tab::Settings => "設定",
        }
    }
}

/// Main application state
pub struct TonsuuApp {
    /// Currently selected tab
    current_tab: Tab,
    /// Analyze panel state
    analyze_panel: AnalyzePanel,
    /// Vehicle panel state
    vehicle_panel: VehiclePanel,
    /// History panel state
    history_panel: HistoryPanel,
    /// Accuracy panel state
    accuracy_panel: AccuracyPanel,
    /// Settings panel state
    settings_panel: SettingsPanel,
    /// Application configuration
    config: Config,
    /// Persistent store for history/feedback
    store: Store,
    /// Vehicle store
    vehicle_store: VehicleStore,
}

impl TonsuuApp {
    /// Create a new application instance
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        // Setup Japanese fonts
        let mut fonts = egui::FontDefinitions::default();

        // Try to load system Japanese font
        if let Some(font_data) = Self::load_system_font() {
            fonts.font_data.insert(
                "japanese".to_owned(),
                egui::FontData::from_owned(font_data).into(),
            );

            // Add Japanese font as primary for proportional text
            fonts
                .families
                .entry(egui::FontFamily::Proportional)
                .or_default()
                .insert(0, "japanese".to_owned());

            // Also for monospace
            fonts
                .families
                .entry(egui::FontFamily::Monospace)
                .or_default()
                .insert(0, "japanese".to_owned());
        }

        cc.egui_ctx.set_fonts(fonts);

        // Configure style for better touch/remote responsiveness
        let mut style = (*cc.egui_ctx.style()).clone();
        style.interaction.tooltip_delay = 0.5; // Faster tooltips
        style.animation_time = 0.1; // Faster animations
        cc.egui_ctx.set_style(style);

        // Load configuration
        let config = Config::load().unwrap_or_default();

        // Open the store
        let store_dir = config.store_dir().unwrap_or_else(|_| {
            std::env::temp_dir().join("tonsuu-checker")
        });
        let store = Store::open(store_dir.clone()).unwrap_or_else(|_| {
            // Fallback to temp directory if store fails to open
            let fallback_dir = std::env::temp_dir().join("tonsuu-checker-fallback");
            Store::open(fallback_dir).expect("Failed to create fallback store")
        });

        // Open vehicle store
        let vehicle_store = VehicleStore::open(store_dir.clone()).unwrap_or_else(|_| {
            let fallback_dir = std::env::temp_dir().join("tonsuu-checker-fallback");
            VehicleStore::open(fallback_dir).expect("Failed to create fallback vehicle store")
        });

        let settings_panel = SettingsPanel::new(&config);

        Self {
            current_tab: Tab::default(),
            analyze_panel: AnalyzePanel::new(),
            vehicle_panel: VehiclePanel::new(),
            history_panel: HistoryPanel::new(),
            accuracy_panel: AccuracyPanel::new(),
            settings_panel,
            config,
            store,
            vehicle_store,
        }
    }

    /// Load system Japanese font
    fn load_system_font() -> Option<Vec<u8>> {
        // Windows font paths to try
        let font_paths = [
            "C:/Windows/Fonts/YuGothM.ttc",   // Yu Gothic Medium
            "C:/Windows/Fonts/yugothic.ttf",  // Yu Gothic
            "C:/Windows/Fonts/meiryo.ttc",    // Meiryo
            "C:/Windows/Fonts/msgothic.ttc",  // MS Gothic
        ];

        for path in &font_paths {
            if let Ok(data) = std::fs::read(path) {
                return Some(data);
            }
        }
        None
    }

    /// Render the tab bar
    fn render_tab_bar(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 0.0;

            for tab in [Tab::Analyze, Tab::Vehicle, Tab::History, Tab::Accuracy, Tab::Settings] {
                let selected = self.current_tab == tab;
                if ui.selectable_label(selected, tab.label()).clicked() {
                    self.current_tab = tab;
                }
                ui.add_space(8.0);
            }
        });
    }
}

impl eframe::App for TonsuuApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Top panel with tab bar
        egui::TopBottomPanel::top("tab_bar").show(ctx, |ui| {
            ui.add_space(4.0);
            self.render_tab_bar(ui);
            ui.add_space(4.0);
        });

        // Central panel with selected tab content
        egui::CentralPanel::default().show(ctx, |ui| {
            match self.current_tab {
                Tab::Analyze => {
                    self.analyze_panel.ui(ui, &self.config, &mut self.store);
                }
                Tab::Vehicle => {
                    self.vehicle_panel.ui(ui, &mut self.vehicle_store, &self.config);
                }
                Tab::History => {
                    self.history_panel.ui(ui, &mut self.store, &self.vehicle_store);
                    // Handle pending actions from context menu
                    if let Some(action) = self.history_panel.take_pending_action() {
                        match action {
                            crate::history_panel::ContextAction::ReAnalyze { hash: _, image_path } => {
                                // TODO: Trigger re-analysis via analyze_panel
                                eprintln!("Re-analyze requested for: {}", image_path);
                            }
                            _ => {}
                        }
                    }
                }
                Tab::Accuracy => {
                    self.accuracy_panel.ui(ui, &self.store);
                }
                Tab::Settings => {
                    self.settings_panel.ui(ui, &mut self.config);
                }
            }
        });
    }
}
