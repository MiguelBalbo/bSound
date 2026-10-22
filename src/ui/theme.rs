//! Sistema visual do bSound: base monocromática editorial (linhas finas, cantos retos,
//! caixa-alta espaçada) com as duas cores da marca reservadas para estados de áudio:
//! violeta = pad tocando, coral = microfone/voz.

use std::sync::atomic::{AtomicBool, Ordering::Relaxed};
use std::sync::Arc;

use eframe::egui::{
    self, text::LayoutJob, Align2, Color32, CornerRadius, CursorIcon, FontData, FontDefinitions, FontFamily, FontId,
    Rect, Response, Sense, Stroke, TextFormat, TextStyle, Ui, Vec2,
};

pub use crate::config::ThemeMode;
pub use crate::ui::logo::{CORAL, VIOLET};

const fn hex(v: u32) -> Color32 {
    Color32::from_rgb((v >> 16) as u8, (v >> 8) as u8, v as u8)
}

/// Cores neutras e semânticas de um tema. As cores da marca (VIOLET/CORAL) são fixas.
pub struct Palette {
    pub bg: Color32,
    pub surface_muted: Color32,
    pub surface_alt: Color32,
    pub inset: Color32,
    pub text: Color32,
    pub text_variant: Color32,
    pub text_muted: Color32,
    pub border: Color32,
    pub success: Color32,
    pub warning: Color32,
    pub danger: Color32,
}

pub const DARK: Palette = Palette {
    bg: hex(0x121212),
    surface_muted: hex(0x1A1A1A),
    surface_alt: hex(0x242424),
    inset: hex(0x0D0D0D),
    text: hex(0xFAFAFA),
    text_variant: hex(0x8E8E8E),
    text_muted: hex(0x525252),
    border: hex(0x2E2E2E),
    success: hex(0x22C55E),
    warning: hex(0xF5A524),
    danger: hex(0xF05252),
};

pub const LIGHT: Palette = Palette {
    bg: hex(0xF6F6F4),
    surface_muted: hex(0xEDEDEA),
    surface_alt: hex(0xE3E3DF),
    inset: hex(0xFFFFFF),
    text: hex(0x141414),
    text_variant: hex(0x6B6B6B),
    text_muted: hex(0xA3A3A0),
    border: hex(0xDADAD6),
    success: hex(0x16A34A),
    warning: hex(0xB86E00),
    danger: hex(0xD92D2D),
};

static IS_DARK: AtomicBool = AtomicBool::new(true);

/// Paleta do tema em uso (atualizada no início de cada frame).
pub fn p() -> &'static Palette {
    if IS_DARK.load(Relaxed) {
        &DARK
    } else {
        &LIGHT
    }
}

pub fn sync(ctx: &egui::Context) {
    IS_DARK.store(ctx.style().visuals.dark_mode, Relaxed);
}

/// Aplica o modo escolhido. "Sistema" é resolvido lendo a aparência do macOS diretamente:
/// o tema que o winit repassa ao egui pode ficar desatualizado depois de trocar a preferência.
pub fn set_mode(ctx: &egui::Context, mode: ThemeMode) {
    let dark = match mode {
        ThemeMode::System => system_is_dark(),
        ThemeMode::Light => false,
        ThemeMode::Dark => true,
    };
    let pref = if dark { egui::ThemePreference::Dark } else { egui::ThemePreference::Light };
    if ctx.options(|o| o.theme_preference) != pref {
        ctx.set_theme(pref);
    }
}

pub fn system_is_dark() -> bool {
    use objc2_foundation::{ns_string, NSUserDefaults};
    NSUserDefaults::standardUserDefaults()
        .stringForKey(ns_string!("AppleInterfaceStyle"))
        .is_some_and(|s| s.to_string().eq_ignore_ascii_case("dark"))
}

pub fn family(name: &str) -> FontFamily {
    FontFamily::Name(name.into())
}

pub fn fonts() -> FontDefinitions {
    let mut fonts = FontDefinitions::default();
    let data: [(&str, &'static [u8]); 6] = [
        ("inter", include_bytes!("../../assets/fonts/Inter-Regular.ttf")),
        ("inter-medium", include_bytes!("../../assets/fonts/Inter-Medium.ttf")),
        ("inter-semibold", include_bytes!("../../assets/fonts/Inter-SemiBold.ttf")),
        ("inter-bold", include_bytes!("../../assets/fonts/Inter-Bold.ttf")),
        ("inter-display", include_bytes!("../../assets/fonts/Inter-Display.ttf")),
        ("phosphor-fill", egui_phosphor::Variant::Fill.font_bytes()),
    ];
    for (name, bytes) in data {
        fonts.font_data.insert(name.into(), Arc::new(FontData::from_static(bytes)));
    }
    fonts
        .font_data
        .insert("phosphor".into(), Arc::new(egui_phosphor::Variant::Light.font_data()));

    // Inter na frente para texto. Ícones usam a família "icon" (Phosphor primeiro): a Inter tem
    // glifos próprios em códigos que o Phosphor usa, e o Phosphor tem algumas letras.
    let prop = fonts.families.entry(FontFamily::Proportional).or_default();
    prop.insert(0, "inter".into());
    let fallback = fonts.families[&FontFamily::Proportional].clone();
    for (name, first) in [
        ("display", "inter-display"),
        ("label", "inter-semibold"),
        ("medium", "inter-medium"),
        ("bold", "inter-bold"),
        ("fill", "phosphor-fill"),
        ("icon", "phosphor"),
    ] {
        let mut list = vec![first.to_string()];
        list.extend(fallback.iter().cloned());
        fonts.families.insert(family(name), list);
    }
    fonts
}

pub fn apply(ctx: &egui::Context, mode: ThemeMode) {
    ctx.set_fonts(fonts());
    for (theme, pal) in [(egui::Theme::Dark, &DARK), (egui::Theme::Light, &LIGHT)] {
        ctx.style_mut_of(theme, |style| style_for(style, pal, theme == egui::Theme::Dark));
    }
    set_mode(ctx, mode);
    sync(ctx);
}

fn style_for(style: &mut egui::Style, pal: &Palette, dark: bool) {
    style.text_styles = [
        (TextStyle::Small, FontId::new(11.0, FontFamily::Proportional)),
        (TextStyle::Body, FontId::new(13.0, FontFamily::Proportional)),
        (TextStyle::Button, FontId::new(13.0, FontFamily::Proportional)),
        (TextStyle::Heading, FontId::new(22.0, family("display"))),
        (TextStyle::Monospace, FontId::new(12.0, FontFamily::Monospace)),
    ]
    .into();

    let s = &mut style.spacing;
    s.item_spacing = Vec2::new(8.0, 8.0);
    s.button_padding = Vec2::new(12.0, 7.0);
    s.interact_size.y = 30.0;
    s.slider_rail_height = 2.0;
    s.combo_height = 320.0;
    s.menu_margin = egui::Margin::same(6);
    s.window_margin = egui::Margin::same(28);

    let v = &mut style.visuals;
    v.dark_mode = dark;
    v.panel_fill = pal.bg;
    v.window_fill = pal.bg;
    v.extreme_bg_color = pal.inset;
    v.faint_bg_color = pal.surface_muted;
    v.code_bg_color = pal.surface_alt;
    v.window_stroke = Stroke::new(1.0_f32, pal.border);
    v.window_shadow = egui::Shadow::NONE;
    v.popup_shadow = egui::Shadow::NONE;
    v.window_corner_radius = CornerRadius::ZERO;
    v.menu_corner_radius = CornerRadius::ZERO;
    v.hyperlink_color = pal.text;
    v.warn_fg_color = pal.warning;
    v.error_fg_color = pal.danger;
    v.handle_shape = egui::style::HandleShape::Rect { aspect_ratio: 0.5 };
    v.slider_trailing_fill = true;
    v.striped = false;
    // Seleção = troca de preenchimento (branco sobre preto), como na referência.
    v.selection.bg_fill = pal.text;
    v.selection.stroke = Stroke::new(1.0_f32, pal.bg);
    v.text_cursor.stroke = Stroke::new(1.5_f32, pal.text);

    let w = &mut v.widgets;
    w.noninteractive.bg_fill = pal.bg;
    w.noninteractive.weak_bg_fill = pal.bg;
    w.noninteractive.bg_stroke = Stroke::new(1.0_f32, pal.border);
    w.noninteractive.fg_stroke = Stroke::new(1.0_f32, if dark { hex(0xC4C4C4) } else { hex(0x3A3A3A) });

    w.inactive.bg_fill = pal.border; // trilho do slider, fundo do checkbox
    w.inactive.weak_bg_fill = pal.bg; // fundo de botões e combos
    w.inactive.bg_stroke = Stroke::new(1.0_f32, pal.border);
    w.inactive.fg_stroke = Stroke::new(1.0_f32, pal.text);

    w.hovered.bg_fill = pal.text;
    w.hovered.weak_bg_fill = pal.surface_muted;
    w.hovered.bg_stroke = Stroke::new(1.0_f32, pal.text_variant);
    w.hovered.fg_stroke = Stroke::new(1.0_f32, pal.text);

    w.active.bg_fill = pal.text;
    w.active.weak_bg_fill = pal.surface_alt;
    w.active.bg_stroke = Stroke::new(1.0_f32, pal.text);
    w.active.fg_stroke = Stroke::new(1.0_f32, pal.text);

    w.open.bg_fill = pal.surface_alt;
    w.open.weak_bg_fill = pal.surface_muted;
    w.open.bg_stroke = Stroke::new(1.0_f32, pal.text_variant);
    w.open.fg_stroke = Stroke::new(1.0_f32, pal.text);

    for state in [&mut w.noninteractive, &mut w.inactive, &mut w.hovered, &mut w.active, &mut w.open] {
        state.corner_radius = CornerRadius::ZERO;
        state.expansion = 0.0;
    }
}

// ---------- Tipografia ----------

fn format(size: f32, family: FontFamily, color: Color32, tracking: f32) -> TextFormat {
    TextFormat {
        font_id: FontId::new(size, family),
        color,
        extra_letter_spacing: tracking,
        ..Default::default()
    }
}

/// Rótulo em caixa-alta com tracking largo (0.2em).
pub fn caps(text: &str, size: f32, color: Color32) -> LayoutJob {
    LayoutJob::single_section(text.to_uppercase(), format(size, family("label"), color, size * 0.2))
}

/// Eyebrow: caixa-alta ainda mais aberta (0.4em), acima de títulos.
pub fn eyebrow(text: &str) -> LayoutJob {
    LayoutJob::single_section(text.to_uppercase(), format(10.0, family("medium"), p().text_variant, 4.0))
}

/// Título com tracking negativo.
pub fn display(text: &str, size: f32, color: Color32) -> LayoutJob {
    LayoutJob::single_section(text.to_owned(), format(size, family("display"), color, -size * 0.03))
}

pub fn icon(glyph: &str, size: f32, color: Color32) -> LayoutJob {
    LayoutJob::single_section(glyph.to_owned(), format(size, family("icon"), color, 0.0))
}

/// Ícone seguido de texto comum (itens de menu, avisos).
pub fn icon_text(glyph: &str, text: &str, color: Color32) -> LayoutJob {
    let mut job = LayoutJob::default();
    job.append(glyph, 0.0, format(15.0, family("icon"), color, 0.0));
    job.append(text, 10.0, format(13.0, FontFamily::Proportional, color, 0.0));
    job
}

pub fn icon_fill(glyph: &str, size: f32, color: Color32) -> LayoutJob {
    LayoutJob::single_section(glyph.to_owned(), format(size, family("fill"), color, 0.0))
}

/// Ícone + rótulo em caixa-alta (+ ícone final opcional); cor PLACEHOLDER para recolorir ao pintar.
fn icon_caps(glyph: Option<&str>, text: &str, size: f32, trailing: Option<&str>) -> LayoutJob {
    let mut job = LayoutJob::default();
    if let Some(g) = glyph {
        job.append(g, 0.0, format(size + 5.0, family("icon"), Color32::PLACEHOLDER, 0.0));
        job.append(" ", 0.0, format(size, family("label"), Color32::PLACEHOLDER, 0.0));
    }
    if !text.is_empty() {
        job.append(&text.to_uppercase(), 0.0, format(size, family("label"), Color32::PLACEHOLDER, size * 0.2));
    }
    if let Some(g) = trailing {
        job.append(g, 6.0, format(size + 5.0, family("icon"), Color32::PLACEHOLDER, 0.0));
    }
    job
}

// ---------- Componentes ----------

#[derive(Clone, Copy, PartialEq)]
pub enum Kind {
    /// Bloco branco com texto escuro; no hover vira contorno.
    Primary,
    /// Contorno fino; no hover inverte (branco).
    Secondary,
    /// Só texto, sem moldura.
    Ghost,
}

pub fn button(ui: &mut Ui, glyph: Option<&str>, text: &str, kind: Kind) -> Response {
    button_trailing(ui, glyph, text, None, kind)
}

/// Botão com ícone final (ex.: seta), como o button-text da referência.
pub fn button_trailing(ui: &mut Ui, glyph: Option<&str>, text: &str, trailing: Option<&str>, kind: Kind) -> Response {
    let galley = ui.fonts(|f| f.layout_job(icon_caps(glyph, text, 10.0, trailing)));
    let pad = match kind {
        Kind::Ghost => Vec2::new(2.0, 6.0),
        _ => Vec2::new(16.0, 10.0),
    };
    let size = galley.size() + pad * 2.0;
    let (rect, resp) = ui.allocate_exact_size(size, Sense::click());
    let hovered = resp.hovered();
    let (fill, stroke, fg) = match (kind, hovered) {
        (Kind::Primary, false) => (p().text, p().text, p().bg),
        (Kind::Primary, true) => (p().bg, p().text, p().text),
        (Kind::Secondary, false) => (p().bg, p().border, p().text),
        (Kind::Secondary, true) => (p().text, p().text, p().bg),
        (Kind::Ghost, false) => (Color32::TRANSPARENT, Color32::TRANSPARENT, p().text_variant),
        (Kind::Ghost, true) => (Color32::TRANSPARENT, Color32::TRANSPARENT, p().text),
    };
    let painter = ui.painter();
    painter.rect_filled(rect, 0, fill);
    if stroke != Color32::TRANSPARENT {
        painter.rect_stroke(rect, 0, Stroke::new(1.0_f32, stroke), egui::StrokeKind::Inside);
    }
    painter.galley(rect.center() - galley.size() / 2.0, galley, fg);
    resp.on_hover_cursor(CursorIcon::PointingHand)
}

/// Botão quadrado com um ícone. `active` = preenchido com `accent`.
pub fn icon_button(ui: &mut Ui, glyph: &str, side: f32, active: Option<Color32>) -> Response {
    let (rect, resp) = ui.allocate_exact_size(Vec2::splat(side), Sense::click());
    let painter = ui.painter();
    let hovered = resp.hovered();
    let (fill, stroke, fg, job) = match active {
        Some(accent) => (accent, accent, Color32::WHITE, icon_fill(glyph, side * 0.5, Color32::PLACEHOLDER)),
        None if hovered => (p().text, p().text, p().bg, icon(glyph, side * 0.5, Color32::PLACEHOLDER)),
        None => (p().bg, p().border, p().text, icon(glyph, side * 0.5, Color32::PLACEHOLDER)),
    };
    painter.rect_filled(rect, 0, fill);
    painter.rect_stroke(rect, 0, Stroke::new(1.0_f32, stroke), egui::StrokeKind::Inside);
    let galley = ui.fonts(|f| f.layout_job(job));
    painter.galley(rect.center() - galley.size() / 2.0, galley, fg);
    resp.on_hover_cursor(CursorIcon::PointingHand)
}

/// Medidor de nível em segmentos (lê como instrumento, não como mais uma linha divisória).
/// Os últimos segmentos acendem em coral para indicar que está perto de clipar.
pub fn meter(ui: &mut Ui, level: f32, color: Color32) {
    const SEGMENTS: usize = 32;
    const GAP: f32 = 2.0;
    let (rect, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 4.0), Sense::hover());
    let seg_w = (rect.width() - GAP * (SEGMENTS - 1) as f32) / SEGMENTS as f32;
    let lit = (level.clamp(0.0, 1.0) * SEGMENTS as f32).round() as usize;
    let painter = ui.painter();
    for k in 0..SEGMENTS {
        let x = rect.left() + k as f32 * (seg_w + GAP);
        let r = Rect::from_min_size(egui::pos2(x, rect.top()), Vec2::new(seg_w, rect.height()));
        let hot = k >= SEGMENTS - 3;
        let c = if k < lit {
            if hot { CORAL } else { color }
        } else {
            p().surface_alt
        };
        painter.rect_filled(r, 0, c);
    }
}

pub fn paint_bar(painter: &egui::Painter, rect: Rect, level: f32, track: Color32, fill: Color32) {
    painter.rect_filled(rect, 0, track);
    let level = level.clamp(0.0, 1.0);
    if level > 0.002 {
        let r = Rect::from_min_size(rect.min, Vec2::new(rect.width() * level, rect.height()));
        painter.rect_filled(r, 0, fill);
    }
}

/// Faixa segmentada: itens divididos por linhas finas, o ativo preenchido de branco.
pub fn segmented<T: PartialEq + Copy>(ui: &mut Ui, value: &mut T, options: &[(T, &str)]) -> bool {
    let mut changed = false;
    let galleys: Vec<_> = options
        .iter()
        .map(|(_, label)| ui.fonts(|f| f.layout_job(caps(label, 10.0, Color32::PLACEHOLDER))))
        .collect();
    let item_w = galleys.iter().map(|g| g.size().x).fold(0.0, f32::max) + 28.0;
    let h = 34.0;
    let (rect, strip) = ui.allocate_exact_size(Vec2::new(item_w * options.len() as f32, h), Sense::hover());
    for (k, ((opt, _), galley)) in options.iter().zip(galleys).enumerate() {
        let r = Rect::from_min_size(rect.min + Vec2::new(item_w * k as f32, 0.0), Vec2::new(item_w, h));
        let resp = ui.interact(r, strip.id.with(k), Sense::click()).on_hover_cursor(CursorIcon::PointingHand);
        let active = *value == *opt;
        let painter = ui.painter();
        let (fill, fg) = if active {
            (p().text, p().bg)
        } else if resp.hovered() {
            (p().surface_muted, p().text)
        } else {
            (p().bg, p().text_variant)
        };
        painter.rect_filled(r, 0, fill);
        if k > 0 {
            painter.vline(r.left(), r.y_range(), Stroke::new(1.0_f32, p().border));
        }
        painter.galley(r.center() - galley.size() / 2.0, galley, fg);
        if resp.clicked() && !active {
            *value = *opt;
            changed = true;
        }
    }
    ui.painter().rect_stroke(rect, 0, Stroke::new(1.0_f32, p().border), egui::StrokeKind::Inside);
    changed
}

/// Linha fina horizontal ocupando a largura disponível.
pub fn hairline(ui: &mut Ui) {
    let (rect, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 1.0), Sense::hover());
    ui.painter().hline(rect.x_range(), rect.center().y, Stroke::new(1.0_f32, p().border));
}

/// Rótulo + valor em % + slider de 2 px na largura toda. Retorna true se mudou.
pub fn gain(ui: &mut Ui, label: &str, value: &mut f32, max: f32) -> bool {
    ui.horizontal(|ui| {
        ui.label(caps(label, 10.0, p().text_variant));
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.label(display(&format!("{:.0}%", *value * 100.0), 15.0, p().text));
        });
    });
    ui.spacing_mut().slider_width = ui.available_width();
    let changed = ui
        .add(egui::Slider::new(value, 0.0..=max).show_value(false))
        .on_hover_cursor(CursorIcon::ResizeHorizontal)
        .changed();
    changed
}

/// Ponto de status (o único elemento redondo, como o badge-circle da referência).
pub fn status_dot(ui: &mut Ui, color: Color32) {
    let (rect, _) = ui.allocate_exact_size(Vec2::splat(10.0), Sense::hover());
    ui.painter().circle_filled(rect.center(), 3.5, color);
}

/// Etiqueta quadrada (badge-pill da referência).
pub fn badge(painter: &egui::Painter, ui: &Ui, pos: egui::Pos2, text: &str, fill: Color32, fg: Color32) -> Rect {
    let galley = ui.fonts(|f| f.layout_job(caps(text, 9.0, fg)));
    let rect = Rect::from_min_size(pos, galley.size() + Vec2::new(12.0, 6.0));
    painter.rect_filled(rect, 0, fill);
    painter.galley(rect.center() - galley.size() / 2.0, galley, fg);
    rect
}

/// Caixa da tecla de atalho (quadrado com contorno).
pub fn key_cap(painter: &egui::Painter, ui: &Ui, pos: egui::Pos2, key: &str, stroke: Color32, fg: Color32) -> Rect {
    let galley = ui.fonts(|f| f.layout_job(caps(key, 9.0, fg)));
    let side = (galley.size().x + 10.0).max(20.0);
    let rect = Rect::from_min_size(pos, Vec2::new(side, 20.0));
    painter.rect_stroke(rect, 0, Stroke::new(1.0_f32, stroke), egui::StrokeKind::Inside);
    painter.galley(rect.center() - galley.size() / 2.0 + Vec2::new(1.0, 0.0), galley, fg);
    rect
}

pub fn text_galley(ui: &Ui, job: LayoutJob) -> Arc<egui::Galley> {
    ui.fonts(|f| f.layout_job(job))
}

pub fn paint_text(painter: &egui::Painter, ui: &Ui, pos: egui::Pos2, anchor: Align2, job: LayoutJob) -> Rect {
    let g = text_galley(ui, job);
    let rect = anchor.anchor_size(pos, g.size());
    painter.galley(rect.min, g, p().text);
    rect
}

/// Moldura das janelas modais (wizard, configurações, tecla): contorno fino e respiro generoso.
pub fn modal_frame() -> egui::Frame {
    egui::Frame::default()
        .fill(p().bg)
        .stroke(Stroke::new(1.0_f32, p().border))
        .inner_margin(egui::Margin::same(40))
}
