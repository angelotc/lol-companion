"""Find the League client and game windows, place the in-game screen over the game, and keep the
panel window where you left it. Windows only."""
from __future__ import annotations

import ctypes
from ctypes import wintypes as w
from dataclasses import dataclass

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
WNDENUMPROC = ctypes.WINFUNCTYPE(w.BOOL, w.HWND, w.LPARAM)
user32.EnumWindows.argtypes = [WNDENUMPROC, w.LPARAM]

CLIENT_CLASS = "RCLIENT"
CLIENT_TITLE = "League of Legends"
GAME_CLASS = "RiotWindowClass"  # shared with other Riot games (Valorant, the launcher)
GAME_TITLE = "League of Legends (TM) Client"  # match the title too, or Valorant passes as the game
GAME_TITLE_PREFIX = "League of Legends"
PANEL_BASE = (300, 1080)  # panel width designed for a 1080px-tall screen
MIN_WIDTH = 240
PANEL_ZOOM = 1.25  # default panel window: this much wider than the design width for its height
HWND_TOPMOST, HWND_NOTOPMOST = w.HWND(-1), w.HWND(-2)
SWP_NOSIZE, SWP_NOMOVE, SWP_NOZORDER, SWP_NOACTIVATE = 0x0001, 0x0002, 0x0004, 0x0010
SW_RESTORE = 9
GA_ROOTOWNER = 3
MONITOR_DEFAULTTONULL = 0


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
    return user32.FindWindowW(None, title) or 0


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


def panel_default(area: Rect) -> Rect:
    """Where the panel window first opens: the right edge of the primary monitor, its full height,
    and wide enough that the contents (which scale with width) read at PANEL_ZOOM x the design
    size for that height (about 480px wide, 1.6x, on a 1440p monitor)."""
    width = round(PANEL_BASE[0] * area.height / PANEL_BASE[1] * PANEL_ZOOM)
    width = min(area.width, max(MIN_WIDTH, width))
    return Rect(area.right - width, area.top, area.right, area.bottom)


EXPLORE_SIZE = (1280, 720)  # out-of-game window: the League client's own default size


def explore_default(area: Rect) -> Rect:
    """Where the panel window opens out of game: a regular, browser-like window the size of the
    League client (EXPLORE_SIZE), at the right edge and vertically centered, so the explorer's
    chart has room without the whole UI zooming in."""
    width = min(EXPLORE_SIZE[0], area.width)
    height = min(EXPLORE_SIZE[1], area.height)
    left = max(area.left, area.right - width)
    top = max(area.top, area.top + (area.height - height) // 2)
    return Rect(left, top, left + width, top + height)


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
    scale = game.height / PANEL_BASE[1] * size
    width = max(200, round(INGAME_BASE * scale))
    top = game.top + round(game.height * 0.13)
    height = round(game.height * 0.56)  # the minimap starts around 72% down
    return Placement("ingame", game.right - width - round(4 * scale), top, width, height, scale)


def dpi_scale(hwnd: int) -> float:
    """The monitor's display scaling for this window (1.0 at 100%). WebView2 applies it on top of
    our own scale, so the panel divides it back out."""
    try:
        return (user32.GetDpiForWindow(hwnd) or 96) / 96
    except (AttributeError, OSError):
        return 1.0


def set_no_activate(hwnd: int, on: bool) -> None:
    """In game: the panel takes the mouse (hover for details; Ctrl+X hides it) but never takes
    focus, so the keyboard stays with the game. Also clears the old click-through bits."""
    style = user32.GetWindowLongW(hwnd, GWL_EXSTYLE)
    new = style & ~(WS_EX_LAYERED | WS_EX_TRANSPARENT)
    new = (new | WS_EX_NOACTIVATE) if on else (new & ~WS_EX_NOACTIVATE)
    if new != style:
        user32.SetWindowLongW(hwnd, GWL_EXSTYLE, new)
