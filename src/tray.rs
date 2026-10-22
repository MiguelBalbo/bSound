//! Ícone na barra de menus (macOS) ou bandeja (Windows/Linux).
//! Clique esquerdo: abre o popover de pads (desenhado pela UI, ver `ui::popover`).
//! Clique direito: menu nativo para abrir a janela ou encerrar o app.
//! No Linux (AppIndicator) não há eventos de clique, então o menu ganha "Mostrar sons".

use std::sync::atomic::{AtomicBool, Ordering::Relaxed};
use std::sync::{Arc, Mutex};

use eframe::egui;
use tray_icon::menu::{Menu, MenuEvent, MenuItem, PredefinedMenuItem};
use tray_icon::{MouseButton, MouseButtonState, TrayIcon, TrayIconBuilder, TrayIconEvent};

/// Onde abrir o popover: centro horizontal, topo e base do ícone, em pixels físicos.
/// `None` nos campos quando o sistema não informa a posição (Linux).
#[derive(Clone, Copy)]
pub struct Anchor {
    pub x: f64,
    pub top: f64,
    pub bottom: f64,
}

/// Pedidos que chegam dos eventos do ícone e são atendidos pela UI no próximo frame.
pub struct TrayLink {
    pub ctx: egui::Context,
    /// Pedido de popover; o `Option` interno é `None` quando não se sabe onde está o ícone.
    pub popover_anchor: Mutex<Option<Option<Anchor>>>,
    pub open_main: AtomicBool,
}

impl TrayLink {
    pub fn new(ctx: egui::Context) -> Self {
        Self { ctx, popover_anchor: Mutex::new(None), open_main: AtomicBool::new(false) }
    }

    pub fn take_popover_request(&self) -> Option<Option<Anchor>> {
        self.popover_anchor.lock().unwrap_or_else(|e| e.into_inner()).take()
    }

    pub fn take_open_main(&self) -> bool {
        self.open_main.swap(false, Relaxed)
    }
}

/// Mantém o ícone vivo enquanto o app roda.
pub struct Tray {
    #[cfg(not(target_os = "linux"))]
    _icon: TrayIcon,
}

fn build_menu(with_pads: bool) -> Menu {
    let menu = Menu::new();
    if with_pads {
        let _ = menu.append_items(&[&MenuItem::with_id("pads", "Mostrar sons", true, None), &PredefinedMenuItem::separator()]);
    }
    let _ = menu.append_items(&[
        &MenuItem::with_id("open", "Abrir bSound", true, None),
        &PredefinedMenuItem::separator(),
        &MenuItem::with_id("quit", "Sair do bSound", true, None),
    ]);
    menu
}

fn build_icon(with_pads_item: bool) -> Option<TrayIcon> {
    // macOS: ícone "template" em preto, que o sistema pinta conforme o tema da barra de menus.
    // Windows/Linux: o símbolo colorido, que funciona em bandejas claras e escuras.
    #[cfg(target_os = "macos")]
    let png = include_bytes!("../assets/tray-icon.png").as_slice();
    #[cfg(not(target_os = "macos"))]
    let png = include_bytes!("../assets/tray-icon-color.png").as_slice();
    let data = eframe::icon_data::from_png_bytes(png).ok()?;
    let icon = tray_icon::Icon::from_rgba(data.rgba, data.width, data.height).ok()?;
    let builder = TrayIconBuilder::new();
    #[cfg(target_os = "macos")]
    let builder = builder.with_icon_templated(icon);
    #[cfg(not(target_os = "macos"))]
    let builder = builder.with_icon(icon);
    builder
        .with_tooltip("bSound")
        .with_menu(Box::new(build_menu(with_pads_item)))
        .with_menu_on_left_click(false)
        .with_menu_on_right_click(true)
        .build()
        .map_err(|e| log::error!("não foi possível criar o ícone da bandeja: {e}"))
        .ok()
}

fn request_popover(link: &TrayLink, anchor: Option<Anchor>) {
    *link.popover_anchor.lock().unwrap_or_else(|e| e.into_inner()) = Some(anchor);
    activate_app();
    link.ctx.request_repaint();
}

impl Tray {
    pub fn new(link: Arc<TrayLink>) -> Option<Self> {
        let click_link = link.clone();
        TrayIconEvent::set_event_handler(Some(move |event: TrayIconEvent| {
            if let TrayIconEvent::Click { button: MouseButton::Left, button_state: MouseButtonState::Down, rect, .. } = event {
                let anchor = Anchor {
                    x: rect.position.x + rect.size.width as f64 / 2.0,
                    top: rect.position.y,
                    bottom: rect.position.y + rect.size.height as f64,
                };
                request_popover(&click_link, Some(anchor));
            }
        }));
        let menu_link = link.clone();
        MenuEvent::set_event_handler(Some(move |event: MenuEvent| match event.id().as_ref() {
            "pads" => request_popover(&menu_link, None),
            "open" => {
                menu_link.open_main.store(true, Relaxed);
                show_app();
                menu_link.ctx.request_repaint();
            }
            "quit" => std::process::exit(0),
            _ => {}
        }));

        // No Linux o ícone precisa de um loop GTK na própria thread.
        #[cfg(target_os = "linux")]
        {
            std::thread::spawn(|| {
                if let Err(e) = gtk::init() {
                    log::error!("GTK indisponível, sem ícone na bandeja: {e}");
                    return;
                }
                let Some(_icon) = build_icon(true) else { return };
                gtk::main();
            });
            Some(Self {})
        }
        #[cfg(not(target_os = "linux"))]
        {
            Some(Self { _icon: build_icon(false)? })
        }
    }
}

// ---------- Integração com o sistema (só o macOS precisa) ----------

#[cfg(target_os = "macos")]
fn app() -> Option<objc2::rc::Retained<NSApplication>> {
    objc2::MainThreadMarker::new().map(NSApplication::sharedApplication)
}

#[cfg(target_os = "macos")]
use objc2_app_kit::{NSApplication, NSApplicationActivationPolicy};

/// Some do Dock (a janela é escondida pela UI); o app continua na barra de menus.
#[cfg(target_os = "macos")]
pub fn hide_app() {
    if let Some(app) = app() {
        app.setActivationPolicy(NSApplicationActivationPolicy::Accessory);
    }
}

#[cfg(target_os = "macos")]
pub fn show_app() {
    if let Some(app) = app() {
        app.setActivationPolicy(NSApplicationActivationPolicy::Regular);
        activate(&app);
    }
}

/// Traz o app para frente (necessário para o popover receber cliques e foco).
#[cfg(target_os = "macos")]
pub fn activate_app() {
    if let Some(app) = app() {
        activate(&app);
    }
}

/// Se o bSound é o app ativo (falso quando o usuário clica em outro app).
#[cfg(target_os = "macos")]
pub fn app_is_active() -> bool {
    app().is_none_or(|app| app.isActive())
}

#[cfg(target_os = "macos")]
fn activate(app: &NSApplication) {
    #[allow(deprecated)]
    app.activateIgnoringOtherApps(true);
}

// Nos outros sistemas a janela é escondida/mostrada pela UI e o foco fica com o egui.
#[cfg(not(target_os = "macos"))]
pub fn hide_app() {}
#[cfg(not(target_os = "macos"))]
pub fn show_app() {}
#[cfg(not(target_os = "macos"))]
pub fn activate_app() {}
#[cfg(not(target_os = "macos"))]
pub fn app_is_active() -> bool {
    true
}
