"""Find the League client and game windows, place the in-game screen over the game, and keep the
panel window where you left it, at one shape (16:9). Windows only."""
from __future__ import annotations

import ctypes
import logging
import os
from ctypes import wintypes as w
from dataclasses import dataclass

log = logging.getLogger("tracker")
user32 = ctypes.windll.user32
user32.SetWindowPos.argtypes = [w.HWND, w.HWND, ctypes.c_int, ctypes.c_int, ctypes.c_int, ctypes.c_int, w.UINT]
user32.ShowWindow.argtypes = [w.HWND, ctypes.c_int]
user32.GetAncestor.argtypes = [w.HWND, w.UINT]
user32.GetAncestor.restype = w.HWND
user32.GetForegroundWindow.restype = w.HWND
user32.MonitorFromRect.argtypes = [ctypes.c_void_p, w.DWORD]
user32.MonitorFromRect.restype = w.HMONITOR
user32.GetWindowLongW.argtypes = [w.HWND, ctypes.c_int]
user32.SetWindowLongW.argtypes = [w.HWND, ctypes.c_int, ctypes.c_long]
user32.WindowFromPoint.argtypes = [w.POINT]
user32.WindowFromPoint.restype = w.HWND
user32.GetClassNameW.argtypes = [w.HWND, w.LPWSTR, ctypes.c_int]
user32.GetWindowTextW.argtypes = [w.HWND, w.LPWSTR, ctypes.c_int]
user32.GetClientRect.argtypes = [w.HWND, ctypes.POINTER(w.RECT)]
user32.FindWindowExW.argtypes = [w.HWND, w.HWND, w.LPCWSTR, w.LPCWSTR]
user32.FindWindowExW.restype = w.HWND
user32.GetWindowThreadProcessId.argtypes = [w.HWND, ctypes.POINTER(w.DWORD)]
WNDENUMPROC = ctypes.WINFUNCTYPE(w.BOOL, w.HWND, w.LPARAM)
user32.EnumWindows.argtypes = [WNDENUMPROC, w.LPARAM]
WNDPROC = ctypes.WINFUNCTYPE(w.LPARAM, w.HWND, w.UINT, w.WPARAM, w.LPARAM)  # returns LRESULT
user32.GetWindowLongPtrW.argtypes = [w.HWND, ctypes.c_int]
user32.GetWindowLongPtrW.restype = ctypes.c_void_p
user32.SetWindowLongPtrW.argtypes = [w.HWND, ctypes.c_int, ctypes.c_void_p]
user32.SetWindowLongPtrW.restype = ctypes.c_void_p
user32.CallWindowProcW.argtypes = [ctypes.c_void_p, w.HWND, w.UINT, w.WPARAM, w.LPARAM]
user32.CallWindowProcW.restype = w.LPARAM

CLIENT_CLASS = "RCLIENT"
CLIENT_TITLE = "League of Legends"
GAME_CLASS = "RiotWindowClass"  # shared with other Riot games (Valorant, the launcher)
GAME_TITLE = "League of Legends (TM) Client"  # match the title too, or Valorant passes as the game
GAME_TITLE_PREFIX = "League of Legends"
INGAME_REF_HEIGHT = 1080  # the in-game window is designed for a 1080px-tall game
PAGE_SIZE = (1280, 720)  # the panel window's default page (client area): the League client's default size
ASPECT = 16 / 9  # the panel's page keeps this shape whatever its size
MIN_PAGE_WIDTH = 640  # smallest page while resizing (640x360)
MIN_WINDOW = (480, 270)  # WinForms' own limit: below ours, so it never cuts in and bends the shape
HWND_TOPMOST, HWND_NOTOPMOST = w.HWND(-1), w.HWND(-2)
SWP_NOSIZE, SWP_NOMOVE, SWP_NOZORDER, SWP_NOACTIVATE = 0x0001, 0x0002, 0x0004, 0x0010
SW_RESTORE = 9
GA_ROOTOWNER = 3
MONITOR_DEFAULTTONULL, MONITOR_DEFAULTTONEAREST = 0, 2


@dataclass(frozen=True)
class Rect:
    left: int
    top: int
    right: int
    bottom: int

    @property
    def width(self) -> int:
        return self.right - self.left

    @property
    def height(self) -> int:
        return self.bottom - self.top


@dataclass(frozen=True)
class Placement:
    mode: str  # the page's body mode: "ingame"
    x: int
    y: int
    width: int
    height: int
    scale: float


def make_dpi_aware() -> None:
    try:
        user32.SetProcessDpiAwarenessContext(ctypes.c_void_p(-4))  # per-monitor v2
    except (AttributeError, OSError):
        pass


def find_client() -> int:
    return user32.FindWindowW(CLIENT_CLASS, CLIENT_TITLE) or 0  # Riot Client shares the class


def game_running() -> bool:
    return bool(find_game())


def find_window(title: str) -> int:
    """This process's top-level window with this title. Only ours: the shell keeps proxy windows
    (Alt+Tab tab previews) that can carry the same title, and moving one of those instead does nothing."""
    pid, hwnd = w.DWORD(), None
    while hwnd := user32.FindWindowExW(None, hwnd, None, title):
        user32.GetWindowThreadProcessId(hwnd, ctypes.byref(pid))
        if pid.value == os.getpid():
            return hwnd
    return 0


def window_rect(hwnd: int) -> Rect:
    r = w.RECT()
    user32.GetWindowRect(hwnd, ctypes.byref(r))
    return Rect(r.left, r.top, r.right, r.bottom)


def primary_work_area() -> Rect:
    r = w.RECT()
    user32.SystemParametersInfoW(0x0030, 0, ctypes.byref(r), 0)  # SPI_GETWORKAREA
    return Rect(r.left, r.top, r.right, r.bottom)


def is_minimized(hwnd: int) -> bool:
    return bool(user32.IsIconic(hwnd)) or not user32.IsWindowVisible(hwnd)


def is_normal(hwnd: int) -> bool:
    """Shown, and neither minimized nor maximized: its rect is the size you gave it."""
    return not is_minimized(hwnd) and not user32.IsZoomed(hwnd)


def foreground_root() -> int:
    fg = user32.GetForegroundWindow()
    return (user32.GetAncestor(fg, GA_ROOTOWNER) or fg) if fg else 0


WMSZ_LEFT, WMSZ_RIGHT, WMSZ_TOP, WMSZ_TOPLEFT, WMSZ_TOPRIGHT, WMSZ_BOTTOM, WMSZ_BOTTOMLEFT, WMSZ_BOTTOMRIGHT = range(1, 9)
LEFT_EDGES = {WMSZ_LEFT, WMSZ_TOPLEFT, WMSZ_BOTTOMLEFT}
TOP_EDGES = {WMSZ_TOP, WMSZ_TOPLEFT, WMSZ_TOPRIGHT}
CORNERS = {WMSZ_TOPLEFT, WMSZ_TOPRIGHT, WMSZ_BOTTOMLEFT, WMSZ_BOTTOMRIGHT}


def frame_size(hwnd: int) -> tuple[int, int]:
    """The window's borders and title bar: its size minus its page's (client area) size."""
    cr = w.RECT()
    user32.GetClientRect(hwnd, ctypes.byref(cr))
    wr = window_rect(hwnd)
    return wr.width - cr.right, wr.height - cr.bottom


def fit_aspect(r: Rect, frame: tuple[int, int], edge: int = WMSZ_BOTTOMRIGHT, current: Rect | None = None) -> Rect:
    """`r` resized so its page (`r` minus `frame`) is ASPECT and at least MIN_PAGE_WIDTH wide.
    `edge` is the side or corner being dragged (WM_SIZING's), and the opposite one stays put. A
    side sets the size along its own axis; a corner follows whichever of width and height moved
    more from `current` (the window before this drag step), width if unknown."""
    fw, fh = frame
    cw, ch = r.width - fw, r.height - fh
    if edge in CORNERS:
        by_width = current is None or abs(cw - (current.width - fw)) >= abs(ch - (current.height - fh)) * ASPECT
    else:
        by_width = edge not in (WMSZ_TOP, WMSZ_BOTTOM)
    cw = max(MIN_PAGE_WIDTH, cw if by_width else round(ch * ASPECT))
    width, height = cw + fw, round(cw / ASPECT) + fh
    left = r.right - width if edge in LEFT_EDGES else r.left
    top = r.bottom - height if edge in TOP_EDGES else r.top
    return Rect(left, top, left + width, top + height)


def window_default(area: Rect, frame: tuple[int, int]) -> Rect:
    """Where the panel window first opens: a PAGE_SIZE page (smaller if the monitor is), at the
    right edge of `area` and vertically centered."""
    fw, fh = frame
    cw = min(PAGE_SIZE[0], area.width - fw, round((area.height - fh) * ASPECT))
    width, height = cw + fw, round(cw / ASPECT) + fh
    left = max(area.left, area.right - width)
    top = max(area.top, area.top + (area.height - height) // 2)
    return Rect(left, top, left + width, top + height)


WM_SIZING = 0x0214
GWLP_WNDPROC = -4
_aspect_procs: dict[int, WNDPROC] = {}  # hwnd -> its window procedure, kept alive for ctypes


def lock_aspect(hwnd: int) -> None:
    """Keep the window's page at ASPECT while you drag an edge or corner: WM_SIZING's rect is
    corrected as you drag, so it only resizes as a whole. Maximize and Snap still fill their
    area (the page zooms to fit the smaller side). Subclasses the window procedure, which then
    runs on the window's own thread."""
    if hwnd in _aspect_procs:
        return
    old = user32.GetWindowLongPtrW(hwnd, GWLP_WNDPROC)

    @WNDPROC
    def proc(h, msg, wparam, lparam):
        result = user32.CallWindowProcW(old, h, msg, wparam, lparam)
        if msg != WM_SIZING:
            return result
        try:
            rc = ctypes.cast(lparam, ctypes.POINTER(w.RECT)).contents
            r = fit_aspect(Rect(rc.left, rc.top, rc.right, rc.bottom), frame_size(h), wparam, window_rect(h))
            rc.left, rc.top, rc.right, rc.bottom = r.left, r.top, r.right, r.bottom
        except Exception:
            log.exception("aspect lock")
        return 1

    _aspect_procs[hwnd] = proc
    user32.SetWindowLongPtrW(hwnd, GWLP_WNDPROC, ctypes.cast(proc, ctypes.c_void_p))


def on_screen(r: Rect) -> bool:
    """Whether a window at `r` would show on a monitor (not one unplugged since)."""
    rc = w.RECT(r.left, r.top, r.right, r.bottom)
    return r.width > 0 and r.height > 0 and bool(user32.MonitorFromRect(ctypes.byref(rc), MONITOR_DEFAULTTONULL))


def set_rect(hwnd: int, r: Rect) -> None:
    user32.SetWindowPos(hwnd, None, r.left, r.top, r.width, r.height, SWP_NOZORDER | SWP_NOACTIVATE)


def bring_to_front(hwnd: int) -> None:
    """Un-minimize and raise above other windows without staying on top. Native calls, not
    pywebview's: setting its on_top off the UI thread can deadlock with the window's own events."""
    if user32.IsIconic(hwnd):
        user32.ShowWindow(hwnd, SW_RESTORE)
    flags = SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE
    user32.SetWindowPos(hwnd, HWND_TOPMOST, 0, 0, 0, 0, flags)
    user32.SetWindowPos(hwnd, HWND_NOTOPMOST, 0, 0, 0, 0, flags)


def move(hwnd: int, p: Placement) -> None:
    user32.SetWindowPos(hwnd, HWND_TOPMOST, p.x, p.y, p.width, p.height, SWP_NOACTIVATE)


INGAME_BASE = 620  # in-game window width at 1080p (before Config.ingame_scale): two 310px columns, players and build
GWL_EXSTYLE = -20
WS_EX_LAYERED, WS_EX_TRANSPARENT, WS_EX_NOACTIVATE = 0x00080000, 0x00000020, 0x08000000


def find_game() -> int:
    """The in-game window: class RiotWindowClass with a League title. The exact title is tried
    first; failing that, any window of the class whose title starts with "League of Legends"
    (so a renamed title still matches, but Valorant and the Riot Client never do)."""
    if hwnd := user32.FindWindowW(GAME_CLASS, GAME_TITLE):
        return hwnd
    found = []

    @WNDENUMPROC
    def callback(hwnd: int, _) -> bool:
        buf = ctypes.create_unicode_buffer(64)
        user32.GetClassNameW(hwnd, buf, 64)
        if buf.value == GAME_CLASS:
            user32.GetWindowTextW(hwnd, buf, 64)
            if buf.value.startswith(GAME_TITLE_PREFIX):
                found.append(hwnd)
                return False  # stop enumerating
        return True

    user32.EnumWindows(callback, 0)
    return found[0] if found else 0


def place_ingame(game: Rect, size: float = 1.0) -> Placement:
    """Right edge of the game, below the top bar and above the minimap (bottom right). Scales with
    the game's height (1.33x at 1440p), times `size` (Config.ingame_scale)."""
    scale = game.height / INGAME_REF_HEIGHT * size
    width = max(200, round(INGAME_BASE * scale))
    top = game.top + round(game.height * 0.13)
    height = round(game.height * 0.56)  # the minimap starts around 72% down
    return Placement("ingame", game.right - width - round(4 * scale), top, width, height, scale)


def dpi_scale(p: Placement) -> float:
    """Display scaling (1.0 at 100%) of the monitor a window placed at `p` lands on. WebView2
    applies it on top of our own scale, so the panel divides it back out. Read from the target
    monitor, not the window: before its first move the window sits wherever WinForms created it
    (the monitor under the mouse), which may have a different scaling than the game's."""
    rc = w.RECT(p.x, p.y, p.x + p.width, p.y + p.height)
    dpi = w.UINT()
    try:
        mon = user32.MonitorFromRect(ctypes.byref(rc), MONITOR_DEFAULTTONEAREST)
        if ctypes.windll.shcore.GetDpiForMonitor(mon, 0, ctypes.byref(dpi), ctypes.byref(w.UINT())) == 0:  # MDT_EFFECTIVE_DPI
            return (dpi.value or 96) / 96
    except (AttributeError, OSError):
        pass
    return 1.0


def set_no_activate(hwnd: int, on: bool) -> None:
    """In game: the panel takes the mouse (hover for details; Ctrl+X hides it) but never takes
    focus, so the keyboard stays with the game. Also clears the old click-through bits."""
    style = user32.GetWindowLongW(hwnd, GWL_EXSTYLE)
    new = style & ~(WS_EX_LAYERED | WS_EX_TRANSPARENT)
    new = (new | WS_EX_NOACTIVATE) if on else (new & ~WS_EX_NOACTIVATE)
    if new != style:
        user32.SetWindowLongW(hwnd, GWL_EXSTYLE, new)
