use std::sync::mpsc::{self, Sender};
use tray_icon::menu::{CheckMenuItem, Menu, MenuEvent, MenuId, MenuItem, PredefinedMenuItem};
use tray_icon::{Icon, TrayIcon, TrayIconBuilder, TrayIconEvent};

use crate::updater::UpdateState;
use crate::win32;

const ICON_PNG: &[u8] = include_bytes!("../assets/icon.png");

pub enum TrayAction {
    TogglePanel,
    UpdateClick,
    OpenSettings,
    ToggleAutostart(bool),
    Quit,
}

#[derive(Clone)]
pub struct TrayIds {
    pub id_toggle: MenuId,
    pub id_update: Option<MenuId>,
    pub id_settings: MenuId,
    pub id_autostart: MenuId,
    pub id_quit: MenuId,
}

impl TrayIds {
    pub fn classify(&self, id: &MenuId) -> Option<TrayAction> {
        if id == &self.id_toggle {
            Some(TrayAction::TogglePanel)
        } else if self.id_update.as_ref() == Some(id) {
            Some(TrayAction::UpdateClick)
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

enum TrayCmd {
    SetAutostart(bool),
    SetUpdateState(UpdateState),
}

#[derive(Clone)]
pub struct TrayHandle {
    tx: Sender<TrayCmd>,
    #[cfg(windows)]
    thread_id: u32,
}

impl TrayHandle {
    pub fn set_autostart_checked(&self, on: bool) {
        let _ = self.tx.send(TrayCmd::SetAutostart(on));
        self.wake();
    }

    pub fn set_update_state(&self, st: UpdateState) {
        let _ = self.tx.send(TrayCmd::SetUpdateState(st));
        self.wake();
    }

    fn wake(&self) {
        #[cfg(windows)]
        if self.thread_id != 0 {
            unsafe {
                windows_sys::Win32::UI::WindowsAndMessaging::PostThreadMessageW(
                    self.thread_id,
                    windows_sys::Win32::UI::WindowsAndMessaging::WM_APP,
                    0,
                    0,
                );
            }
        }
    }
}

pub struct AppTray {
    tray: TrayIcon,
    ids: TrayIds,
    update_item: Option<MenuItem>,
    autostart_item: CheckMenuItem,
}

fn update_label_enabled(st: &UpdateState) -> Option<(&'static str, bool)> {
    match st {
        UpdateState::Unsupported => None,
        UpdateState::Checking => Some(("Checking for updates…", false)),
        UpdateState::Downloading(_) => Some(("Downloading update…", false)),
        UpdateState::Ready(_) => Some(("Restart to update", true)),
        UpdateState::Idle | UpdateState::Error => Some(("Check for updates", true)),
    }
}

impl AppTray {
    pub fn new(initial_update: &UpdateState) -> Result<Self, String> {
        let dyn_img = image::load_from_memory(ICON_PNG).map_err(|e| e.to_string())?;
        let rgba = dyn_img.to_rgba8();
        let (w, h) = rgba.dimensions();
        let icon = Icon::from_rgba(rgba.into_raw(), w, h).map_err(|e| e.to_string())?;

        let menu = Menu::new();
        let toggle = MenuItem::new("Show / hide panel (Ctrl+X)", true, None);
        let update_item = update_label_enabled(initial_update).map(|(lbl, en)| MenuItem::new(lbl, en, None));
        let settings = MenuItem::new("Settings", true, None);
        let autostart = CheckMenuItem::new("Start with Windows", true, win32::autostart_enabled(), None);
        let quit = MenuItem::new("Quit", true, None);

        let ids = TrayIds {
            id_toggle: toggle.id().clone(),
            id_update: update_item.as_ref().map(|u| u.id().clone()),
            id_settings: settings.id().clone(),
            id_autostart: autostart.id().clone(),
            id_quit: quit.id().clone(),
        };

        let _ = menu.append(&toggle);
        if let Some(ref upd) = update_item {
            let _ = menu.append(upd);
        }
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
            tray,
            ids,
            update_item,
            autostart_item: autostart,
        })
    }

    pub fn ids(&self) -> TrayIds {
        self.ids.clone()
    }

    pub fn set_autostart_checked(&self, on: bool) {
        self.autostart_item.set_checked(on);
    }

    pub fn set_update_state(&self, st: &UpdateState) {
        if let Some(ref item) = self.update_item
            && let Some((lbl, en)) = update_label_enabled(st)
        {
            item.set_text(lbl);
            item.set_enabled(en);
            let tip = match st {
                UpdateState::Ready(ver) => format!("LoL Companion — Update {ver} ready (restart to apply)"),
                _ => "LoL Companion".to_string(),
            };
            let _ = self.tray.set_tooltip(Some(tip));
        }
    }
}

/// Spawns a dedicated Win32 message-pump thread owning `AppTray` so menu labels update even when
/// `ROOT` is hidden, blocking with 0% CPU in `GetMessageW`.
pub fn spawn_tray(initial_update: UpdateState) -> Option<(TrayHandle, TrayIds)> {
    let (tx, rx) = mpsc::channel::<TrayCmd>();
    let (init_tx, init_rx) = mpsc::channel::<Option<(u32, TrayIds)>>();

    std::thread::spawn(move || {
        let Ok(app_tray) = AppTray::new(&initial_update) else {
            let _ = init_tx.send(None);
            return;
        };
        let ids = app_tray.ids();
        #[cfg(windows)]
        let tid = unsafe { windows_sys::Win32::System::Threading::GetCurrentThreadId() };
        #[cfg(not(windows))]
        let tid = 0u32;

        let _ = init_tx.send(Some((tid, ids)));

        #[cfg(windows)]
        unsafe {
            use windows_sys::Win32::UI::WindowsAndMessaging::{
                DispatchMessageW, GetMessageW, MSG, TranslateMessage, WM_APP,
            };
            let mut msg: MSG = std::mem::zeroed();
            while GetMessageW(&mut msg, std::ptr::null_mut(), 0, 0) > 0 {
                if msg.message == WM_APP {
                    while let Ok(cmd) = rx.try_recv() {
                        match cmd {
                            TrayCmd::SetAutostart(on) => app_tray.set_autostart_checked(on),
                            TrayCmd::SetUpdateState(st) => app_tray.set_update_state(&st),
                        }
                    }
                } else {
                    TranslateMessage(&msg);
                    DispatchMessageW(&msg);
                }
            }
        }
        #[cfg(not(windows))]
        {
            while let Ok(cmd) = rx.recv() {
                match cmd {
                    TrayCmd::SetAutostart(on) => app_tray.set_autostart_checked(on),
                    TrayCmd::SetUpdateState(st) => app_tray.set_update_state(&st),
                }
            }
        }
    });

    init_rx.recv().ok().flatten().map(|(thread_id, ids)| {
        (
            TrayHandle {
                tx,
                #[cfg(windows)]
                thread_id,
            },
            ids,
        )
    })
}

pub fn recv_menu_event() -> Option<MenuId> {
    MenuEvent::receiver().recv().ok().map(|e| e.id)
}

pub fn recv_tray_double_click() -> bool {
    matches!(TrayIconEvent::receiver().recv(), Ok(TrayIconEvent::DoubleClick { .. }))
}
