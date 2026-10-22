//! Área principal: cabeçalho editorial e grade de pads em treliça de linhas finas.

use eframe::egui::{self, text::LayoutJob, Align, Align2, Color32, CursorIcon, Layout, Pos2, Rect, Sense, Stroke, Vec2};
use egui_phosphor::light as ph;

use crate::app::SoundboardApp;
use crate::audio::mixer::Command;
use crate::audio::shared::{MAX_PAGES, PAGE_SIZE};
use crate::ui::theme::{self, *};

enum Action {
    Trigger(usize),
    NewPage,
    GoToPage(usize),
    Pick(usize),
    Remove(usize),
    CaptureKey(usize),
}

const GRID_PRESETS: &[((usize, usize), &str)] = &[((3, 3), "3×3"), ((4, 4), "4×4"), ((5, 5), "5×5"), ((6, 6), "6×6")];

impl SoundboardApp {
    pub(crate) fn board_ui(&mut self, ui: &mut egui::Ui) {
        let mut actions = Vec::new();
        ui.add_space(30.0);
        self.pager(ui, &mut actions);
        ui.add_space(14.0);
        if self.board_header(ui) {
            actions.push(Action::NewPage);
        }
        ui.add_space(28.0);

        let rows = self.config.rows;
        let cols = self.config.cols;
        let avail = ui.available_size();
        let cell_w = ((avail.x - (cols + 1) as f32) / cols as f32).floor().max(120.0);
        let cell_h = ((avail.y - (rows + 1) as f32) / rows as f32).floor().max(112.0);
        let grid = Vec2::new(cell_w * cols as f32 + (cols + 1) as f32, cell_h * rows as f32 + (rows + 1) as f32);

        let hovering_files = ui.ctx().input(|i| !i.raw.hovered_files.is_empty());
        self.slot_rects.clear();
        let start = self.config.current_page * PAGE_SIZE;

        egui::ScrollArea::both().auto_shrink([false, false]).show(ui, |ui| {
            let (grid_rect, _) = ui.allocate_exact_size(grid, Sense::hover());
            // Treliça: fundo na cor da linha, células com 1 px de folga entre si.
            ui.painter().rect_filled(grid_rect, 0, p().border);
            for r in 0..rows {
                for c in 0..cols {
                    let i = start + r * cols + c;
                    let min = grid_rect.min + Vec2::new(1.0 + c as f32 * (cell_w + 1.0), 1.0 + r as f32 * (cell_h + 1.0));
                    let rect = Rect::from_min_size(min, Vec2::new(cell_w, cell_h));
                    self.slot_ui(ui, i, rect, hovering_files, &mut actions);
                }
            }
        });

        for a in actions {
            match a {
                Action::Trigger(i) => self.trigger(i),
                Action::NewPage => self.new_page(),
                Action::GoToPage(page) => self.go_to_page(page),
                Action::Pick(i) => self.pick_file(i),
                Action::Remove(i) => self.remove(i),
                Action::CaptureKey(i) => self.capture_key_for = Some(i),
            }
        }
    }

    /// Faixa no topo: setas, indicador de páginas e "Página N de M".
    fn pager(&self, ui: &mut egui::Ui, actions: &mut Vec<Action>) {
        let page = self.config.current_page;
        let pages = self.config.pages;
        ui.horizontal(|ui| {
            ui.label(eyebrow("Soundboard"));
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                ui.label(caps(&format!("de {pages:02}"), 9.0, p().text_variant));
                ui.label(display(&format!("{:02}", page + 1), 15.0, p().text));
                ui.label(caps("Página", 9.0, p().text_variant));
                ui.add_space(12.0);
                if pager_arrow(ui, ph::CARET_RIGHT, page + 1 < pages, "Próxima página (→)") {
                    actions.push(Action::GoToPage(page + 1));
                }
                // Indicador: um segmento por página; violeta se há som tocando nela.
                ui.add_space(4.0);
                for k in (0..pages).rev() {
                    let (rect, resp) = ui.allocate_exact_size(Vec2::new(22.0, 20.0), Sense::click());
                    let resp = resp.on_hover_text(format!("Página {}", k + 1)).on_hover_cursor(CursorIcon::PointingHand);
                    let playing = self.config.page_slots(k).any(|i| self.shared.is_playing(i));
                    let color = if k == page {
                        p().text
                    } else if playing {
                        VIOLET
                    } else if resp.hovered() {
                        p().text_variant
                    } else {
                        p().border
                    };
                    let h = if k == page { 3.0 } else { 2.0 };
                    let bar = Rect::from_center_size(rect.center(), Vec2::new(rect.width(), h));
                    ui.painter().rect_filled(bar, 0, color);
                    if resp.clicked() {
                        actions.push(Action::GoToPage(k));
                    }
                }
                ui.add_space(4.0);
                if pager_arrow(ui, ph::CARET_LEFT, page > 0, "Página anterior (←)") {
                    actions.push(Action::GoToPage(page.saturating_sub(1)));
                }
            });
        });
    }

    /// Retorna true se "Nova página" foi clicado.
    fn board_header(&mut self, ui: &mut egui::Ui) -> bool {
        let mut new_page = false;
        let range = self.config.current_slots();
        let visible = range.len();
        let filled = range.clone().filter(|&i| self.config.slots[i].is_some()).count();
        let playing = self.config.all_visible_slots().filter(|&i| self.shared.is_playing(i)).count();

        ui.horizontal(|ui| {
            ui.vertical(|ui| {
                ui.horizontal(|ui| {
                    ui.label(display("Sons", 46.0, p().text));
                    ui.add_space(10.0);
                    ui.vertical(|ui| {
                        ui.add_space(24.0); // alinha o contador à base do título
                        ui.horizontal(|ui| {
                            ui.label(display(&filled.to_string(), 22.0, p().text));
                            ui.label(caps(&format!("de {visible} pads"), 9.0, p().text_variant));
                            if playing > 0 {
                                ui.add_space(6.0);
                                let (rect, _) = ui.allocate_exact_size(Vec2::new(92.0, 20.0), Sense::hover());
                                let label = format!("{playing} tocando");
                                theme::badge(ui.painter(), ui, rect.left_center() - Vec2::new(0.0, 10.0), &label, VIOLET, Color32::WHITE);
                            }
                        });
                    });
                });
            });

            ui.with_layout(Layout::right_to_left(Align::Max), |ui| {
                let full = self.config.pages >= MAX_PAGES;
                let hint = if full { "Limite de páginas atingido" } else { "Criar uma página de pads vazia" };
                new_page = button(ui, Some(ph::PLUS), "Nova página", Kind::Primary).on_hover_text(hint).clicked();
                ui.add_space(8.0);
                if button(ui, Some(ph::STOP_CIRCLE), "Parar tudo", Kind::Secondary).on_hover_text("Para todos os sons").clicked() {
                    self.engine.send(Command::StopAll);
                }
                ui.add_space(12.0);
                let mut grid = (self.config.rows, self.config.cols);
                if segmented(ui, &mut grid, GRID_PRESETS) {
                    (self.config.rows, self.config.cols) = grid;
                    self.dirty = true;
                }
            });
        });
        new_page
    }

    fn slot_ui(&mut self, ui: &mut egui::Ui, i: usize, rect: Rect, hovering_files: bool, actions: &mut Vec<Action>) {
        self.slot_rects.push((i, rect));
        let resp = ui
            .interact(rect, ui.id().with(("slot", i)), Sense::click())
            .on_hover_cursor(CursorIcon::PointingHand);
        let painter = ui.painter_at(rect);
        let progress = self.shared.progress(i);
        let key = self.config.key_for(i);
        let pad = 16.0;
        let drop_target = hovering_files && ui.ctx().pointer_hover_pos().is_some_and(|p| rect.contains(p));
        let pressed = resp.is_pointer_button_down_on();

        match &self.config.slots[i] {
            None => {
                let bg = if pressed {
                    p().surface_alt
                } else if resp.hovered() || drop_target {
                    p().surface_muted
                } else {
                    p().bg
                };
                painter.rect_filled(rect, 0, bg);
                let fg = if resp.hovered() || drop_target { p().text } else { p().text_muted };
                paint_text(&painter, ui, rect.center() - Vec2::new(0.0, 6.0), Align2::CENTER_BOTTOM, icon(ph::PLUS, 22.0, fg));
                let label = if drop_target { "Soltar aqui" } else { "Adicionar som" };
                paint_text(&painter, ui, rect.center() + Vec2::new(0.0, 6.0), Align2::CENTER_TOP, caps(label, 9.0, fg));
                if let Some(k) = &key {
                    theme::key_cap(&painter, ui, rect.min + Vec2::splat(pad), k, p().border, p().text_muted);
                }
            }
            Some(slot) => {
                let playing = progress.is_some();
                let bg = if playing {
                    VIOLET
                } else if pressed {
                    p().surface_alt
                } else if resp.hovered() || drop_target {
                    p().surface_muted
                } else {
                    p().bg
                };
                painter.rect_filled(rect, 0, bg);
                let soft = if playing { Color32::from_white_alpha(200) } else { p().text_variant };
                let faint = if playing { Color32::from_white_alpha(110) } else { p().border };

                if let Some(k) = &key {
                    theme::key_cap(&painter, ui, rect.min + Vec2::splat(pad), k, faint, soft);
                }

                // Indicadores no canto superior direito.
                let mut flags = String::new();
                if slot.looping {
                    flags.push_str(ph::REPEAT);
                }
                if (slot.volume - 1.0).abs() > 0.01 {
                    if !flags.is_empty() {
                        flags.push_str("  ");
                    }
                    flags.push_str(if slot.volume < 1.0 { ph::SPEAKER_LOW } else { ph::SPEAKER_HIGH });
                }
                if !flags.is_empty() {
                    paint_text(&painter, ui, Pos2::new(rect.right() - pad, rect.top() + pad + 2.0), Align2::RIGHT_TOP, icon(&flags, 15.0, soft));
                }

                // Metadados (duração · formato) ou estado.
                let (meta, meta_color) = if self.loading.contains(&i) {
                    ("Carregando…".to_string(), soft)
                } else if let Some(e) = self.load_errors.get(&i) {
                    let msg = if slot.path.exists() { e.clone() } else { "Arquivo não encontrado".into() };
                    (msg, p().danger)
                } else {
                    let ext = slot.path.extension().and_then(|e| e.to_str()).unwrap_or("").to_uppercase();
                    let dur = self.clips.get(&i).map(|c| c.frames() as f32 / c.sample_rate as f32);
                    match (dur, progress) {
                        (Some(d), Some(p)) => (format!("{} / {}", fmt_time(d * p), fmt_time(d)), soft),
                        (Some(d), None) => (format!("{} · {ext}", fmt_time(d)), soft),
                        _ => (ext, soft),
                    }
                };
                let mut meta_job = caps(&meta, 9.0, meta_color);
                meta_job.wrap.max_width = rect.width() - pad * 2.0 - 28.0;
                meta_job.wrap.max_rows = 1;
                let meta_rect = paint_text(&painter, ui, Pos2::new(rect.left() + pad, rect.bottom() - pad - 2.0), Align2::LEFT_BOTTOM, meta_job);

                // Nome, até duas linhas.
                let size = if rect.width() > 190.0 { 21.0 } else { 17.0 };
                let fg = if playing { Color32::WHITE } else { p().text };
                let mut name: LayoutJob = display(&slot.name, size, fg);
                name.wrap.max_width = rect.width() - pad * 2.0;
                name.wrap.max_rows = 2;
                paint_text(&painter, ui, Pos2::new(rect.left() + pad, meta_rect.top() - 6.0), Align2::LEFT_BOTTOM, name);

                // Ação no canto inferior direito.
                let action = if playing {
                    Some(icon_fill(ph::STOP, 16.0, Color32::WHITE))
                } else if resp.hovered() {
                    Some(icon_fill(ph::PLAY, 16.0, fg))
                } else {
                    None
                };
                if let Some(job) = action {
                    paint_text(&painter, ui, Pos2::new(rect.right() - pad, rect.bottom() - pad), Align2::RIGHT_BOTTOM, job);
                }

                if let Some(frac) = progress {
                    let bar = Rect::from_min_max(Pos2::new(rect.left(), rect.bottom() - 2.0), rect.max);
                    paint_bar(&painter, bar, frac, Color32::from_white_alpha(60), Color32::WHITE);
                }
            }
        }
        if drop_target {
            painter.rect_stroke(rect, 0, Stroke::new(1.0_f32, p().text), egui::StrokeKind::Inside);
        }

        if resp.clicked() {
            actions.push(if self.config.slots[i].is_some() { Action::Trigger(i) } else { Action::Pick(i) });
        }

        let mut dirty = false;
        resp.context_menu(|ui| {
            ui.set_min_width(260.0);
            match self.config.slots[i].as_mut() {
                Some(slot) => {
                    ui.add_space(4.0);
                    ui.label(eyebrow(&format!("Pad {:02} · página {}", i % PAGE_SIZE + 1, i / PAGE_SIZE + 1)));
                    ui.add_space(2.0);
                    dirty |= ui
                        .add(egui::TextEdit::singleline(&mut slot.name).desired_width(f32::INFINITY).margin(Vec2::new(10.0, 8.0)))
                        .changed();
                    ui.add_space(6.0);
                    dirty |= theme::gain(ui, "Volume", &mut slot.volume, 2.0);
                    ui.add_space(4.0);
                    dirty |= ui.checkbox(&mut slot.looping, "Repetir em loop").changed();
                    dirty |= ui.checkbox(&mut slot.restart, "Clique reinicia em vez de parar").changed();
                    dirty |= ui.checkbox(&mut slot.pinned, "Mostrar na barra de menus").changed();
                    ui.separator();
                    let key_label = key.as_deref().unwrap_or("nenhuma");
                    if ui.button(icon_text(ph::KEYBOARD, &format!("Tecla de atalho: {key_label}"), p().text)).clicked() {
                        actions.push(Action::CaptureKey(i));
                        ui.close_menu();
                    }
                    if ui.button(icon_text(ph::FOLDER_OPEN, "Trocar arquivo…", p().text)).clicked() {
                        actions.push(Action::Pick(i));
                        ui.close_menu();
                    }
                    ui.separator();
                    if ui.button(icon_text(ph::TRASH, "Remover", p().danger)).clicked() {
                        actions.push(Action::Remove(i));
                        ui.close_menu();
                    }
                    ui.add_space(2.0);
                    ui.label(egui::RichText::new(slot.path.display().to_string()).size(10.5).color(p().text_muted));
                }
                None => {
                    if ui.button(icon_text(ph::FOLDER_OPEN, "Escolher arquivo…", p().text)).clicked() {
                        actions.push(Action::Pick(i));
                        ui.close_menu();
                    }
                }
            }
        });
        self.dirty |= dirty;
    }
}

fn fmt_time(secs: f32) -> String {
    let s = secs.max(0.0).round() as u32;
    format!("{}:{:02}", s / 60, s % 60)
}

/// Seta do paginador; apagada quando não há para onde ir.
fn pager_arrow(ui: &mut egui::Ui, glyph: &str, enabled: bool, tip: &str) -> bool {
    if enabled {
        return icon_button(ui, glyph, 28.0, None).on_hover_text(tip).clicked();
    }
    let (rect, _) = ui.allocate_exact_size(Vec2::splat(28.0), Sense::hover());
    let painter = ui.painter();
    painter.rect_stroke(rect, 0, Stroke::new(1.0_f32, p().border), egui::StrokeKind::Inside);
    paint_text(painter, ui, rect.center(), Align2::CENTER_CENTER, icon(glyph, 14.0, p().text_muted));
    false
}
