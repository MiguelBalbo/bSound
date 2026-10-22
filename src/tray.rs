//! Ícone na barra de menus do macOS.
//! Clique esquerdo: abre o popover de pads (desenhado pela UI, ver `ui::popover`).
//! Clique direito: menu nativo para abrir a janela ou encerrar o app.

use std::sync::atomic::{AtomicBool, Ordering::Relaxed};
use std::sync::{Arc, Mutex};

use eframe::egui;
use objc2::MainThreadMarker;
use objc2_app_kit::{NSApplication, NSApplicationActivationPolicy};
use tray_icon::menu::{Menu, MenuEvent, MenuItem, PredefinedMenuItem};
use tray_icon::{MouseButton, MouseButtonState, TrayIcon, TrayIconBuilder, TrayIconEvent};

/// Pedidos que chegam dos eventos do ícone e são atendidos pela UI no próximo frame.
pub struct TrayLink {
    pub ctx: egui::Context,
    /// Clique esquerdo: centro horizontal e base do ícone, em pixels físicos da tela.
    pub popover_anchor: Mutex<Option<(f64, f64)>>,
    pub open_main: AtomicBool,
}

impl TrayLink {
    pub fn new(ctx: egui::Context) -> Self {
        Self { ctx, popover_anchor: Mutex::new(None), open_main: AtomicBool::new(false) }
    }

    pub fn take_popover_request(&self) -> Option<(f64, f64)> {
        self.popover_anchor.lock().unwrap_or_else(|e| e.into_inner()).take()
    }

    pub fn take_open_main(&self) -> bool {
        self.open_main.swap(false, Relaxed)
    }
}

/// Mantém o ícone vivo enquanto o app roda.
pub struct Tray {
    _icon: TrayIcon,
}

impl Tray {
    pub fn new(link: Arc<TrayLink>) -> Option<Self> {
        let data = eframe::icon_data::from_png_bytes(include_bytes!("../assets/tray-icon.png")).ok()?;
        let icon = tray_icon::Icon::from_rgba(data.rgba, data.width, data.height).ok()?;

        let menu = Menu::new();
        let _ = menu.append_items(&[
            &MenuItem::with_id("open", "Abrir bSound", true, None),
            &PredefinedMenuItem::separator(),
            &MenuItem::with_id("quit", "Sair do bSound", true, None),
        ]);
        let tray = TrayIconBuilder::new()
            .with_icon_templated(icon)
            .with_tooltip("bSound")
            .with_menu(Box::new(menu))
            .with_menu_on_left_click(false)
            .with_menu_on_right_click(true)
            .build()
            .map_err(|e| log::error!("não foi possível criar o ícone da barra de menus: {e}"))
            .ok()?;

        let click_link = link.clone();
        TrayIconEvent::set_event_handler(Some(move |event: TrayIconEvent| {
            if let TrayIconEvent::Click { button: MouseButton::Left, button_state: MouseButtonState::Down, rect, .. } = event {
                let anchor = (rect.position.x + rect.size.width as f64 / 2.0, rect.position.y + rect.size.height as f64);
                *click_link.popover_anchor.lock().unwrap_or_else(|e| e.into_inner()) = Some(anchor);
                activate_app();
                click_link.ctx.request_repaint();
            }
        }));
        MenuEvent::set_event_handler(Some(move |event: MenuEvent| match event.id().as_ref() {
            "open" => {
                link.open_main.store(true, Relaxed);
                show_app();
                link.ctx.request_repaint();
            }
            "quit" => std::process::exit(0),
            _ => {}
        }));
        Some(Self { _icon: tray })
    }
}

fn app() -> Option<objc2::rc::Retained<NSApplication>> {
    MainThreadMarker::new().map(NSApplication::sharedApplication)
}

/// Some do Dock (a janela é escondida pela UI); o app continua na barra de menus.
pub fn hide_app() {
    if let Some(app) = app() {
        app.setActivationPolicy(NSApplicationActivationPolicy::Accessory);
    }
}

pub fn show_app() {
    if let Some(app) = app() {
        app.setActivationPolicy(NSApplicationActivationPolicy::Regular);
        activate(&app);
    }
}

/// Traz o app para frente (necessário para o popover receber cliques e foco).
pub fn activate_app() {
    if let Some(app) = app() {
        activate(&app);
    }
}

/// Se o bSound é o app ativo (falso quando o usuário clica em outro app).
pub fn app_is_active() -> bool {
    app().is_none_or(|app| app.isActive())
}

fn activate(app: &NSApplication) {
    #[allow(deprecated)]
    app.activateIgnoringOtherApps(true);
}
