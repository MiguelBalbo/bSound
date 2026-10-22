//! Wizard de instalação do BlackHole (microfone virtual).

use std::path::PathBuf;
use std::process::Command;

use eframe::egui::{self, Color32, RichText};

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
    if found.is_some() && matches!(w.step, Step::Intro) {
        w.step = Step::Done;
    }

    egui::Window::new("Configurar microfone virtual")
        .collapsible(false)
        .resizable(false)
        .default_width(460.0)
        .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
        .show(ctx, |ui| {
            let steps = ["Introdução", "Instalar", "Verificar", "Pronto"];
            let current = w.step as usize;
            ui.horizontal(|ui| {
                for (n, s) in steps.iter().enumerate() {
                    let text = RichText::new(format!("{}. {s}", n + 1));
                    ui.label(if n == current { text.strong() } else { text.weak() });
                    if n + 1 < steps.len() {
                        ui.label(RichText::new("›").weak());
                    }
                }
            });
            ui.separator();

            match w.step {
                Step::Intro => {
                    crate::ui::logo::symbol(ui, 56.0);
                    ui.vertical_centered(|ui| ui.heading("Bem-vindo ao bSound"));
                    ui.add_space(6.0);
                    ui.label(
                        "Para Discord, Zoom, OBS e outros apps receberem seu microfone junto com o \
                         soundboard, o macOS precisa de um dispositivo de áudio virtual.",
                    );
                    ui.add_space(4.0);
                    ui.label(
                        "O BlackHole 2ch é um driver gratuito e de código aberto: este app envia a mistura \
                         para ele, e nos outros apps você escolhe \"BlackHole 2ch\" como microfone.",
                    );
                    ui.add_space(8.0);
                    ui.horizontal(|ui| {
                        if ui.button("Continuar ›").clicked() {
                            w.step = Step::Install;
                        }
                        if ui.button("Agora não").clicked() {
                            outcome = WizardOutcome::Close { hide: w.hide };
                        }
                        ui.checkbox(&mut w.hide, "Não mostrar ao abrir");
                    });
                }
                Step::Install => {
                    if let Some(cmd) = w.brew_command() {
                        ui.label("Opção 1 — Homebrew (recomendado):");
                        ui.label(RichText::new(&cmd).monospace());
                        ui.label(
                            RichText::new("Abre o Terminal com o comando; ele pede sua senha de administrador.")
                                .small()
                                .weak(),
                        );
                        ui.horizontal(|ui| {
                            if ui.button("Instalar com Homebrew").clicked() {
                                w.message = Some(match open_terminal(&cmd) {
                                    Ok(()) => "Terminal aberto. Conclua a instalação lá e depois clique em \"Já instalei\".".into(),
                                    Err(e) => format!("Não foi possível abrir o Terminal: {e}. Copie o comando e rode manualmente."),
                                });
                            }
                            if ui.button("Copiar comando").clicked() {
                                ui.ctx().copy_text(cmd.clone());
                                w.message = Some("Comando copiado.".into());
                            }
                        });
                        ui.add_space(8.0);
                        ui.label("Opção 2 — instalador oficial:");
                    } else {
                        ui.label("Homebrew não encontrado. Baixe o instalador oficial (escolha a versão 2ch):");
                    }
                    if ui.button("Baixar instalador (existential.audio)").clicked() {
                        let _ = Command::new("open").arg(DOWNLOAD_URL).spawn();
                    }
                    if let Some(m) = &w.message {
                        ui.add_space(6.0);
                        ui.colored_label(Color32::from_rgb(120, 180, 255), m);
                    }
                    ui.add_space(8.0);
                    ui.horizontal(|ui| {
                        if ui.button("‹ Voltar").clicked() {
                            w.step = Step::Intro;
                        }
                        if ui.button("Já instalei ›").clicked() {
                            w.step = Step::Verify;
                            outcome = WizardOutcome::Rescan;
                        }
                    });
                }
                Step::Verify => {
                    if found.is_some() {
                        w.step = Step::Done;
                    } else {
                        ui.colored_label(Color32::from_rgb(230, 170, 60), "BlackHole ainda não foi detectado.");
                        ui.label(
                            "Se a instalação acabou de terminar, aguarde alguns segundos e verifique de novo. \
                             Se continuar sem aparecer, reinicie o Mac (ou o app).",
                        );
                    }
                    ui.add_space(8.0);
                    ui.horizontal(|ui| {
                        if ui.button("‹ Voltar").clicked() {
                            w.step = Step::Install;
                        }
                        if ui.button("Verificar novamente").clicked() {
                            outcome = WizardOutcome::Rescan;
                        }
                    });
                }
                Step::Done => {
                    let name = found.unwrap_or("BlackHole 2ch");
                    ui.colored_label(Color32::from_rgb(90, 200, 120), format!("✔ {name} detectado."));
                    ui.add_space(4.0);
                    ui.label(format!("• No Discord, Zoom, OBS etc., escolha \"{name}\" como microfone."));
                    ui.label("• Para ouvir os sons você mesmo, escolha seus fones em \"Monitor\".");
                    ui.label("• Clique com o botão direito num quadro para volume, loop, tecla e remoção.");
                    ui.add_space(8.0);
                    if ui.button("Usar e fechar").clicked() {
                        outcome = match found {
                            Some(n) => WizardOutcome::UseDevice(n.to_string()),
                            None => WizardOutcome::Close { hide: w.hide },
                        };
                    }
                }
            }
        });

    outcome
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
