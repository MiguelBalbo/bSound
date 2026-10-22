//! Wizard de instalação do BlackHole (microfone virtual).

use std::path::PathBuf;
use std::process::Command;

use eframe::egui::{self, Color32, Stroke, Vec2};
use egui_phosphor::light as ph;

use crate::ui::logo;
use crate::ui::theme::{self, *};

use crate::devices;

const DOWNLOAD_URL: &str = "https://existential.audio/blackhole/";

#[derive(PartialEq, Clone, Copy)]
enum Step {
    Intro,
    Install,
    Verify,
    Done,
}

pub struct Wizard {
    step: Step,
    brew: Option<PathBuf>,
    message: Option<String>,
    hide: bool,
}

pub enum WizardOutcome {
    None,
    Rescan,
    UseDevice(String),
    Close { hide: bool },
}

impl Wizard {
    pub fn new() -> Self {
        Self { step: Step::Intro, brew: devices::brew_path(), message: None, hide: false }
    }

    fn brew_command(&self) -> Option<String> {
        self.brew.as_ref().map(|b| format!("{} install --cask blackhole-2ch", b.display()))
    }
}

pub fn show(ctx: &egui::Context, w: &mut Wizard, found: Option<&str>) -> WizardOutcome {
    let mut outcome = WizardOutcome::None;
    // Se o driver já existe, abre direto no último passo.
    if found.is_some() && matches!(w.step, Step::Intro | Step::Verify) {
        w.step = Step::Done;
    }

    let modal = egui::Modal::new(egui::Id::new("wizard"))
        .backdrop_color(Color32::from_black_alpha(190))
        .frame(modal_frame())
        .show(ctx, |ui| {
            ui.set_width(500.0);
            let current = w.step as usize;

            ui.horizontal(|ui| {
                let (rect, _) = ui.allocate_exact_size(Vec2::new(22.0, 24.0), egui::Sense::hover());
                logo::paint_symbol(ui.painter(), rect);
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(caps(&format!("Passo {} de 4", current + 1), 9.0, p().text_variant));
                });
            });
            ui.add_space(14.0);
            // Progresso em quatro segmentos de 2 px.
            let (bar, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 2.0), egui::Sense::hover());
            let seg = (bar.width() - 6.0) / 4.0;
            for k in 0..4 {
                let r = egui::Rect::from_min_size(bar.min + Vec2::new(k as f32 * (seg + 2.0), 0.0), Vec2::new(seg, 2.0));
                ui.painter().rect_filled(r, 0, if k <= current { p().text } else { p().border });
            }
            ui.add_space(28.0);

            match w.step {
                Step::Intro => {
                    ui.label(eyebrow("Microfone virtual"));
                    ui.add_space(8.0);
                    ui.label(display("Sua voz e seus sons, juntos em qualquer app.", 30.0, p().text));
                    ui.add_space(14.0);
                    body(ui, "Para Discord, Zoom, OBS e outros apps receberem seu microfone junto com o soundboard, o macOS precisa de um dispositivo de áudio virtual.");
                    ui.add_space(6.0);
                    body(ui, "O BlackHole 2ch é gratuito e de código aberto. O bSound envia a mistura para ele, e nos outros apps você escolhe \"BlackHole 2ch\" como microfone.");
                    ui.add_space(18.0);
                    ui.checkbox(&mut w.hide, "Não mostrar ao abrir o app");
                    ui.add_space(22.0);
                    let (back, next) = footer(ui, None, "Agora não", Some(ph::ARROW_RIGHT), "Continuar");
                    if back {
                        outcome = WizardOutcome::Close { hide: w.hide };
                    }
                    if next {
                        w.step = Step::Install;
                    }
                }
                Step::Install => {
                    ui.label(eyebrow("Instalação"));
                    ui.add_space(8.0);
                    ui.label(display("Instale o BlackHole 2ch", 30.0, p().text));
                    ui.add_space(18.0);
                    if let Some(cmd) = w.brew_command() {
                        card(ui, |ui| {
                            ui.horizontal(|ui| {
                                ui.label(display("Homebrew", 18.0, p().text));
                                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                    let (r, _) = ui.allocate_exact_size(Vec2::new(96.0, 20.0), egui::Sense::hover());
                                    theme::badge(ui.painter(), ui, r.right_top() - Vec2::new(92.0, 0.0), "Recomendado", p().text, p().bg);
                                });
                            });
                            ui.add_space(6.0);
                            body(ui, "Abre o Terminal com o comando abaixo. Ele pede sua senha de administrador.");
                            ui.add_space(10.0);
                            egui::Frame::default().fill(p().inset).stroke(Stroke::new(1.0_f32, p().border)).inner_margin(10.0).show(ui, |ui| {
                                ui.set_width(ui.available_width());
                                ui.label(egui::RichText::new(&cmd).monospace().size(11.5).color(p().text));
                            });
                            ui.add_space(12.0);
                            ui.horizontal(|ui| {
                                if button(ui, Some(ph::TERMINAL), "Instalar com Homebrew", Kind::Primary).clicked() {
                                    w.message = Some(match open_terminal(&cmd) {
                                        Ok(()) => "Terminal aberto. Conclua a instalação lá e depois clique em \"Já instalei\".".into(),
                                        Err(e) => format!("Não foi possível abrir o Terminal: {e}. Copie o comando e rode manualmente."),
                                    });
                                }
                                if button(ui, Some(ph::COPY), "Copiar", Kind::Secondary).clicked() {
                                    ui.ctx().copy_text(cmd.clone());
                                    w.message = Some("Comando copiado.".into());
                                }
                            });
                        });
                        ui.add_space(10.0);
                    }
                    card(ui, |ui| {
                        ui.label(display("Instalador oficial", 18.0, p().text));
                        ui.add_space(6.0);
                        body(ui, if w.brew.is_some() { "Prefere o instalador .pkg? Baixe a versão 2ch no site do projeto." } else { "Homebrew não encontrado. Baixe a versão 2ch no site do projeto." });
                        ui.add_space(12.0);
                        if button(ui, Some(ph::ARROW_SQUARE_OUT), "Abrir existential.audio", Kind::Secondary).clicked() {
                            let _ = Command::new("open").arg(DOWNLOAD_URL).spawn();
                        }
                    });
                    if let Some(m) = &w.message {
                        ui.add_space(12.0);
                        ui.horizontal(|ui| {
                            status_dot(ui, p().text);
                            ui.label(egui::RichText::new(m).size(12.5).color(p().text_variant));
                        });
                    }
                    ui.add_space(22.0);
                    let (back, next) = footer(ui, Some(ph::ARROW_LEFT), "Voltar", Some(ph::ARROW_RIGHT), "Já instalei");
                    if back {
                        w.step = Step::Intro;
                    }
                    if next {
                        w.step = Step::Verify;
                        outcome = WizardOutcome::Rescan;
                    }
                }
                Step::Verify => {
                    ui.label(eyebrow("Verificação"));
                    ui.add_space(8.0);
                    ui.label(display("Ainda não encontramos o BlackHole", 30.0, p().text));
                    ui.add_space(14.0);
                    ui.horizontal(|ui| {
                        status_dot(ui, p().warning);
                        ui.label(caps("Dispositivo não detectado", 9.0, p().warning));
                    });
                    ui.add_space(10.0);
                    body(ui, "Se a instalação acabou de terminar, aguarde alguns segundos e verifique de novo. Se continuar sem aparecer, reinicie o Mac.");
                    ui.add_space(22.0);
                    let (back, next) = footer(ui, Some(ph::ARROW_LEFT), "Voltar", Some(ph::ARROWS_CLOCKWISE), "Verificar novamente");
                    if back {
                        w.step = Step::Install;
                    }
                    if next {
                        outcome = WizardOutcome::Rescan;
                    }
                }
                Step::Done => {
                    let name = found.unwrap_or("BlackHole 2ch");
                    ui.label(icon_fill(ph::CHECK_CIRCLE, 34.0, p().success));
                    ui.add_space(10.0);
                    ui.label(eyebrow("Tudo pronto"));
                    ui.add_space(8.0);
                    ui.label(display(&format!("{name} detectado."), 30.0, p().text));
                    ui.add_space(18.0);
                    let tips = [
                        format!("No Discord, Zoom ou OBS, escolha \"{name}\" como microfone."),
                        "Para ouvir os sons você mesmo, escolha seus fones em Monitor.".to_string(),
                        "Clique com o botão direito num pad para volume, loop e tecla.".to_string(),
                    ];
                    for (k, tip) in tips.iter().enumerate() {
                        hairline(ui);
                        ui.add_space(12.0);
                        ui.horizontal(|ui| {
                            ui.label(display(&format!("{:02}", k + 1), 18.0, p().text_muted));
                            ui.add_space(8.0);
                            ui.label(egui::RichText::new(tip).size(13.5).color(p().text));
                        });
                        ui.add_space(12.0);
                    }
                    hairline(ui);
                    ui.add_space(22.0);
                    let (_, next) = footer(ui, None, "", Some(ph::ARROW_RIGHT), "Usar e começar");
                    if next {
                        outcome = match found {
                            Some(n) => WizardOutcome::UseDevice(n.to_string()),
                            None => WizardOutcome::Close { hide: w.hide },
                        };
                    }
                }
            }
        });
    if modal.should_close() && matches!(outcome, WizardOutcome::None) {
        outcome = WizardOutcome::Close { hide: w.hide };
    }
    outcome
}

fn body(ui: &mut egui::Ui, text: &str) {
    ui.label(egui::RichText::new(text).size(13.5).color(p().text_variant).line_height(Some(20.0)));
}

fn card(ui: &mut egui::Ui, add: impl FnOnce(&mut egui::Ui)) {
    egui::Frame::default().stroke(Stroke::new(1.0_f32, p().border)).inner_margin(20.0).show(ui, |ui| {
        ui.set_width(ui.available_width());
        add(ui);
    });
}

/// Rodapé com ação secundária (texto) à esquerda e primária à direita. Retorna (voltar, avançar).
fn footer(ui: &mut egui::Ui, back_icon: Option<&str>, back: &str, next_icon: Option<&str>, next: &str) -> (bool, bool) {
    let mut clicked = (false, false);
    ui.horizontal(|ui| {
        if !back.is_empty() {
            clicked.0 = button(ui, back_icon, back, Kind::Ghost).clicked();
        }
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            clicked.1 = button(ui, next_icon, next, Kind::Primary).clicked();
        });
    });
    clicked
}

fn open_terminal(cmd: &str) -> std::io::Result<()> {
    let script = format!(
        "tell application \"Terminal\"\nactivate\ndo script \"{}\"\nend tell",
        cmd.replace('\\', "\\\\").replace('"', "\\\"")
    );
    let status = Command::new("osascript").arg("-e").arg(script).status()?;
    if status.success() {
        Ok(())
    } else {
        Err(std::io::Error::other("osascript falhou"))
    }
}
