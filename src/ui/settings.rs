//! Painel de configurações: aparência e comportamento do ícone na barra de menus/bandeja.

use eframe::egui::{self, Color32};
use egui_phosphor::light as ph;

use crate::config::{Config, ThemeMode, TrayMode};
use crate::platform::TRAY_AREA;
use crate::ui::theme::{self, *};

pub enum Outcome {
    None,
    Changed,
    Close,
}

pub fn show(ctx: &egui::Context, config: &mut Config) -> Outcome {
    let mut outcome = Outcome::None;
    let modal = egui::Modal::new(egui::Id::new("settings"))
        .backdrop_color(Color32::from_black_alpha(170))
        .frame(modal_frame())
        .show(ctx, |ui| {
            ui.set_width(460.0);
            ui.label(eyebrow("Configurações"));
            ui.add_space(8.0);
            ui.label(display("Preferências", 30.0, p().text));
            ui.add_space(26.0);

            section(ui, "Aparência");
            if segmented(ui, &mut config.theme, &[(ThemeMode::System, "Sistema"), (ThemeMode::Light, "Claro"), (ThemeMode::Dark, "Escuro")]) {
                theme::set_mode(ctx, config.theme);
                outcome = Outcome::Changed;
            }
            ui.add_space(8.0);
            hint(ui, "\"Sistema\" acompanha o modo claro/escuro do sistema operacional.");

            ui.add_space(22.0);
            hairline(ui);
            ui.add_space(22.0);

            section(ui, &capitalize(TRAY_AREA));
            if ui.checkbox(&mut config.close_to_tray, format!("Continuar rodando na {TRAY_AREA} ao fechar a janela")).changed() {
                outcome = Outcome::Changed;
            }
            ui.add_space(4.0);
            hint(ui, &format!("Para encerrar, clique com o botão direito no ícone do bSound na {TRAY_AREA} e escolha \"Sair\"."));
            ui.add_space(18.0);
            ui.label(caps("Sons no menu", 9.0, p().text_variant));
            ui.add_space(6.0);
            if segmented(ui, &mut config.tray_mode, &[(TrayMode::MostPlayed, "Mais tocados"), (TrayMode::Pinned, "Selecionados")]) {
                outcome = Outcome::Changed;
            }
            ui.add_space(8.0);
            let tray_hint = match config.tray_mode {
                TrayMode::MostPlayed => "O clique no ícone mostra os 9 sons que você mais toca.".to_string(),
                TrayMode::Pinned => format!("Marque \"Mostrar na {TRAY_AREA}\" no menu de cada pad (botão direito)."),
            };
            hint(ui, &tray_hint);

            ui.add_space(30.0);
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if button(ui, Some(ph::CHECK), "Concluir", Kind::Primary).clicked() {
                    outcome = Outcome::Close;
                }
            });
        });
    if modal.should_close() && matches!(outcome, Outcome::None) {
        outcome = Outcome::Close;
    }
    outcome
}

fn section(ui: &mut egui::Ui, title: &str) {
    ui.label(display(title, 18.0, p().text));
    ui.add_space(10.0);
}

fn hint(ui: &mut egui::Ui, text: &str) {
    ui.label(egui::RichText::new(text).size(12.5).color(p().text_variant));
}

fn capitalize(s: &str) -> String {
    let mut c = s.chars();
    c.next().map(|f| f.to_uppercase().collect::<String>() + c.as_str()).unwrap_or_default()
}
