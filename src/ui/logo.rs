//! Marca do bSound desenhada com formas do egui (mesma geometria de branding/build_brand.py).

use eframe::egui::{self, Color32, CornerRadius, FontFamily, FontId, Pos2, Rect, Sense, Shape, Stroke, Vec2};

pub const VIOLET: Color32 = Color32::from_rgb(108, 77, 255);
pub const CORAL: Color32 = Color32::from_rgb(255, 92, 77);

// Bbox do símbolo no grid de 256.
const MIN_X: f32 = 24.0;
const MIN_Y: f32 = 18.0;
const W: f32 = 208.0;
const H: f32 = 226.48;

/// Pinta o símbolo dentro de `rect`, mantendo a proporção e centralizando.
pub fn paint_symbol(painter: &egui::Painter, rect: Rect) {
    let s = (rect.width() / W).min(rect.height() / H);
    let origin = rect.center() - Vec2::new(W, H) * s / 2.0;
    let p = |x: f32, y: f32| Pos2::new(origin.x + (x - MIN_X) * s, origin.y + (y - MIN_Y) * s);
    let r = (24.0 * s).round().min(255.0) as u8;
    let pad = |x: f32, y: f32| Rect::from_min_max(p(x, y), p(x + 96.0, y + 96.0));

    painter.rect_filled(pad(24.0, 18.0), r, VIOLET);
    painter.rect_filled(pad(136.0, 130.0), r, VIOLET);
    let bubble = CornerRadius { nw: r, ne: r, sw: 0, se: r };
    painter.rect_filled(pad(24.0, 130.0), bubble, CORAL);
    painter.add(Shape::convex_polygon(
        vec![p(24.0, 225.0), p(56.0, 225.0), p(24.0, 244.48)],
        CORAL,
        Stroke::NONE,
    ));
}

/// Símbolo + nome "bSound", para o topo da janela.
pub fn header(ui: &mut egui::Ui, height: f32) {
    ui.horizontal(|ui| {
        let (rect, _) = ui.allocate_exact_size(Vec2::new(height * W / H, height), Sense::hover());
        paint_symbol(ui.painter(), rect);
        ui.add_space(height * 0.25);
        ui.label(
            egui::RichText::new("bSound")
                .font(FontId::new(height * 0.82, FontFamily::Name("bold".into())))
                .color(ui.visuals().strong_text_color()),
        );
    });
}
