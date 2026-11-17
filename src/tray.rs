use tray_icon::menu::{CheckMenuItem, Menu, MenuEvent, MenuId, MenuItem, PredefinedMenuItem};
use tray_icon::{Icon, TrayIcon, TrayIconBuilder, TrayIconEvent};

use crate::win32;

const ICON_PNG: &[u8] = include_bytes!("../assets/icon.png");

pub enum TrayAction {
    TogglePanel,
    OpenSettings,
    ToggleAutostart(bool),
    Quit,
}

#[derive(Clone)]
pub struct TrayIds {
    pub id_toggle: MenuId,
    pub id_settings: MenuId,
    pub id_autostart: MenuId,
    pub id_quit: MenuId,
}

impl TrayIds {
    pub fn classify(&self, id: &MenuId) -> Option<TrayAction> {
        if id == &self.id_toggle {
            Some(TrayAction::TogglePanel)
        } else if id == &self.id_settings {
            Some(TrayAction::OpenSettings)
        } else if id == &self.id_autostart {
            let new_val = !win32::autostart_enabled();
            win32::set_autostart(new_val);
            Some(TrayAction::ToggleAutostart(new_val))
        } else if id == &self.id_quit {
            Some(TrayAction::Quit)
        } else {
            None
        }
    }
}

pub struct AppTray {
    _tray: TrayIcon,
    ids: TrayIds,
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

        let ids = TrayIds {
            id_toggle: toggle.id().clone(),
            id_settings: settings.id().clone(),
            id_autostart: autostart.id().clone(),
            id_quit: quit.id().clone(),
        };

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
            ids,
            autostart_item: autostart,
        })
    }

    pub fn ids(&self) -> TrayIds {
        self.ids.clone()
    }

    pub fn set_autostart_checked(&self, on: bool) {
        self.autostart_item.set_checked(on);
    }
}

pub fn recv_menu_event() -> Option<MenuId> {
    MenuEvent::receiver().recv().ok().map(|e| e.id)
}

pub fn recv_tray_double_click() -> bool {
    matches!(TrayIconEvent::receiver().recv(), Ok(TrayIconEvent::DoubleClick { .. }))
}
