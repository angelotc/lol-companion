#[cfg(windows)]
mod imp {
    use std::collections::HashMap;
    use std::ffi::OsStr;
    use std::os::windows::ffi::OsStrExt;
    use std::sync::Mutex;
    use windows_sys::Win32::Foundation::{CloseHandle, ERROR_ALREADY_EXISTS, GetLastError, HANDLE, HWND, LPARAM, LRESULT, RECT, WPARAM};
    use windows_sys::Win32::Graphics::Gdi::{MONITOR_DEFAULTTONEAREST, MONITOR_DEFAULTTONULL, MonitorFromRect};
    use windows_sys::Win32::System::Registry::{HKEY_CURRENT_USER, KEY_READ, KEY_WRITE, REG_SZ, RegCreateKeyExW, RegDeleteValueW, RegOpenKeyExW, RegQueryValueExW, RegSetValueExW, RegCloseKey};
    use windows_sys::Win32::System::Threading::{CreateEventW, CreateMutexW, EVENT_MODIFY_STATE, INFINITE, OpenEventW, SetEvent, WaitForSingleObject};
    use windows_sys::Win32::UI::HiDpi::{DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2, GetDpiForMonitor, MDT_EFFECTIVE_DPI, SetProcessDpiAwarenessContext};
    use windows_sys::Win32::UI::Input::KeyboardAndMouse::{GetAsyncKeyState, MOD_CONTROL, MOD_NOREPEAT, MOD_SHIFT, RegisterHotKey};
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        CallWindowProcW, DefWindowProcW, DispatchMessageW, EnumWindows, FindWindowExW, FindWindowW, GA_ROOTOWNER,
        GWL_EXSTYLE, GWLP_WNDPROC, GetAncestor, GetClassNameW, GetClientRect, GetForegroundWindow, GetMessageW,
        GetWindowLongPtrW, GetWindowLongW, GetWindowRect, GetWindowTextW, GetWindowThreadProcessId, HWND_NOTOPMOST,
        HWND_TOPMOST, IsIconic, IsWindowVisible, IsZoomed, MSG, SPI_GETWORKAREA, SW_RESTORE, SWP_NOACTIVATE, SWP_NOMOVE,
        SWP_NOSIZE, SWP_NOZORDER, SetWindowLongPtrW, SetWindowLongW, SetWindowPos, ShowWindow, SystemParametersInfoW,
        TranslateMessage, WM_HOTKEY, WM_SIZING, WMSZ_BOTTOM, WMSZ_BOTTOMLEFT, WMSZ_BOTTOMRIGHT, WMSZ_LEFT,
        WMSZ_TOP, WMSZ_TOPLEFT, WMSZ_TOPRIGHT, WS_EX_LAYERED, WS_EX_NOACTIVATE, WS_EX_TRANSPARENT,
        SW_HIDE, SW_SHOW, SW_SHOWNA,
    };

    pub const CLIENT_CLASS: &str = "RCLIENT";
    pub const CLIENT_TITLE: &str = "League of Legends";
    pub const GAME_CLASS: &str = "RiotWindowClass";
    pub const GAME_TITLE: &str = "League of Legends (TM) Client";
    pub const GAME_TITLE_PREFIX: &str = "League of Legends";
    pub const INGAME_REF_HEIGHT: f32 = 1080.0;
    pub const INGAME_BASE: f32 = 620.0;
    pub const ASPECT: f64 = 16.0 / 9.0;
    pub const MIN_PAGE_WIDTH: i32 = 640;
    pub const PAGE_SIZE: (i32, i32) = (1280, 720);

    const INSTANCE_MUTEX: &str = "Local\\LoLCompanion";
    const SHOW_EVENT: &str = "Local\\LoLCompanion.Show";
    const RUN_KEY: &str = "Software\\Microsoft\\Windows\\CurrentVersion\\Run";
    const RUN_VALUE: &str = "LoLCompanion";

    fn wide(s: &str) -> Vec<u16> {
        OsStr::new(s).encode_wide().chain(std::iter::once(0)).collect()
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct Rect {
        pub left: i32,
        pub top: i32,
        pub right: i32,
        pub bottom: i32,
    }

    impl Rect {
        pub fn width(self) -> i32 {
            self.right - self.left
        }
        pub fn height(self) -> i32 {
            self.bottom - self.top
        }
    }

    #[derive(Debug, Clone, Copy, PartialEq)]
    pub struct Placement {
        pub x: i32,
        pub y: i32,
        pub width: i32,
        pub height: i32,
        pub scale: f32,
    }

    pub fn make_dpi_aware() {
        unsafe {
            let _ = SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2);
        }
    }

    static INSTANCE_HOLD: Mutex<usize> = Mutex::new(0);

    pub fn claim_single_instance() -> bool {
        unsafe {
            let mname = wide(INSTANCE_MUTEX);
            let h = CreateMutexW(std::ptr::null(), 0, mname.as_ptr());
            *INSTANCE_HOLD.lock().unwrap() = h as usize;
            if GetLastError() != ERROR_ALREADY_EXISTS {
                return true;
            }
            let ename = wide(SHOW_EVENT);
            let ev = OpenEventW(EVENT_MODIFY_STATE, 0, ename.as_ptr());
            if !ev.is_null() {
                SetEvent(ev);
                CloseHandle(ev);
            }
            false
        }
    }

    pub fn wait_show_requests(on_show: impl Fn() + Send + 'static) {
        std::thread::spawn(move || unsafe {
            let ename = wide(SHOW_EVENT);
            let ev: HANDLE = CreateEventW(std::ptr::null(), 0, 0, ename.as_ptr());
            if ev.is_null() {
                return;
            }
            while WaitForSingleObject(ev, INFINITE) == 0 {
                on_show();
            }
        });
    }

    pub fn register_toggle_hotkey(on_hotkey: impl Fn() + Send + 'static) {
        std::thread::spawn(move || unsafe {
            if RegisterHotKey(std::ptr::null_mut(), 1, MOD_CONTROL | MOD_SHIFT | MOD_NOREPEAT, b'L' as u32) == 0 {
                tracing::warn!("Ctrl+Shift+L is taken by another app; fallback hotkey disabled");
            }
            let mut msg: MSG = std::mem::zeroed();
            while GetMessageW(&mut msg, std::ptr::null_mut(), 0, 0) > 0 {
                if msg.message == WM_HOTKEY && msg.wParam == 1 {
                    on_hotkey();
                } else {
                    TranslateMessage(&msg);
                    DispatchMessageW(&msg);
                }
            }
        });
    }

    pub fn find_client() -> isize {
        unsafe {
            let cls = wide(CLIENT_CLASS);
            let ttl = wide(CLIENT_TITLE);
            FindWindowW(cls.as_ptr(), ttl.as_ptr()) as isize
        }
    }

    pub fn find_window_by_title(title: &str) -> isize {
        unsafe {
            let ttl = wide(title);
            let my_pid = std::process::id();
            let mut hwnd: HWND = std::ptr::null_mut();
            loop {
                hwnd = FindWindowExW(std::ptr::null_mut(), hwnd, std::ptr::null(), ttl.as_ptr());
                if hwnd.is_null() {
                    return 0;
                }
                let mut pid = 0u32;
                GetWindowThreadProcessId(hwnd, &mut pid);
                if pid == my_pid {
                    return hwnd as isize;
                }
            }
        }
    }

    pub fn find_game() -> isize {
        unsafe {
            let cls = wide(GAME_CLASS);
            let ttl = wide(GAME_TITLE);
            let exact = FindWindowW(cls.as_ptr(), ttl.as_ptr()) as isize;
            if exact != 0 {
                return exact;
            }
            struct State {
                found: isize,
            }
            unsafe extern "system" fn enum_cb(hwnd: HWND, lparam: LPARAM) -> i32 {
                unsafe {
                    let mut buf = [0u16; 64];
                    let len = GetClassNameW(hwnd, buf.as_mut_ptr(), 64);
                    if len > 0 && String::from_utf16_lossy(&buf[..len as usize]) == GAME_CLASS {
                        let tlen = GetWindowTextW(hwnd, buf.as_mut_ptr(), 64);
                        if tlen > 0 && String::from_utf16_lossy(&buf[..tlen as usize]).starts_with(GAME_TITLE_PREFIX) {
                            let st = &mut *(lparam as *mut State);
                            st.found = hwnd as isize;
                            return 0;
                        }
                    }
                    1
                }
            }
            let mut st = State { found: 0 };
            EnumWindows(Some(enum_cb), &mut st as *mut State as LPARAM);
            st.found
        }
    }

    pub fn foreground_root() -> isize {
        unsafe {
            let fg = GetForegroundWindow();
            if fg.is_null() {
                return 0;
            }
            let root = GetAncestor(fg, GA_ROOTOWNER);
            if root.is_null() { fg as isize } else { root as isize }
        }
    }

    pub fn is_minimized(hwnd: isize) -> bool {
        if hwnd == 0 {
            return true;
        }
        unsafe { IsIconic(hwnd as HWND) != 0 || IsWindowVisible(hwnd as HWND) == 0 }
    }

    pub fn is_normal(hwnd: isize) -> bool {
        if hwnd == 0 {
            return false;
        }
        unsafe { !is_minimized(hwnd) && IsZoomed(hwnd as HWND) == 0 }
    }

    pub fn window_rect(hwnd: isize) -> Rect {
        unsafe {
            let mut r: RECT = std::mem::zeroed();
            GetWindowRect(hwnd as HWND, &mut r);
            Rect {
                left: r.left,
                top: r.top,
                right: r.right,
                bottom: r.bottom,
            }
        }
    }

    pub fn primary_work_area() -> Rect {
        unsafe {
            let mut r: RECT = std::mem::zeroed();
            SystemParametersInfoW(SPI_GETWORKAREA, 0, &mut r as *mut RECT as *mut _, 0);
            Rect {
                left: r.left,
                top: r.top,
                right: r.right,
                bottom: r.bottom,
            }
        }
    }

    pub fn on_screen(r: Rect) -> bool {
        if r.width() <= 0 || r.height() <= 0 {
            return false;
        }
        unsafe {
            let rc = RECT {
                left: r.left,
                top: r.top,
                right: r.right,
                bottom: r.bottom,
            };
            !MonitorFromRect(&rc, MONITOR_DEFAULTTONULL).is_null()
        }
    }

    pub fn place_ingame(game: Rect, size: f32) -> Placement {
        let scale = (game.height() as f32 / INGAME_REF_HEIGHT) * size;
        let width = (INGAME_BASE * scale).round().max(200.0) as i32;
        let top = game.top + (game.height() as f32 * 0.13).round() as i32;
        let height = (game.height() as f32 * 0.56).round() as i32;
        Placement {
            x: game.right - width - (4.0 * scale).round() as i32,
            y: top,
            width,
            height,
            scale,
        }
    }

    pub fn dpi_scale(p: Placement) -> f32 {
        unsafe {
            let rc = RECT {
                left: p.x,
                top: p.y,
                right: p.x + p.width,
                bottom: p.y + p.height,
            };
            let mon = MonitorFromRect(&rc, MONITOR_DEFAULTTONEAREST);
            let mut dpi_x = 96u32;
            let mut dpi_y = 96u32;
            if GetDpiForMonitor(mon, MDT_EFFECTIVE_DPI, &mut dpi_x, &mut dpi_y) == 0 && dpi_x > 0 {
                return dpi_x as f32 / 96.0;
            }
            1.0
        }
    }

    pub fn set_no_activate(hwnd: isize, on: bool) {
        if hwnd == 0 {
            return;
        }
        unsafe {
            let h = hwnd as HWND;
            let style = GetWindowLongW(h, GWL_EXSTYLE);
            let mut new = style & !((WS_EX_LAYERED | WS_EX_TRANSPARENT) as i32);
            if on {
                new |= WS_EX_NOACTIVATE as i32;
            } else {
                new &= !(WS_EX_NOACTIVATE as i32);
            }
            if new != style {
                SetWindowLongW(h, GWL_EXSTYLE, new);
            }
        }
    }

    pub fn move_topmost(hwnd: isize, p: Placement) {
        if hwnd == 0 {
            return;
        }
        unsafe {
            SetWindowPos(
                hwnd as HWND,
                HWND_TOPMOST,
                p.x,
                p.y,
                p.width,
                p.height,
                SWP_NOACTIVATE,
            );
        }
    }

    pub fn set_rect(hwnd: isize, r: Rect) {
        if hwnd == 0 {
            return;
        }
        unsafe {
            SetWindowPos(
                hwnd as HWND,
                std::ptr::null_mut(),
                r.left,
                r.top,
                r.width(),
                r.height(),
                SWP_NOZORDER | SWP_NOACTIVATE,
            );
        }
    }

    pub fn show_window(hwnd: isize, show: bool) {
        if hwnd == 0 {
            return;
        }
        unsafe {
            ShowWindow(hwnd as HWND, if show { SW_SHOW } else { SW_HIDE });
        }
    }

    pub fn show_window_na(hwnd: isize) {
        if hwnd == 0 {
            return;
        }
        unsafe {
            ShowWindow(hwnd as HWND, SW_SHOWNA);
        }
    }

    pub fn bring_to_front(hwnd: isize) {
        if hwnd == 0 {
            return;
        }
        unsafe {
            let h = hwnd as HWND;
            if IsIconic(h) != 0 {
                ShowWindow(h, SW_RESTORE);
            }
            let flags = SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE;
            SetWindowPos(h, HWND_TOPMOST, 0, 0, 0, 0, flags);
            SetWindowPos(h, HWND_NOTOPMOST, 0, 0, 0, 0, flags);
        }
    }

    pub fn ctrl_x_down() -> bool {
        unsafe {
            let down = |vk: i32| (GetAsyncKeyState(vk) as u16 & 0x8000) != 0;
            // Check X and Ctrl FIRST before checking Shift/Alt or window handles!
            down(b'X' as i32) && down(0x11) && !down(0x10) && !down(0x12)
        }
    }

    pub fn autostart_enabled() -> bool {
        unsafe {
            let sub = wide(RUN_KEY);
            let val = wide(RUN_VALUE);
            let mut hkey = std::ptr::null_mut();
            if RegOpenKeyExW(HKEY_CURRENT_USER, sub.as_ptr(), 0, KEY_READ, &mut hkey) != 0 {
                return false;
            }
            let ok = RegQueryValueExW(
                hkey,
                val.as_ptr(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
            ) == 0;
            RegCloseKey(hkey);
            ok
        }
    }

    pub fn set_autostart(on: bool) {
        unsafe {
            let sub = wide(RUN_KEY);
            let val = wide(RUN_VALUE);
            let mut hkey = std::ptr::null_mut();
            if RegCreateKeyExW(
                HKEY_CURRENT_USER,
                sub.as_ptr(),
                0,
                std::ptr::null(),
                0,
                KEY_WRITE,
                std::ptr::null(),
                &mut hkey,
                std::ptr::null_mut(),
            ) != 0
            {
                return;
            }
            if on {
                let exe = crate::updater::stable_exe_path();
                if !exe.as_os_str().is_empty() {
                    let cmd = format!("\"{}\"", exe.display());
                    let wcmd = wide(&cmd);
                    RegSetValueExW(
                        hkey,
                        val.as_ptr(),
                        0,
                        REG_SZ,
                        wcmd.as_ptr() as *const u8,
                        (wcmd.len() * 2) as u32,
                    );
                }
            } else {
                RegDeleteValueW(hkey, val.as_ptr());
            }
            RegCloseKey(hkey);
        }
    }

    static OLD_PROCS: Mutex<Option<HashMap<isize, isize>>> = Mutex::new(None);

    fn frame_size(hwnd: HWND) -> (i32, i32) {
        unsafe {
            let mut cr: RECT = std::mem::zeroed();
            GetClientRect(hwnd, &mut cr);
            let wr = window_rect(hwnd as isize);
            (wr.width() - cr.right, wr.height() - cr.bottom)
        }
    }

    pub fn fit_aspect(r: Rect, frame: (i32, i32), edge: usize, current: Option<Rect>) -> Rect {
        let (fw, fh) = frame;
        let cw = r.width() - fw;
        let ch = r.height() - fh;
        let is_corner = matches!(
            edge as u32,
            WMSZ_TOPLEFT | WMSZ_TOPRIGHT | WMSZ_BOTTOMLEFT | WMSZ_BOTTOMRIGHT
        );
        let by_width = if is_corner {
            match current {
                None => true,
                Some(cur) => {
                    (cw - (cur.width() - fw)).abs() as f64
                        >= ((ch - (cur.height() - fh)).abs() as f64) * ASPECT
                }
            }
        } else {
            !matches!(edge as u32, WMSZ_TOP | WMSZ_BOTTOM)
        };
        let cw = if by_width {
            cw
        } else {
            (ch as f64 * ASPECT).round() as i32
        }
        .max(MIN_PAGE_WIDTH);
        let width = cw + fw;
        let height = (cw as f64 / ASPECT).round() as i32 + fh;
        let left = if matches!(edge as u32, WMSZ_LEFT | WMSZ_TOPLEFT | WMSZ_BOTTOMLEFT) {
            r.right - width
        } else {
            r.left
        };
        let top = if matches!(edge as u32, WMSZ_TOP | WMSZ_TOPLEFT | WMSZ_TOPRIGHT) {
            r.bottom - height
        } else {
            r.top
        };
        Rect {
            left,
            top,
            right: left + width,
            bottom: top + height,
        }
    }

    unsafe extern "system" fn aspect_wndproc(
        hwnd: HWND,
        msg: u32,
        wparam: WPARAM,
        lparam: LPARAM,
    ) -> LRESULT {
        unsafe {
            let old = OLD_PROCS
                .lock()
                .unwrap()
                .as_ref()
                .and_then(|m| m.get(&(hwnd as isize)).copied())
                .unwrap_or(0);
            let result = if old != 0 {
                let proc_fn: unsafe extern "system" fn(HWND, u32, WPARAM, LPARAM) -> LRESULT =
                    std::mem::transmute(old);
                CallWindowProcW(Some(proc_fn), hwnd, msg, wparam, lparam)
            } else {
                DefWindowProcW(hwnd, msg, wparam, lparam)
            };
            if msg == WM_SIZING && lparam != 0 {
                let rc = &mut *(lparam as *mut RECT);
                let r = fit_aspect(
                    Rect {
                        left: rc.left,
                        top: rc.top,
                        right: rc.right,
                        bottom: rc.bottom,
                    },
                    frame_size(hwnd),
                    wparam,
                    Some(window_rect(hwnd as isize)),
                );
                rc.left = r.left;
                rc.top = r.top;
                rc.right = r.right;
                rc.bottom = r.bottom;
                return 1;
            }
            result
        }
    }

    pub fn lock_aspect(hwnd: isize) {
        if hwnd == 0 {
            return;
        }
        let mut map = OLD_PROCS.lock().unwrap();
        let m = map.get_or_insert_with(HashMap::new);
        if m.contains_key(&hwnd) {
            return;
        }
        unsafe {
            let old = GetWindowLongPtrW(hwnd as HWND, GWLP_WNDPROC);
            m.insert(hwnd, old);
            SetWindowLongPtrW(hwnd as HWND, GWLP_WNDPROC, aspect_wndproc as usize as isize);
        }
    }

    pub fn open_file(path: &std::path::Path) {
        let _ = std::process::Command::new("cmd")
            .args(["/C", "start", "", &path.display().to_string()])
            .spawn();
    }
}

#[cfg(windows)]
pub use imp::*;

#[cfg(not(windows))]
mod imp {
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct Rect {
        pub left: i32,
        pub top: i32,
        pub right: i32,
        pub bottom: i32,
    }
    impl Rect {
        pub fn width(self) -> i32 { self.right - self.left }
        pub fn height(self) -> i32 { self.bottom - self.top }
    }
    #[derive(Debug, Clone, Copy, PartialEq)]
    pub struct Placement {
        pub x: i32,
        pub y: i32,
        pub width: i32,
        pub height: i32,
        pub scale: f32,
    }
    pub fn make_dpi_aware() {}
    pub fn claim_single_instance() -> bool { true }
    pub fn wait_show_requests(_: impl Fn() + Send + 'static) {}
    pub fn register_toggle_hotkey(_: impl Fn() + Send + 'static) {}
    pub fn find_client() -> isize { 0 }
    pub fn find_window_by_title(_: &str) -> isize { 0 }
    pub fn find_game() -> isize { 0 }
    pub fn foreground_root() -> isize { 0 }
    pub fn is_minimized(_: isize) -> bool { false }
    pub fn is_normal(_: isize) -> bool { false }
    pub fn window_rect(_: isize) -> Rect { Rect { left: 0, top: 0, right: 1280, bottom: 720 } }
    pub fn primary_work_area() -> Rect { Rect { left: 0, top: 0, right: 1920, bottom: 1080 } }
    pub fn on_screen(_: Rect) -> bool { true }
    pub fn place_ingame(_: Rect, _: f32) -> Placement { Placement { x: 0, y: 0, width: 620, height: 600, scale: 1.0 } }
    pub fn dpi_scale(_: Placement) -> f32 { 1.0 }
    pub fn set_no_activate(_: isize, _: bool) {}
    pub fn show_window(_: isize, _: bool) {}
    pub fn show_window_na(_: isize) {}
    pub fn move_topmost(_: isize, _: Placement) {}
    pub fn set_rect(_: isize, _: Rect) {}
    pub fn bring_to_front(_: isize) {}
    pub fn ctrl_x_down() -> bool { false }
    pub fn autostart_enabled() -> bool { false }
    pub fn set_autostart(_: bool) {}
    pub fn lock_aspect(_: isize) {}
    pub fn open_file(_: &std::path::Path) {}
}

#[cfg(not(windows))]
pub use imp::*;
