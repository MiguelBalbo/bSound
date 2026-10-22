//! Assistente do microfone virtual (BlackHole no macOS, VB-CABLE no Windows,
//! saída virtual do PulseAudio/PipeWire no Linux).

use eframe::egui::{self, Color32, Stroke, Vec2};
use egui_phosphor::light as ph;

use crate::ui::logo;
use crate::ui::theme::{self, *};

use crate::platform::{self, InstallCommand, VIRTUAL};

#[derive(PartialEq, Clone, Copy)]
enum Step {
    Intro,
    Install,
    Verify,
    Done,
}

pub struct Wizard {
    step: Step,
    install: Option<InstallCommand>,
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
        Self { step: Step::Intro, install: platform::install_command(), message: None, hide: false }
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
                    body(ui, &format!("Para Discord, Zoom, OBS e outros apps receberem seu microfone junto com o soundboard, {} precisa de um dispositivo de áudio virtual.", VIRTUAL.os));
                    ui.add_space(6.0);
                    body(ui, VIRTUAL.intro);
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
                    ui.label(display(VIRTUAL.install_title, 30.0, p().text));
                    ui.add_space(18.0);
                    let mut run = None;
                    if let Some(install) = &w.install {
                        card(ui, |ui| {
                            ui.horizontal(|ui| {
                                ui.label(display(install.title, 18.0, p().text));
                                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                    let (r, _) = ui.allocate_exact_size(Vec2::new(96.0, 20.0), egui::Sense::hover());
                                    theme::badge(ui.painter(), ui, r.right_top() - Vec2::new(92.0, 0.0), "Recomendado", p().text, p().bg);
                                });
                            });
                            ui.add_space(6.0);
                            body(ui, install.description);
                            ui.add_space(10.0);
                            egui::Frame::default().fill(p().inset).stroke(Stroke::new(1.0_f32, p().border)).inner_margin(10.0).show(ui, |ui| {
                                ui.set_width(ui.available_width());
                                ui.label(egui::RichText::new(&install.command).monospace().size(11.5).color(p().text));
                            });
                            ui.add_space(12.0);
                            ui.horizontal(|ui| {
                                if button(ui, Some(ph::TERMINAL), install.button, Kind::Primary).clicked() {
                                    run = Some(install.command.clone());
                                }
                                if button(ui, Some(ph::COPY), "Copiar", Kind::Secondary).clicked() {
                                    ui.ctx().copy_text(install.command.clone());
                                    w.message = Some("Comando copiado.".into());
                                }
                            });
                        });
                        ui.add_space(10.0);
                    }
                    if let Some(cmd) = run {
                        w.message = Some(match platform::run_install(&cmd) {
                            Ok(()) if cfg!(target_os = "macos") => "Terminal aberto. Conclua a instalação lá e depois clique em \"Já instalei\".".into(),
                            Ok(()) => {
                                outcome = WizardOutcome::Rescan;
                                "Microfone virtual criado. Clique em \"Já instalei\".".into()
                            }
                            Err(e) => format!("Não foi possível executar: {e}. Copie o comando e rode manualmente."),
                        });
                    }
                    card(ui, |ui| {
                        ui.label(display(VIRTUAL.download_title, 18.0, p().text));
                        ui.add_space(6.0);
                        body(ui, VIRTUAL.download_text);
                        ui.add_space(12.0);
                        if button(ui, Some(ph::ARROW_SQUARE_OUT), VIRTUAL.download_label, Kind::Secondary).clicked() {
                            platform::open_url(VIRTUAL.download_url);
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
                    ui.label(display(&format!("Ainda não encontramos o {}", VIRTUAL.driver), 30.0, p().text));
                    ui.add_space(14.0);
                    ui.horizontal(|ui| {
                        status_dot(ui, p().warning);
                        ui.label(caps("Dispositivo não detectado", 9.0, p().warning));
                    });
                    ui.add_space(10.0);
                    body(ui, "Se a instalação acabou de terminar, aguarde alguns segundos e verifique de novo. Se continuar sem aparecer, reinicie o computador.");
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
                    let name = VIRTUAL.mic_name;
                    ui.label(icon_fill(ph::CHECK_CIRCLE, 34.0, p().success));
                    ui.add_space(10.0);
                    ui.label(eyebrow("Tudo pronto"));
                    ui.add_space(8.0);
                    ui.label(display(&format!("{name} pronto."), 30.0, p().text));
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
