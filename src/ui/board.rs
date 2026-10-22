//! Grade de quadrados do soundboard.

use eframe::egui::{self, Align2, Color32, CornerRadius, FontId, Rect, Sense, Stroke, StrokeKind, Vec2};

use crate::app::SoundboardApp;

enum Action {
    Trigger(usize),
    Pick(usize),
    Remove(usize),
    CaptureKey(usize),
}

const GAP: f32 = 10.0;
const RADIUS: CornerRadius = CornerRadius::same(10);

impl SoundboardApp {
    pub(crate) fn board_ui(&mut self, ui: &mut egui::Ui) {
        let rows = self.config.rows;
        let cols = self.config.cols;
        let avail = ui.available_size();
        let w = ((avail.x - GAP * (cols as f32 - 1.0)) / cols as f32).max(70.0);
        let h = ((avail.y - GAP * (rows as f32 - 1.0)) / rows as f32).max(60.0);
        let size = Vec2::new(w, h);

        let hovering_files = ui.ctx().input(|i| !i.raw.hovered_files.is_empty());
        let mut actions = Vec::new();
        self.slot_rects.clear();

        egui::ScrollArea::both().auto_shrink([false, false]).show(ui, |ui| {
            ui.spacing_mut().item_spacing = Vec2::splat(GAP);
            for r in 0..rows {
                ui.horizontal(|ui| {
                    for c in 0..cols {
                        let i = r * cols + c;
                        self.slot_ui(ui, i, size, hovering_files, &mut actions);
                    }
                });
            }
        });

        for a in actions {
            match a {
                Action::Trigger(i) => self.trigger(i),
                Action::Pick(i) => self.pick_file(i),
                Action::Remove(i) => self.remove(i),
                Action::CaptureKey(i) => self.capture_key_for = Some(i),
            }
        }
    }

    fn slot_ui(&mut self, ui: &mut egui::Ui, i: usize, size: Vec2, hovering_files: bool, actions: &mut Vec<Action>) {
        let (rect, resp) = ui.allocate_exact_size(size, Sense::click());
        self.slot_rects.push((i, rect));
        let visuals = ui.visuals().clone();
        let painter = ui.painter_at(rect);
        let progress = self.shared.progress(i);
        let key = self.config.key_for(i);

        match &self.config.slots[i] {
            None => {
                let bg = if resp.hovered() || hovering_files {
                    visuals.widgets.hovered.bg_fill
                } else {
                    visuals.faint_bg_color
                };
                painter.rect_filled(rect, RADIUS, bg);
                painter.rect_stroke(
                    rect,
                    RADIUS,
                    Stroke::new(1.5f32, visuals.widgets.noninteractive.bg_stroke.color),
                    StrokeKind::Inside,
                );
                painter.text(rect.center(), Align2::CENTER_CENTER, "+", FontId::proportional(32.0), visuals.weak_text_color());
                painter.text(
                    rect.center_bottom() - Vec2::new(0.0, 12.0),
                    Align2::CENTER_BOTTOM,
                    "clique ou arraste um som",
                    FontId::proportional(11.0),
                    visuals.weak_text_color(),
                );
            }
            Some(slot) => {
                let base = slot_color(i, visuals.dark_mode);
                let bg = if progress.is_some() {
                    visuals.selection.bg_fill
                } else if resp.hovered() {
                    base.gamma_multiply(1.25)
                } else {
                    base
                };
                painter.rect_filled(rect, RADIUS, bg);
                if let Some(p) = progress {
                    let bar = Rect::from_min_max(
                        egui::pos2(rect.left(), rect.bottom() - 6.0),
                        egui::pos2(rect.left() + rect.width() * p, rect.bottom()),
                    );
                    painter.rect_filled(bar, CornerRadius::same(3), Color32::from_white_alpha(200));
                }
                if resp.hovered() || hovering_files {
                    painter.rect_stroke(rect, RADIUS, Stroke::new(2.0f32, visuals.strong_text_color()), StrokeKind::Inside);
                }

                let text_color = Color32::WHITE;
                let galley = painter.layout(
                    slot.name.clone(),
                    FontId::proportional(16.0),
                    text_color,
                    rect.width() - 16.0,
                );
                painter.galley(rect.center() - galley.size() / 2.0, galley, text_color);

                let status = if self.loading.contains(&i) {
                    Some("carregando…".to_string())
                } else {
                    self.load_errors.get(&i).map(|e| {
                        if slot.path.exists() { format!("⚠ {e}") } else { "⚠ arquivo não encontrado".into() }
                    })
                };
                if let Some(s) = status {
                    let g = painter.layout(s, FontId::proportional(11.0), Color32::from_rgb(255, 220, 160), rect.width() - 12.0);
                    painter.galley(egui::pos2(rect.left() + 6.0, rect.bottom() - 10.0 - g.size().y), g, Color32::WHITE);
                }
                let mut badges = Vec::new();
                if slot.looping {
                    badges.push("🔁");
                }
                if (slot.volume - 1.0).abs() > 0.01 {
                    badges.push("🔉");
                }
                painter.text(
                    rect.right_top() + Vec2::new(-8.0, 6.0),
                    Align2::RIGHT_TOP,
                    badges.join(" "),
                    FontId::proportional(12.0),
                    text_color,
                );
            }
        }
        if let Some(k) = &key {
            painter.text(
                rect.left_top() + Vec2::new(8.0, 6.0),
                Align2::LEFT_TOP,
                k,
                FontId::monospace(12.0),
                if self.config.slots[i].is_some() { Color32::from_white_alpha(220) } else { visuals.weak_text_color() },
            );
        }

        if resp.clicked() {
            actions.push(if self.config.slots[i].is_some() { Action::Trigger(i) } else { Action::Pick(i) });
        }

        let mut dirty = false;
        resp.context_menu(|ui| {
            ui.set_min_width(220.0);
            match self.config.slots[i].as_mut() {
                Some(slot) => {
                    ui.horizontal(|ui| {
                        ui.label("Nome");
                        dirty |= ui.text_edit_singleline(&mut slot.name).changed();
                    });
                    dirty |= ui
                        .add(
                            egui::Slider::new(&mut slot.volume, 0.0..=2.0)
                                .text("Volume")
                                .custom_formatter(|v, _| format!("{:.0}%", v * 100.0)),
                        )
                        .changed();
                    dirty |= ui.checkbox(&mut slot.looping, "Repetir (loop)").changed();
                    dirty |= ui.checkbox(&mut slot.restart, "Clique reinicia em vez de parar").changed();
                    ui.separator();
                    let key_label = key.as_deref().unwrap_or("nenhuma");
                    if ui.button(format!("⌨ Tecla de atalho: {key_label}")).clicked() {
                        actions.push(Action::CaptureKey(i));
                        ui.close_menu();
                    }
                    if ui.button("📂 Trocar arquivo…").clicked() {
                        actions.push(Action::Pick(i));
                        ui.close_menu();
                    }
                    ui.label(egui::RichText::new(slot.path.display().to_string()).small().weak());
                    ui.separator();
                    if ui.button("🗑 Remover").clicked() {
                        actions.push(Action::Remove(i));
                        ui.close_menu();
                    }
                }
                None => {
                    if ui.button("📂 Escolher arquivo…").clicked() {
                        actions.push(Action::Pick(i));
                        ui.close_menu();
                    }
                }
            }
        });
        self.dirty |= dirty;
    }
}

fn slot_color(i: usize, dark: bool) -> Color32 {
    const PALETTE: [(u8, u8, u8); 8] = [
        (79, 110, 210),
        (155, 89, 182),
        (41, 150, 120),
        (210, 110, 60),
        (190, 70, 110),
        (60, 140, 190),
        (120, 140, 60),
        (170, 120, 50),
    ];
    let (r, g, b) = PALETTE[i % PALETTE.len()];
    let c = Color32::from_rgb(r, g, b);
    if dark { c } else { c.gamma_multiply(0.95) }
}
