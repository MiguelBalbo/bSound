//! Ícone na barra de menus do macOS.
//! Clique esquerdo: menu com os sons (mais tocados ou selecionados) para tocar na hora.
//! Clique direito: abrir a janela ou encerrar o app.
//!
//! O menu toca os sons direto pelo canal do mixer, sem passar pela UI, porque a janela
//! pode estar escondida (e aí o egui não roda frames).

use std::cell::RefCell;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use crossbeam_channel::Sender;
use eframe::egui;
use objc2::MainThreadMarker;
use objc2_app_kit::{NSApplication, NSApplicationActivationPolicy};
use tray_icon::menu::{ContextMenu, Menu, MenuEvent, MenuItem, PredefinedMenuItem};
use tray_icon::{MouseButton, MouseButtonState, TrayIcon, TrayIconBuilder, TrayIconEvent};

use crate::audio::clip::Clip;
use crate::audio::mixer::Command;
use crate::audio::shared::Shared;
use crate::stats::Stats;

/// O que o menu precisa para tocar sons; a UI mantém isto atualizado.
pub struct Playback {
    pub tx: Sender<Command>,
    pub clips: HashMap<usize, Arc<Clip>>,
}

pub struct TrayLink {
    pub playback: Mutex<Playback>,
    pub shared: Arc<Shared>,
    pub stats: Arc<Stats>,
    pub ctx: egui::Context,
}

// TrayIcon e Menu não são Send; vivem na thread principal, onde os eventos também chegam.
thread_local! {
    static TRAY: RefCell<Option<TrayIcon>> = const { RefCell::new(None) };
    static SOUNDS_MENU: RefCell<Option<Menu>> = const { RefCell::new(None) };
    static APP_MENU: RefCell<Option<Menu>> = const { RefCell::new(None) };
}

pub struct Tray {
    last: Option<(String, Vec<(usize, String)>)>,
}

impl Tray {
    pub fn new(link: Arc<TrayLink>) -> Option<Self> {
        let data = eframe::icon_data::from_png_bytes(include_bytes!("../assets/tray-icon.png")).ok()?;
        let icon = tray_icon::Icon::from_rgba(data.rgba, data.width, data.height).ok()?;

        let app_menu = Menu::new();
        let _ = app_menu.append_items(&[
            &MenuItem::with_id("open", "Abrir bSound", true, None),
            &PredefinedMenuItem::separator(),
            &MenuItem::with_id("quit", "Sair do bSound", true, None),
        ]);
        let tray = TrayIconBuilder::new()
            .with_icon_templated(icon)
            .with_tooltip("bSound")
            .with_menu(Box::new(app_menu.clone()))
            .with_menu_on_left_click(false)
            .with_menu_on_right_click(true)
            .build()
            .map_err(|e| log::error!("não foi possível criar o ícone da barra de menus: {e}"))
            .ok()?;
        TRAY.with(|t| *t.borrow_mut() = Some(tray));
        APP_MENU.with(|m| *m.borrow_mut() = Some(app_menu));

        TrayIconEvent::set_event_handler(Some(|event: TrayIconEvent| {
            if let TrayIconEvent::Click { button: MouseButton::Left, button_state: MouseButtonState::Down, .. } = event {
                show_sounds_menu();
            }
        }));
        MenuEvent::set_event_handler(Some(move |event: MenuEvent| handle_menu(&link, event.id().as_ref())));
        Some(Self { last: None })
    }

    /// Recria o menu de sons se a lista mudou.
    pub fn update(&mut self, title: &str, entries: Vec<(usize, String)>) {
        if self.last.as_ref().is_some_and(|(t, e)| t == title && *e == entries) {
            return;
        }
        let menu = Menu::new();
        let _ = menu.append(&MenuItem::new(title, false, None));
        if entries.is_empty() {
            let _ = menu.append(&MenuItem::new("Nenhum som ainda", false, None));
        }
        for (slot, name) in &entries {
            let _ = menu.append(&MenuItem::with_id(format!("play:{slot}"), name, true, None));
        }
        let _ = menu.append_items(&[
            &PredefinedMenuItem::separator(),
            &MenuItem::with_id("stop", "Parar tudo", true, None),
            &MenuItem::with_id("open", "Abrir bSound", true, None),
        ]);
        SOUNDS_MENU.with(|m| *m.borrow_mut() = Some(menu));
        self.last = Some((title.to_owned(), entries));
    }
}

fn show_sounds_menu() {
    let sounds = SOUNDS_MENU.with(|m| m.borrow().clone());
    let app = APP_MENU.with(|m| m.borrow().clone());
    TRAY.with(|t| {
        let t = t.borrow();
        let (Some(tray), Some(sounds)) = (t.as_ref(), sounds) else { return };
        // show_menu é síncrono no macOS: troca o menu, mostra, e devolve o menu do clique direito.
        tray.set_menu(Some(Box::new(sounds)));
        tray.show_menu();
        tray.set_menu(app.map(|m| Box::new(m) as Box<dyn ContextMenu>));
    });
}

fn handle_menu(link: &TrayLink, id: &str) {
    let playback = link.playback.lock().unwrap_or_else(|e| e.into_inner());
    match id {
        "open" => show_app(),
        "quit" => std::process::exit(0),
        "stop" => {
            let _ = playback.tx.try_send(Command::StopAll);
        }
        _ => {
            let Some(slot) = id.strip_prefix("play:").and_then(|s| s.parse::<usize>().ok()) else { return };
            if link.shared.is_playing(slot) {
                let _ = playback.tx.try_send(Command::Stop(slot));
            } else if let Some(clip) = playback.clips.get(&slot) {
                let _ = playback.tx.try_send(Command::Play { slot, clip: clip.clone() });
                link.stats.record(slot);
            }
        }
    }
    link.ctx.request_repaint();
}

/// Esconde a janela e o ícone do Dock; o app continua na barra de menus.
pub fn hide_app() {
    let Some(mtm) = MainThreadMarker::new() else { return };
    let app = NSApplication::sharedApplication(mtm);
    app.setActivationPolicy(NSApplicationActivationPolicy::Accessory);
    app.hide(None);
}

pub fn show_app() {
    let Some(mtm) = MainThreadMarker::new() else { return };
    let app = NSApplication::sharedApplication(mtm);
    app.setActivationPolicy(NSApplicationActivationPolicy::Regular);
    app.unhide(None);
    #[allow(deprecated)]
    app.activateIgnoringOtherApps(true);
}
