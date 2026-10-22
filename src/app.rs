//! Estado do app, carregamento de clipes, barra lateral e barra de status.

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::atomic::Ordering::Relaxed;
use std::sync::Arc;
use std::time::Duration;

use crossbeam_channel::{unbounded, Receiver, Sender};
use eframe::egui::{self, Color32};
use egui_phosphor::light as ph;

use crate::audio::clip::{self, Clip, AUDIO_EXTENSIONS};
use crate::audio::engine::AudioEngine;
use crate::audio::mixer::Command;
use crate::audio::shared::Shared;
use crate::audio::shared::MAX_PAGES;
use crate::config::{Config, Slot, TrayMode};
use crate::devices;
use crate::stats::Stats;
use crate::tray::{self, Tray, TrayLink};
use crate::ui::popover::Popover;
use crate::ui::theme::*;
use crate::ui::settings;
use crate::ui::wizard::{self, Wizard, WizardOutcome};

struct LoadResult {
    slot: usize,
    path: PathBuf,
    result: Result<Arc<Clip>, String>,
}

pub struct SoundboardApp {
    pub(crate) config: Config,
    pub(crate) shared: Arc<Shared>,
    pub(crate) engine: AudioEngine,
    pub(crate) clips: HashMap<usize, Arc<Clip>>,
    pub(crate) loading: HashSet<usize>,
    pub(crate) load_errors: HashMap<usize, String>,
    load_tx: Sender<LoadResult>,
    load_rx: Receiver<LoadResult>,
    ctx: egui::Context,
    inputs: Vec<String>,
    outputs: Vec<String>,
    wizard: Option<Wizard>,
    settings_open: bool,
    /// Janela escondida na barra de menus/bandeja.
    window_hidden: bool,
    last_theme_check: std::time::Instant,
    pub(crate) stats: Arc<Stats>,
    tray_link: Arc<TrayLink>,
    tray: Option<Tray>,
    pub(crate) popover: Option<Popover>,
    pub(crate) capture_key_for: Option<usize>,
    pub(crate) slot_rects: Vec<(usize, egui::Rect)>,
    mic_meter: f32,
    out_meter: f32,
    pub(crate) dirty: bool,
}

impl SoundboardApp {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        let mut config = Config::load();
        let inputs = devices::input_names();
        let outputs = devices::output_names();
        let blackhole = devices::virtual_output(&outputs);

        if config.output_device.as_ref().is_none_or(|o| !outputs.contains(o)) {
            config.output_device = blackhole.clone();
        }
        // Sem microfone virtual, a saída cai nos alto-falantes: muta o mic para evitar microfonia.
        if !config.output_device.as_deref().is_some_and(devices::is_virtual) {
            config.mic_muted = true;
        }

        let shared = Arc::new(Shared::new());
        let engine = AudioEngine::start(
            config.input_device.as_deref(),
            config.output_device.as_deref(),
            config.monitor_device.as_deref(),
            shared.clone(),
        );
        let (load_tx, load_rx) = unbounded();
        let wizard = (blackhole.is_none() && !config.hide_wizard).then(Wizard::new);
        crate::ui::theme::apply(&cc.egui_ctx, config.theme);

        let stats = Arc::new(Stats::load());
        let tray_link = Arc::new(TrayLink::new(cc.egui_ctx.clone()));
        let tray = Tray::new(tray_link.clone());

        let mut app = Self {
            config,
            shared,
            engine,
            clips: HashMap::new(),
            loading: HashSet::new(),
            load_errors: HashMap::new(),
            load_tx,
            load_rx,
            ctx: cc.egui_ctx.clone(),
            inputs,
            outputs,
            wizard,
            settings_open: false,
            window_hidden: false,
            last_theme_check: std::time::Instant::now(),
            stats,
            tray_link,
            tray,
            popover: None,
            capture_key_for: None,
            slot_rects: Vec::new(),
            mic_meter: 0.0,
            out_meter: 0.0,
            dirty: false,
        };
        app.reload_all_clips();
        app
    }

    pub(crate) fn request_load(&mut self, i: usize) {
        let Some(slot) = &self.config.slots[i] else { return };
        let path = slot.path.clone();
        let rate = self.engine.sample_rate;
        let tx = self.load_tx.clone();
        let ctx = self.ctx.clone();
        self.loading.insert(i);
        self.load_errors.remove(&i);
        std::thread::spawn(move || {
            let result = clip::load(&path, rate).map(Arc::new).map_err(|e| format!("{e:#}"));
            let _ = tx.send(LoadResult { slot: i, path, result });
            ctx.request_repaint();
        });
    }

    fn reload_all_clips(&mut self) {
        self.clips.clear();
        for i in 0..self.config.slots.len() {
            self.request_load(i);
        }
    }

    fn poll_loads(&mut self) {
        while let Ok(r) = self.load_rx.try_recv() {
            let current = self.config.slots[r.slot].as_ref().map(|s| &s.path);
            let stale_rate = r.result.as_ref().is_ok_and(|c| c.sample_rate != self.engine.sample_rate);
            if current != Some(&r.path) || stale_rate {
                continue; // resultado obsoleto
            }
            self.loading.remove(&r.slot);
            match r.result {
                Ok(c) => {
                    self.clips.insert(r.slot, c);
                }
                Err(e) => {
                    self.load_errors.insert(r.slot, e);
                }
            }
        }
    }

    pub(crate) fn assign(&mut self, i: usize, path: PathBuf) {
        self.engine.send(Command::Stop(i));
        let key = self.config.slots[i].as_ref().and_then(|s| s.key.clone());
        let mut slot = Slot::from_path(path);
        slot.key = key;
        self.config.slots[i] = Some(slot);
        self.clips.remove(&i);
        self.stats.reset(i);
        self.request_load(i);
        self.dirty = true;
    }

    pub(crate) fn remove(&mut self, i: usize) {
        self.engine.send(Command::Stop(i));
        self.config.slots[i] = None;
        self.clips.remove(&i);
        self.stats.reset(i);
        self.loading.remove(&i);
        self.load_errors.remove(&i);
        self.dirty = true;
    }

    fn choose_file() -> Option<PathBuf> {
        rfd::FileDialog::new()
            .set_title("Escolha um som")
            .add_filter("Áudio", AUDIO_EXTENSIONS)
            .pick_file()
    }

    pub(crate) fn pick_file(&mut self, i: usize) {
        if let Some(path) = Self::choose_file() {
            self.assign(i, path);
        }
    }

    /// Cria uma página vazia depois da última e abre nela.
    pub(crate) fn new_page(&mut self) {
        if self.config.pages < MAX_PAGES {
            self.config.pages += 1;
            self.config.current_page = self.config.pages - 1;
            self.dirty = true;
        }
    }

    pub(crate) fn go_to_page(&mut self, page: usize) {
        if page < self.config.pages && page != self.config.current_page {
            self.config.current_page = page;
            self.dirty = true;
        }
    }

    pub(crate) fn trigger(&mut self, i: usize) {
        let Some(slot) = &self.config.slots[i] else { return };
        if self.shared.is_playing(i) && !slot.restart {
            self.engine.send(Command::Stop(i));
        } else if let Some(clip) = self.clips.get(&i) {
            self.engine.send(Command::Play { slot: i, clip: clip.clone() });
            self.stats.record(i);
        }
    }

    /// Sons do menu da barra de menus: selecionados ou os mais tocados.
    pub(crate) fn tray_entries(&self) -> (&'static str, Vec<(usize, String)>) {
        let filled: Vec<(usize, &Slot)> = self
            .config
            .all_visible_slots()
            .filter_map(|i| self.config.slots[i].as_ref().map(|s| (i, s)))
            .collect();
        let name = |(i, s): &(usize, &Slot)| (*i, s.name.clone());
        match self.config.tray_mode {
            TrayMode::Pinned => ("Selecionados", filled.iter().filter(|(_, s)| s.pinned).take(12).map(name).collect()),
            TrayMode::MostPlayed => {
                let mut played: Vec<_> = filled.iter().filter(|(i, _)| self.stats.get(*i) > 0).collect();
                played.sort_by_key(|(i, _)| std::cmp::Reverse(self.stats.get(*i)));
                if played.is_empty() {
                    ("Sons", filled.iter().take(9).map(name).collect())
                } else {
                    ("Mais tocados", played.into_iter().take(9).map(name).collect())
                }
            }
        }
    }

    /// Só redesenha rápido quando algo na tela está mudando. Pedidos do ícone, carregamento de
    /// clipes e entrada do usuário já acordam a UI na hora; isto cobre as animações.
    fn repaint_interval(&self) -> Duration {
        let playing = self.config.all_visible_slots().any(|i| self.shared.is_playing(i));
        let meters_moving = self.mic_meter > 0.01 || self.out_meter > 0.01;
        let visible = !self.window_hidden || self.popover.is_some();
        match (visible, playing, meters_moving) {
            (false, _, _) => Duration::from_secs(1),
            (true, true, _) => Duration::from_millis(33),
            (true, false, true) => Duration::from_millis(50),
            // Parado: confere de tempos em tempos se o mic começou a captar som.
            (true, false, false) => Duration::from_millis(250),
        }
    }

    pub(crate) fn show_main_window(&mut self, ctx: &egui::Context) {
        tray::show_app();
        self.window_hidden = false;
        ctx.send_viewport_cmd(egui::ViewportCommand::Visible(true));
        ctx.send_viewport_cmd(egui::ViewportCommand::Focus);
    }

    fn restart_engine(&mut self) {
        let old_rate = self.engine.sample_rate;
        self.engine.stop();
        self.engine = AudioEngine::start(
            self.config.input_device.as_deref(),
            self.config.output_device.as_deref(),
            self.config.monitor_device.as_deref(),
            self.shared.clone(),
        );
        if self.engine.sample_rate != old_rate {
            self.reload_all_clips();
        }
        self.dirty = true;
    }

    fn refresh_devices(&mut self) {
        crate::platform::refresh();
        self.inputs = devices::input_names();
        self.outputs = devices::output_names();
    }

    fn sync_shared(&self) {
        let c = &self.config;
        let s = &self.shared;
        s.mic_gain.set(c.mic_gain);
        s.board_gain.set(c.board_gain);
        s.master_gain.set(c.master_gain);
        s.monitor_gain.set(c.monitor_gain);
        s.mic_muted.store(c.mic_muted, Relaxed);
        s.monitor_mic.store(c.monitor_mic, Relaxed);
        for (i, slot) in c.slots.iter().enumerate() {
            if let Some(slot) = slot {
                s.slot_gain[i].set(slot.volume);
                s.slot_loop[i].store(slot.looping, Relaxed);
            }
        }
    }

    fn sidebar(&mut self, ui: &mut egui::Ui) {
        let mut restart = false;
        ui.add_space(crate::platform::TITLEBAR_INSET);
        ui.horizontal(|ui| {
            crate::ui::logo::header(ui, 22.0);
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if icon_button(ui, ph::GEAR, 32.0, None).on_hover_text("Configurações").clicked() {
                    self.settings_open = true;
                }
            });
        });
        ui.add_space(30.0);

        egui::ScrollArea::vertical()
            .auto_shrink([false, false])
            .scroll_bar_visibility(egui::scroll_area::ScrollBarVisibility::AlwaysHidden)
            .show(ui, |ui| {
            // Entrada
            ui.label(eyebrow("Entrada"));
            ui.add_space(2.0);
            ui.horizontal(|ui| {
                ui.label(display("Microfone", 22.0, p().text));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let muted = self.config.mic_muted;
                    let (glyph, active, tip) = if muted {
                        (ph::MICROPHONE_SLASH, Some(CORAL), "Ativar microfone")
                    } else {
                        (ph::MICROPHONE, None, "Silenciar microfone")
                    };
                    if icon_button(ui, glyph, 32.0, active).on_hover_text(tip).clicked() {
                        self.config.mic_muted = !muted;
                        self.dirty = true;
                    }
                });
            });
            ui.add_space(4.0);
            restart |= device_combo(ui, "input", &mut self.config.input_device, "Padrão do sistema", &self.inputs);
            ui.add_space(10.0);
            self.dirty |= gain(ui, "Ganho", &mut self.config.mic_gain, 2.0);
            ui.add_space(2.0);
            meter(ui, self.mic_meter, CORAL);
            if self.config.mic_muted {
                ui.add_space(4.0);
                ui.label(caps("Silenciado", 9.0, CORAL));
            }

            section_gap(ui);
            ui.label(eyebrow("Saída"));
            ui.add_space(2.0);
            ui.label(display("Microfone virtual", 22.0, p().text));
            ui.add_space(4.0);
            restart |= device_combo(ui, "output", &mut self.config.output_device, "Padrão do sistema", &self.outputs);
            ui.add_space(6.0);
            let out_is_virtual = self.engine.output_name.as_deref().is_some_and(devices::is_virtual);
            ui.horizontal(|ui| {
                if out_is_virtual {
                    status_dot(ui, p().success);
                    ui.label(caps("Pronto para Discord, Zoom e OBS", 9.0, p().text_variant));
                } else {
                    status_dot(ui, p().warning);
                    ui.label(caps("Não é um microfone virtual", 9.0, p().warning));
                }
            });
            if !out_is_virtual {
                ui.add_space(6.0);
                if button(ui, Some(ph::ARROW_RIGHT), "Configurar microfone virtual", Kind::Secondary).clicked() {
                    self.wizard = Some(Wizard::new());
                }
            }

            section_gap(ui);
            ui.label(eyebrow("Mix"));
            ui.add_space(8.0);
            self.dirty |= gain(ui, "Sons", &mut self.config.board_gain, 2.0);
            ui.add_space(8.0);
            self.dirty |= gain(ui, "Master", &mut self.config.master_gain, 2.0);
            ui.add_space(2.0);
            meter(ui, self.out_meter, p().text);

            section_gap(ui);
            ui.label(eyebrow("Monitor"));
            ui.add_space(2.0);
            ui.label(display("Fones", 22.0, p().text));
            ui.add_space(4.0);
            let monitors: Vec<String> = self.outputs.iter().filter(|n| !devices::is_virtual(n)).cloned().collect();
            restart |= device_combo(ui, "monitor", &mut self.config.monitor_device, "Nenhum", &monitors);
            if self.config.monitor_device.is_some() {
                ui.add_space(10.0);
                self.dirty |= gain(ui, "Volume", &mut self.config.monitor_gain, 2.0);
                ui.add_space(4.0);
                self.dirty |= ui.checkbox(&mut self.config.monitor_mic, "Ouvir meu microfone").changed();
            } else {
                ui.add_space(4.0);
                ui.label(egui::RichText::new("Escolha seus fones para ouvir os sons que você toca.").size(12.0).color(p().text_variant));
            }

            section_gap(ui);
            if button(ui, Some(ph::ARROWS_CLOCKWISE), "Atualizar dispositivos", Kind::Ghost).clicked() {
                self.refresh_devices();
                restart = true;
            }
            if button(ui, Some(ph::BROADCAST), "Microfone virtual", Kind::Ghost).clicked() {
                self.wizard = Some(Wizard::new());
            }
            ui.add_space(16.0);
        });

        if restart {
            self.restart_engine();
        }
    }

    fn status_bar(&mut self, ui: &mut egui::Ui) {
        let mut restart = false;
        ui.horizontal_centered(|ui| {
            let out_is_virtual = self.engine.output_name.as_deref().is_some_and(devices::is_virtual);
            let failed = self.shared.stream_error.load(Relaxed);
            let color = if failed {
                p().danger
            } else if out_is_virtual {
                p().success
            } else {
                p().warning
            };
            status_dot(ui, color);
            let rate = format!("{:.1}", self.engine.sample_rate as f32 / 1000.0);
            let name = self.engine.output_name.clone().unwrap_or_else(|| "Sem saída".into());
            ui.label(caps(&format!("{name} · {} kHz", rate.trim_end_matches(".0")), 9.0, p().text_variant));

            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if failed {
                    if button(ui, Some(ph::ARROWS_CLOCKWISE), "Reiniciar áudio", Kind::Ghost).clicked() {
                        restart = true;
                    }
                    ui.label(caps("Um dispositivo foi desconectado", 9.0, p().danger));
                } else if let Some(e) = self.engine.errors.first() {
                    ui.label(icon_text(ph::WARNING, e, p().danger));
                } else {
                    ui.label(caps("Clique direito edita  ·  arraste arquivos para os pads", 9.0, p().text_muted));
                }
            });
        });
        if restart {
            self.restart_engine();
        }
    }

    fn handle_dropped_files(&mut self, ctx: &egui::Context) {
        let (dropped, pos) = ctx.input(|i| (i.raw.dropped_files.clone(), i.pointer.latest_pos()));
        let mut paths = dropped.into_iter().filter_map(|f| f.path).filter(|p| {
            p.extension()
                .and_then(|e| e.to_str())
                .is_some_and(|e| AUDIO_EXTENSIONS.contains(&e.to_lowercase().as_str()))
        });
        let Some(first) = paths.next() else { return };

        let under_pointer = pos.and_then(|p| self.slot_rects.iter().find(|(_, r)| r.contains(p)).map(|(i, _)| *i));
        let mut free = self.config.current_slots().filter(|&i| self.config.slots[i].is_none()).collect::<Vec<_>>().into_iter();
        let target = under_pointer.or_else(|| free.next());
        if let Some(t) = target {
            self.assign(t, first);
        }
        for p in paths {
            let Some(i) = free.find(|&i| Some(i) != target && self.config.slots[i].is_none()) else { break };
            self.assign(i, p);
        }
    }

    fn handle_keys(&mut self, ctx: &egui::Context) {
        if let Some(i) = self.capture_key_for {
            let pressed = ctx.input(|inp| {
                inp.events.iter().find_map(|e| match e {
                    egui::Event::Key { key, pressed: true, .. } => Some(*key),
                    _ => None,
                })
            });
            if let Some(key) = pressed {
                if key != egui::Key::Escape {
                    if let Some(slot) = self.config.slots[i].as_mut() {
                        slot.key = Some(key.name().to_string());
                        self.dirty = true;
                    }
                }
                self.capture_key_for = None;
            }
            return;
        }
        if ctx.wants_keyboard_input() || self.wizard.is_some() || self.settings_open {
            return;
        }
        // Setas ← → trocam de página.
        let (left, right) = ctx.input(|inp| {
            (
                inp.modifiers.is_none() && inp.key_pressed(egui::Key::ArrowLeft),
                inp.modifiers.is_none() && inp.key_pressed(egui::Key::ArrowRight),
            )
        });
        if left && self.config.current_page > 0 {
            self.go_to_page(self.config.current_page - 1);
        }
        if right {
            self.go_to_page(self.config.current_page + 1);
        }

        let mut hits = Vec::new();
        for i in self.config.current_slots() {
            if self.config.slots[i].is_none() {
                continue;
            }
            if let Some(key) = self.config.key_for(i).and_then(|k| egui::Key::from_name(&k)) {
                if ctx.input(|inp| inp.key_pressed(key) && inp.modifiers.is_none()) {
                    hits.push(i);
                }
            }
        }
        for i in hits {
            self.trigger(i);
        }
    }
}

fn section_gap(ui: &mut egui::Ui) {
    ui.add_space(22.0);
    hairline(ui);
    ui.add_space(22.0);
}

fn device_combo(ui: &mut egui::Ui, id: &str, value: &mut Option<String>, none_label: &str, options: &[String]) -> bool {
    let mut changed = false;
    let selected = value.clone().unwrap_or_else(|| none_label.to_owned());
    egui::ComboBox::from_id_salt(id)
        .selected_text(selected)
        .width(ui.available_width())
        .show_ui(ui, |ui| {
            changed |= ui.selectable_value(value, None, none_label).changed();
            for n in options {
                changed |= ui.selectable_value(value, Some(n.clone()), n).changed();
            }
        });
    changed
}

impl eframe::App for SoundboardApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Acompanha o modo claro/escuro do sistema quando a aparência é "Sistema".
        if self.config.theme == crate::config::ThemeMode::System && self.last_theme_check.elapsed() > Duration::from_secs(1) {
            self.last_theme_check = std::time::Instant::now();
            crate::ui::theme::set_mode(ctx, self.config.theme);
        }
        crate::ui::theme::sync(ctx);
        // Fechar a janela esconde o app na barra de menus (se ativado nas configurações).
        if ctx.input(|i| i.viewport().close_requested()) && self.config.close_to_tray && self.tray.is_some() {
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            ctx.send_viewport_cmd(egui::ViewportCommand::Visible(false));
            self.window_hidden = true;
            tray::hide_app();
        }
        // Pedidos do ícone da barra de menus.
        if let Some(anchor) = self.tray_link.take_popover_request() {
            if self.popover.take().is_none() {
                let ppp = ctx.native_pixels_per_point().unwrap_or(1.0) as f64;
                let to_pts = |v: f64| (v / ppp) as f32;
                let anchor = anchor.map(|a| (to_pts(a.x), to_pts(a.top), to_pts(a.bottom)));
                let screen = ctx.input(|i| i.viewport().monitor_size).unwrap_or(egui::vec2(1440.0, 900.0));
                self.popover = Some(Popover::new(anchor, screen));
            }
        }
        if self.tray_link.take_open_main() {
            self.popover = None;
            self.show_main_window(ctx);
        }
        self.poll_loads();
        self.handle_dropped_files(ctx);
        self.handle_keys(ctx);

        self.mic_meter = self.shared.mic_level.take().max(self.mic_meter * 0.85);
        self.out_meter = self.shared.out_level.take().max(self.out_meter * 0.85);

        // Com uma janela modal aberta, cliques não podem chegar aos pads e controles de trás.
        let modal_open = self.settings_open || self.wizard.is_some() || self.capture_key_for.is_some();
        egui::SidePanel::left("sidebar")
            .exact_width(300.0)
            .resizable(false)
            .frame(egui::Frame::default().fill(p().bg).inner_margin(egui::Margin::symmetric(24, 0)))
            .show(ctx, |ui| {
                if modal_open {
                    ui.disable();
                }
                self.sidebar(ui)
            });
        egui::TopBottomPanel::bottom("status")
            .exact_height(36.0)
            .frame(egui::Frame::default().fill(p().bg).inner_margin(egui::Margin::symmetric(24, 0)))
            .show(ctx, |ui| self.status_bar(ui));
        egui::CentralPanel::default()
            .frame(egui::Frame::default().fill(p().bg).inner_margin(egui::Margin { left: 32, right: 32, top: 0, bottom: 24 }))
            .show(ctx, |ui| {
                if modal_open {
                    ui.disable();
                }
                self.board_ui(ui)
            });

        if let Some(i) = self.capture_key_for {
            let modal = egui::Modal::new(egui::Id::new("key-capture"))
                .backdrop_color(Color32::from_black_alpha(180))
                .frame(modal_frame())
                .show(ctx, |ui| {
                    ui.set_width(380.0);
                    let name = self.config.slots[i].as_ref().map(|s| s.name.as_str()).unwrap_or("");
                    ui.label(eyebrow("Tecla de atalho"));
                    ui.add_space(8.0);
                    ui.label(display(&format!("Pressione uma tecla para “{name}”"), 24.0, p().text));
                    ui.add_space(14.0);
                    ui.label(egui::RichText::new("Esc cancela.").size(12.5).color(p().text_variant));
                });
            if modal.should_close() {
                self.capture_key_for = None;
            }
        }

        if let Some(w) = &mut self.wizard {
            let found = devices::virtual_output(&self.outputs);
            match wizard::show(ctx, w, found.as_deref()) {
                WizardOutcome::None => {}
                WizardOutcome::Rescan => self.refresh_devices(),
                WizardOutcome::Close { hide } => {
                    self.config.hide_wizard = hide;
                    self.wizard = None;
                    self.dirty = true;
                }
                WizardOutcome::UseDevice(name) => {
                    self.wizard = None;
                    self.config.output_device = Some(name);
                    if self.config.monitor_device.as_deref().is_some_and(devices::is_virtual) {
                        self.config.monitor_device = None;
                    }
                    self.restart_engine();
                }
            }
        }

        if self.settings_open {
            match settings::show(ctx, &mut self.config) {
                settings::Outcome::None => {}
                settings::Outcome::Changed => self.dirty = true,
                settings::Outcome::Close => self.settings_open = false,
            }
        }

        self.popover_ui(ctx);
        self.sync_shared();
        if self.dirty {
            self.config.save();
            self.dirty = false;
        }
        ctx.request_repaint_after(self.repaint_interval());
    }
}
