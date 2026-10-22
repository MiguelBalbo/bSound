//! Popover da barra de menus: os sons em pads, no visual do app.
//! É uma janela sem bordas (viewport do egui) aberta logo abaixo do ícone.

use std::time::Instant;

use eframe::egui::{self, text::LayoutJob, Align, Align2, Color32, CursorIcon, Layout, Pos2, Rect, Sense, Stroke, Vec2};
use egui_phosphor::light as ph;

use crate::app::SoundboardApp;
use crate::audio::mixer::Command;
use crate::audio::shared::PAGE_SIZE;
use crate::ui::logo;
use crate::ui::theme::*;

const COLS: usize = 3;
const CELL: f32 = 96.0;
const PAD: f32 = 16.0;

pub struct Popover {
    /// Centro horizontal, topo e base do ícone, em pontos (`None` se o sistema não informa).
    anchor: Option<(f32, f32, f32)>,
    screen: Vec2,
    had_focus: bool,
    opened: Instant,
    /// Diferença entre onde pedimos a janela e onde o sistema a colocou (corrigida uma vez).
    offset: Option<Vec2>,
}

impl Popover {
    pub fn new(anchor: Option<(f32, f32, f32)>, screen: Vec2) -> Self {
        Self { anchor, screen, had_focus: false, opened: Instant::now(), offset: None }
    }

    /// Abaixo do ícone se ele está no topo da tela (macOS), acima se está embaixo (barra de
    /// tarefas do Windows); sem posição conhecida, no canto superior direito.
    fn position(&self, size: Vec2) -> Pos2 {
        let margin = 8.0;
        let (x, y) = match self.anchor {
            Some((cx, top, bottom)) => {
                let y = if bottom < self.screen.y / 2.0 { bottom + 6.0 } else { top - size.y - 6.0 };
                (cx - size.x / 2.0, y)
            }
            None => (self.screen.x - size.x - margin, 40.0),
        };
        let max_x = (self.screen.x - size.x - margin).max(margin);
        Pos2::new(x.clamp(margin, max_x), y.max(0.0))
    }
}

struct PadInfo {
    slot: usize,
    name: String,
    progress: Option<f32>,
    plays: u32,
}

enum PopAction {
    Trigger(usize),
    StopAll,
    OpenMain,
    Close,
}

impl SoundboardApp {
    pub(crate) fn popover_ui(&mut self, ctx: &egui::Context) {
        let Some(pop) = &self.popover else { return };
        let (title, entries) = self.tray_entries();
        let pads: Vec<PadInfo> = entries
            .into_iter()
            .map(|(slot, name)| PadInfo { slot, name, progress: self.shared.progress(slot), plays: self.stats.get(slot) })
            .collect();

        let rows = pads.len().div_ceil(COLS).max(1);
        let grid = Vec2::new(COLS as f32 * CELL + (COLS + 1) as f32, rows as f32 * CELL + (rows + 1) as f32);
        let size = Vec2::new(grid.x + PAD * 2.0, PAD + 22.0 + 14.0 + grid.y + 14.0 + 36.0 + PAD);
        let wanted = pop.position(size);
        let pos = wanted - pop.offset.unwrap_or_default();
        let needs_offset = pop.offset.is_none();
        let anchor_age = pop.opened.elapsed().as_secs_f32();

        let builder = egui::ViewportBuilder::default()
            .with_title("bSound")
            .with_decorations(false)
            .with_resizable(false)
            .with_always_on_top()
            .with_taskbar(false)
            .with_inner_size(size)
            .with_position(pos);

        let mut actions = Vec::new();
        let mut focused_now = None;
        let mut measured_offset = None;
        ctx.show_viewport_immediate(egui::ViewportId::from_hash_of("bsound-popover"), builder, |ctx, _| {
            let frame = egui::Frame::default()
                .fill(p().bg)
                .stroke(Stroke::new(1.0_f32, p().border))
                .inner_margin(egui::Margin::same(PAD as i8));
            egui::CentralPanel::default().frame(frame).show(ctx, |ui| {
                ui.spacing_mut().item_spacing.y = 0.0;
                // Cabeçalho: marca + título da lista.
                ui.horizontal(|ui| {
                    let (r, _) = ui.allocate_exact_size(Vec2::new(16.0, 18.0), Sense::hover());
                    logo::paint_symbol(ui.painter(), r);
                    ui.label(display("bSound", 16.0, p().text));
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        ui.label(caps(title, 8.5, p().text_variant));
                    });
                });
                ui.add_space(14.0);

                let (grid_rect, _) = ui.allocate_exact_size(grid, Sense::hover());
                ui.painter().rect_filled(grid_rect, 0, p().border);
                for k in 0..rows * COLS {
                    let (r, c) = (k / COLS, k % COLS);
                    let min = grid_rect.min + Vec2::new(1.0 + c as f32 * (CELL + 1.0), 1.0 + r as f32 * (CELL + 1.0));
                    let rect = Rect::from_min_size(min, Vec2::splat(CELL));
                    match pads.get(k) {
                        Some(pad) => {
                            if pad_cell(ui, rect, pad) {
                                actions.push(PopAction::Trigger(pad.slot));
                            }
                        }
                        None => {
                            ui.painter().rect_filled(rect, 0, p().bg);
                            if pads.is_empty() && k == 1 {
                                paint_text(ui.painter(), ui, rect.center(), Align2::CENTER_CENTER, caps("Nenhum som", 8.5, p().text_muted));
                            }
                        }
                    }
                }

                ui.add_space(14.0);
                ui.horizontal(|ui| {
                    if button(ui, Some(ph::STOP_CIRCLE), "Parar tudo", Kind::Secondary).clicked() {
                        actions.push(PopAction::StopAll);
                    }
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if button(ui, Some(ph::ARROW_SQUARE_OUT), "Abrir bSound", Kind::Ghost).clicked() {
                            actions.push(PopAction::OpenMain);
                        }
                    });
                });
            });
            let (focused, esc, close) = ctx.input(|i| (i.viewport().focused, i.key_pressed(egui::Key::Escape), i.viewport().close_requested()));
            focused_now = focused;
            // O sistema pode deslocar janelas sem borda; mede onde ela ficou e compensa uma vez.
            if needs_offset {
                if let Some(outer) = ctx.input(|i| i.viewport().outer_rect) {
                    let delta = outer.min - wanted;
                    measured_offset = Some(delta);
                    if delta.length() > 1.0 {
                        ctx.send_viewport_cmd(egui::ViewportCommand::OuterPosition(wanted - delta));
                    }
                }
            }
            if esc || close {
                actions.push(PopAction::Close);
            }
            // Pedido de foco nos primeiros instantes (o app acabou de ser ativado pelo clique no ícone).
            if anchor_age < 0.3 && focused != Some(true) {
                ctx.send_viewport_cmd(egui::ViewportCommand::Focus);
            }
        });

        // Fecha como um menu: clique na janela principal, outro app ativado ou perda de foco.
        let clicked_main = ctx.input(|i| i.pointer.any_pressed());
        let grace = anchor_age > 0.4;
        if grace && (clicked_main || !crate::tray::app_is_active()) {
            actions.push(PopAction::Close);
        }
        if let Some(pop) = &mut self.popover {
            if measured_offset.is_some() {
                pop.offset = measured_offset;
            }
            match focused_now {
                Some(true) => pop.had_focus = true,
                Some(false) if pop.had_focus => actions.push(PopAction::Close),
                _ => {}
            }
        }

        for a in actions {
            match a {
                PopAction::Trigger(slot) => self.trigger(slot),
                PopAction::StopAll => self.engine.send(Command::StopAll),
                PopAction::OpenMain => {
                    self.popover = None;
                    self.show_main_window(ctx);
                }
                PopAction::Close => self.popover = None,
            }
        }
    }
}

/// Um pad do popover. Retorna true se foi clicado.
fn pad_cell(ui: &mut egui::Ui, rect: Rect, pad: &PadInfo) -> bool {
    let resp = ui
        .interact(rect, ui.id().with(("pop-pad", pad.slot)), Sense::click())
        .on_hover_cursor(CursorIcon::PointingHand);
    let painter = ui.painter_at(rect);
    let playing = pad.progress.is_some();
    let bg = if playing {
        VIOLET
    } else if resp.is_pointer_button_down_on() {
        p().surface_alt
    } else if resp.hovered() {
        p().surface_muted
    } else {
        p().bg
    };
    painter.rect_filled(rect, 0, bg);
    let fg = if playing { Color32::WHITE } else { p().text };
    let soft = if playing { Color32::from_white_alpha(200) } else { p().text_variant };
    let inset = 10.0;

    paint_text(
        &painter,
        ui,
        rect.min + Vec2::splat(inset),
        Align2::LEFT_TOP,
        caps(&format!("Pág {}", pad.slot / PAGE_SIZE + 1), 7.5, soft),
    );
    let glyph = if playing {
        Some(icon_fill(ph::STOP, 12.0, Color32::WHITE))
    } else if resp.hovered() {
        Some(icon_fill(ph::PLAY, 12.0, fg))
    } else {
        None
    };
    if let Some(job) = glyph {
        paint_text(&painter, ui, Pos2::new(rect.right() - inset, rect.top() + inset), Align2::RIGHT_TOP, job);
    }

    let meta = if pad.plays > 0 { format!("{}×", pad.plays) } else { String::new() };
    let meta_rect = paint_text(&painter, ui, Pos2::new(rect.left() + inset, rect.bottom() - inset), Align2::LEFT_BOTTOM, caps(&meta, 7.5, soft));
    let mut name: LayoutJob = display(&pad.name, 14.0, fg);
    name.wrap.max_width = rect.width() - inset * 2.0;
    name.wrap.max_rows = 3;
    let name_bottom = if meta.is_empty() { rect.bottom() - inset } else { meta_rect.top() - 4.0 };
    paint_text(&painter, ui, Pos2::new(rect.left() + inset, name_bottom), Align2::LEFT_BOTTOM, name);

    if let Some(frac) = pad.progress {
        let bar = Rect::from_min_max(Pos2::new(rect.left(), rect.bottom() - 2.0), rect.max);
        paint_bar(&painter, bar, frac, Color32::from_white_alpha(60), Color32::WHITE);
    }
    resp.clicked()
}
