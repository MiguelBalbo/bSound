mod app;
mod audio;
mod config;
mod devices;
mod stats;
mod tray;
mod ui;

use std::sync::Arc;

use eframe::egui;

fn main() -> eframe::Result<()> {
    env_logger::init();
    let icon = eframe::icon_data::from_png_bytes(include_bytes!("../assets/icon-256.png")).expect("ícone inválido");
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("bSound")
            .with_app_id("bsound")
            .with_icon(Arc::new(icon))
            .with_inner_size([1200.0, 800.0])
            .with_min_inner_size([920.0, 620.0])
            // Barra de título transparente: os botões do macOS ficam sobre a barra lateral.
            .with_fullsize_content_view(true)
            .with_titlebar_shown(false)
            .with_title_shown(false)
            .with_drag_and_drop(true),
        ..Default::default()
    };
    eframe::run_native(
        "bSound",
        options,
        Box::new(|cc| {
            Ok(Box::new(app::SoundboardApp::new(cc)))
        }),
    )
}
