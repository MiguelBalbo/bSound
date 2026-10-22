mod app;
mod audio;
mod config;
mod devices;
mod ui;

use std::sync::Arc;

use eframe::egui::{self, FontData, FontDefinitions, FontFamily};

fn fonts() -> FontDefinitions {
    let mut fonts = FontDefinitions::default();
    for (name, bytes) in [
        ("inter", &include_bytes!("../assets/fonts/Inter-Regular.ttf")[..]),
        ("inter-semibold", &include_bytes!("../assets/fonts/Inter-SemiBold.ttf")[..]),
        ("inter-bold", &include_bytes!("../assets/fonts/Inter-Bold.ttf")[..]),
    ] {
        fonts.font_data.insert(name.into(), Arc::new(FontData::from_static(bytes)));
    }
    // Inter na frente; as fontes padrão continuam como fallback para emoji e símbolos.
    fonts.families.entry(FontFamily::Proportional).or_default().insert(0, "inter".into());
    let fallback = fonts.families[&FontFamily::Proportional].clone();
    for (family, first) in [("bold", "inter-bold"), ("semibold", "inter-semibold")] {
        let mut list = vec![first.to_string()];
        list.extend(fallback.iter().cloned());
        fonts.families.insert(FontFamily::Name(family.into()), list);
    }
    fonts
}

fn main() -> eframe::Result<()> {
    env_logger::init();
    let icon = eframe::icon_data::from_png_bytes(include_bytes!("../assets/icon-256.png")).expect("ícone inválido");
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("bSound")
            .with_app_id("bsound")
            .with_icon(Arc::new(icon))
            .with_inner_size([960.0, 760.0])
            .with_min_inner_size([560.0, 460.0])
            .with_drag_and_drop(true),
        ..Default::default()
    };
    eframe::run_native(
        "bSound",
        options,
        Box::new(|cc| {
            cc.egui_ctx.set_fonts(fonts());
            for theme in [egui::Theme::Dark, egui::Theme::Light] {
                cc.egui_ctx.style_mut_of(theme, |style| {
                    style.visuals.selection.bg_fill = ui::logo::VIOLET;
                    style.visuals.hyperlink_color = ui::logo::VIOLET;
                });
            }
            Ok(Box::new(app::SoundboardApp::new(cc)))
        }),
    )
}
