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
            .with_inner_size([1000.0, 700.0])
            .with_min_inner_size([800.0, 500.0]),
        ..Default::default()
    };

    eframe::run_native(
        "トン数チェッカー",
        options,
        Box::new(|cc| Ok(Box::new(TonsuuApp::new(cc)))),
    )
}
