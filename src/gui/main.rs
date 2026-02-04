//! GUI entry point for Tonsuu Checker

mod app;
mod analyze_panel;
mod history_panel;
mod accuracy_panel;
mod settings_panel;
mod vehicle_panel;

use app::TonsuuApp;
use eframe::egui;

fn main() -> eframe::Result<()> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1200.0, 900.0])
            .with_min_inner_size([900.0, 600.0])
            .with_resizable(true),
        ..Default::default()
    };

    eframe::run_native(
        "トン数チェッカー",
        options,
        Box::new(|cc| Ok(Box::new(TonsuuApp::new(cc)))),
    )
}
