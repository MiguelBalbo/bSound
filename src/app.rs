//! Estado do app, carregamento de clipes e barra superior.

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::atomic::Ordering::Relaxed;
use std::sync::Arc;
use std::time::Duration;

use crossbeam_channel::{unbounded, Receiver, Sender};
use eframe::egui::{self, Color32, RichText};

use crate::audio::clip::{self, Clip, AUDIO_EXTENSIONS};
use crate::audio::engine::AudioEngine;
use crate::audio::mixer::Command;
use crate::audio::shared::Shared;
use crate::config::{Config, Slot};
use crate::devices;
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
        let blackhole = devices::blackhole_output(&outputs);

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
        self.request_load(i);
        self.dirty = true;
    }

    pub(crate) fn remove(&mut self, i: usize) {
        self.engine.send(Command::Stop(i));
        self.config.slots[i] = None;
        self.clips.remove(&i);
        self.loading.remove(&i);
        self.load_errors.remove(&i);
        self.dirty = true;
    }

    pub(crate) fn pick_file(&mut self, i: usize) {
        let file = rfd::FileDialog::new()
            .set_title("Escolha um som")
            .add_filter("Áudio", AUDIO_EXTENSIONS)
            .pick_file();
        if let Some(path) = file {
            self.assign(i, path);
        }
    }

    pub(crate) fn trigger(&mut self, i: usize) {
        let Some(slot) = &self.config.slots[i] else { return };
        if self.shared.is_playing(i) && !slot.restart {
            self.engine.send(Command::Stop(i));
        } else if let Some(clip) = self.clips.get(&i) {
            self.engine.send(Command::Play { slot: i, clip: clip.clone() });
        }
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

    fn top_bar(&mut self, ui: &mut egui::Ui) {
        let mut restart = false;
        ui.horizontal_wrapped(|ui| {
            ui.label("🎤 Microfone");
            let label = self.config.input_device.clone().unwrap_or_else(|| "Padrão do sistema".into());
            egui::ComboBox::from_id_salt("input").selected_text(label).width(200.0).show_ui(ui, |ui| {
                restart |= ui.selectable_value(&mut self.config.input_device, None, "Padrão do sistema").changed();
                for n in &self.inputs {
                    restart |= ui.selectable_value(&mut self.config.input_device, Some(n.clone()), n).changed();
                }
            });

            ui.label("📤 Saída");
            let label = self.config.output_device.clone().unwrap_or_else(|| "Padrão do sistema".into());
            egui::ComboBox::from_id_salt("output").selected_text(label).width(200.0).show_ui(ui, |ui| {
                restart |= ui.selectable_value(&mut self.config.output_device, None, "Padrão do sistema").changed();
                for n in &self.outputs {
                    restart |= ui.selectable_value(&mut self.config.output_device, Some(n.clone()), n).changed();
                }
            });

            ui.label("🎧 Monitor");
            let label = self.config.monitor_device.clone().unwrap_or_else(|| "Nenhum".into());
            egui::ComboBox::from_id_salt("monitor").selected_text(label).width(180.0).show_ui(ui, |ui| {
                restart |= ui.selectable_value(&mut self.config.monitor_device, None, "Nenhum").changed();
                for n in self.outputs.iter().filter(|n| !devices::is_virtual(n)) {
                    restart |= ui.selectable_value(&mut self.config.monitor_device, Some(n.clone()), n).changed();
                }
            });

            if ui.button("↻").on_hover_text("Atualizar lista de dispositivos").clicked() {
                self.refresh_devices();
                restart = true;
            }
            if ui.button("Microfone virtual…").clicked() {
                self.wizard = Some(Wizard::new());
            }
        });

        ui.add_space(4.0);
        ui.horizontal_wrapped(|ui| {
            let mute_label = if self.config.mic_muted { "🔇 Mic mudo" } else { "🎤 Mic ativo" };
            if ui.toggle_value(&mut self.config.mic_muted, mute_label).changed() {
                self.dirty = true;
            }
            self.dirty |= gain_slider(ui, &mut self.config.mic_gain, "Mic");
            self.dirty |= gain_slider(ui, &mut self.config.board_gain, "Sons");
            self.dirty |= gain_slider(ui, &mut self.config.master_gain, "Master");
            if self.config.monitor_device.is_some() {
                self.dirty |= gain_slider(ui, &mut self.config.monitor_gain, "Monitor");
                self.dirty |= ui.checkbox(&mut self.config.monitor_mic, "Ouvir meu mic").changed();
            }
        });

        ui.add_space(4.0);
        ui.horizontal_wrapped(|ui| {
            self.mic_meter = self.shared.mic_level.take().max(self.mic_meter * 0.85);
            self.out_meter = self.shared.out_level.take().max(self.out_meter * 0.85);
            level_meter(ui, "Mic", self.mic_meter);
            level_meter(ui, "Saída", self.out_meter);
            if ui.button("⏹ Parar tudo").clicked() {
                self.engine.send(Command::StopAll);
            }
            ui.separator();
            ui.label("Grade");
            self.dirty |= ui.add(egui::DragValue::new(&mut self.config.rows).range(1..=8).suffix(" lin")).changed();
            ui.label("×");
            self.dirty |= ui.add(egui::DragValue::new(&mut self.config.cols).range(1..=8).suffix(" col")).changed();
        });

        let out_is_virtual = self.engine.output_name.as_deref().is_some_and(devices::is_virtual);
        if !out_is_virtual {
            ui.colored_label(
                Color32::from_rgb(230, 170, 60),
                "⚠ A saída não é o BlackHole: outros apps não vão receber o áudio como microfone.",
            );
        }
        for e in &self.engine.errors {
            ui.colored_label(Color32::from_rgb(230, 90, 90), format!("⚠ {e}"));
        }
        if self.shared.stream_error.load(Relaxed) {
            ui.horizontal(|ui| {
                ui.colored_label(Color32::from_rgb(230, 90, 90), "⚠ Um dispositivo de áudio foi desconectado ou falhou.");
                if ui.button("Reiniciar áudio").clicked() {
                    restart = true;
                }
            });
        }

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

        let visible = self.config.visible_slots();
        let under_pointer = pos.and_then(|p| self.slot_rects.iter().find(|(_, r)| r.contains(p)).map(|(i, _)| *i));
        let mut free = (0..visible).filter(|&i| self.config.slots[i].is_none()).collect::<Vec<_>>().into_iter();
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
        if ctx.wants_keyboard_input() || self.wizard.is_some() {
            return;
        }
        let mut hits = Vec::new();
        for i in 0..self.config.visible_slots() {
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

/// Medidor de nível simples (o ProgressBar do egui desenha um ponto quando está em 0).
fn level_meter(ui: &mut egui::Ui, label: &str, level: f32) {
    ui.label(label);
    let (rect, _) = ui.allocate_exact_size(egui::vec2(120.0, 10.0), egui::Sense::hover());
    let painter = ui.painter();
    painter.rect_filled(rect, 3.0, ui.visuals().extreme_bg_color);
    let level = level.clamp(0.0, 1.0);
    if level > 0.001 {
        let fill = egui::Rect::from_min_size(rect.min, egui::vec2(rect.width() * level, rect.height()));
        let color = if level > 0.9 { crate::ui::logo::CORAL } else { crate::ui::logo::VIOLET };
        painter.rect_filled(fill, 3.0, color);
    }
}

fn gain_slider(ui: &mut egui::Ui, value: &mut f32, label: &str) -> bool {
    ui.add(
        egui::Slider::new(value, 0.0..=2.0)
            .text(label)
            .custom_formatter(|v, _| format!("{:.0}%", v * 100.0)),
    )
    .changed()
}

impl eframe::App for SoundboardApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.poll_loads();
        self.handle_dropped_files(ctx);
        self.handle_keys(ctx);

        egui::TopBottomPanel::top("top").show(ctx, |ui| {
            ui.add_space(8.0);
            crate::ui::logo::header(ui, 26.0);
            ui.add_space(6.0);
            self.top_bar(ui);
            ui.add_space(6.0);
        });
        egui::CentralPanel::default().show(ctx, |ui| self.board_ui(ui));

        if let Some(i) = self.capture_key_for {
            egui::Window::new("Definir tecla")
                .collapsible(false)
                .resizable(false)
                .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
                .show(ctx, |ui| {
                    let name = self.config.slots[i].as_ref().map(|s| s.name.as_str()).unwrap_or("");
                    ui.label(RichText::new(format!("Pressione uma tecla para \"{name}\"")).size(16.0));
                    ui.label("Esc cancela.");
                });
        }

        if let Some(w) = &mut self.wizard {
            let found = devices::blackhole_output(&self.outputs);
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

        self.sync_shared();
        if self.dirty {
            self.config.save();
            self.dirty = false;
        }
        ctx.request_repaint_after(Duration::from_millis(33));
    }
}
