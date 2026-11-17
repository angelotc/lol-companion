use tray_icon::menu::{CheckMenuItem, Menu, MenuEvent, MenuId, MenuItem, PredefinedMenuItem};
use tray_icon::{Icon, TrayIcon, TrayIconBuilder, TrayIconEvent};

use crate::win32;

const ICON_PNG: &[u8] = include_bytes!("../assets/icon.png");

pub enum TrayAction {
    TogglePanel,
    OpenSettings,
    ToggleAutostart,
    Quit,
}

pub struct AppTray {
    _tray: TrayIcon,
    id_toggle: MenuId,
    id_settings: MenuId,
    id_autostart: MenuId,
    id_quit: MenuId,
    autostart_item: CheckMenuItem,
}

impl AppTray {
    pub fn new() -> Result<Self, String> {
        let dyn_img = image::load_from_memory(ICON_PNG).map_err(|e| e.to_string())?;
        let rgba = dyn_img.to_rgba8();
        let (w, h) = rgba.dimensions();
        let icon = Icon::from_rgba(rgba.into_raw(), w, h).map_err(|e| e.to_string())?;

        let menu = Menu::new();
        let toggle = MenuItem::new("Show / hide panel (Ctrl+X)", true, None);
        let settings = MenuItem::new("Settings", true, None);
        let autostart = CheckMenuItem::new("Start with Windows", true, win32::autostart_enabled(), None);
        let quit = MenuItem::new("Quit", true, None);

        let id_toggle = toggle.id().clone();
        let id_settings = settings.id().clone();
        let id_autostart = autostart.id().clone();
        let id_quit = quit.id().clone();

        let _ = menu.append(&toggle);
        let _ = menu.append(&settings);
        let _ = menu.append(&autostart);
        let _ = menu.append(&PredefinedMenuItem::separator());
        let _ = menu.append(&quit);

        let tray = TrayIconBuilder::new()
            .with_menu(Box::new(menu))
            .with_tooltip("LoL Companion")
            .with_icon(icon)
            .build()
            .map_err(|e| e.to_string())?;

        Ok(Self {
            _tray: tray,
            id_toggle,
            id_settings,
            id_autostart,
            id_quit,
            autostart_item: autostart,
        })
    }

    pub fn poll_actions(&self) -> Vec<TrayAction> {
        let mut actions = Vec::new();
        while let Ok(ev) = MenuEvent::receiver().try_recv() {
            if ev.id == self.id_toggle {
                actions.push(TrayAction::TogglePanel);
            } else if ev.id == self.id_settings {
                actions.push(TrayAction::OpenSettings);
            } else if ev.id == self.id_autostart {
                let new_val = !win32::autostart_enabled();
                win32::set_autostart(new_val);
                self.autostart_item.set_checked(new_val);
                actions.push(TrayAction::ToggleAutostart);
            } else if ev.id == self.id_quit {
                actions.push(TrayAction::Quit);
            }
        }
        while let Ok(ev) = TrayIconEvent::receiver().try_recv() {
            if let TrayIconEvent::DoubleClick { .. } = ev {
                actions.push(TrayAction::TogglePanel);
            }
        }
        actions
    }
}
