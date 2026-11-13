"""Desktop app: the panel is a normal window (bans, picks and build in champ select, the blind-pick
explorer between games); in game a second window over the game's right edge shows your build and
every player's rank and form.

  uv run app.py                          live
  uv run app.py --record drafts/x.jsonl  live, and save every session update for replay
  uv run app.py --replay drafts/x.jsonl  replay a recorded draft (panel always shown)
  uv run app.py --simulate --role middle --allies jinx --enemies syndra,leesin

  uv run app.py --simulate --role support --items 3858,1001 --players
                                         the in-game screen (either or both halves; players from
                                         tests/fixtures/players)

Ctrl+X (with League or the panel in front) toggles the panel; in game it toggles the in-game screen.
Closing the panel window hides it to the tray.
"""
from __future__ import annotations

import argparse
import asyncio
import ctypes
import json
import logging
import os
import shutil
import threading
import time
from ctypes import wintypes as w
from dataclasses import dataclass
from pathlib import Path

import webview

from desktop import explore_ui
from desktop import settings_ui
from desktop import tracker
from desktop import updater
from config import load_config
from draft.controller import Controller, load_settings, save_settings
from data.store import Store
from data.lcu import DraftState, LcuClient, Slot, read_lockfile, watch_session
from data.live import LiveGame
from paths import DATA_DIR, RESOURCES, config_path
from draft.scoring import LANES
from draft.state import Stage
from desktop.tray import Tray

TITLE = "LoL Companion"
GAME_TITLE = "LoL Companion — In game"
SETTINGS_TITLE = "LoL Companion — Settings"
# stages where the panel is between games and opens wide for the explorer
WIDE_STAGES = {Stage.IDLE.value, Stage.LOBBY.value, Stage.QUEUE.value, Stage.READY.value,
               Stage.POSTGAME.value}
MOD_CONTROL, MOD_SHIFT, MOD_NOREPEAT, WM_HOTKEY = 0x2, 0x4, 0x4000, 0x0312
HOTKEYS = {1: "L"}  # Ctrl+Shift+L panel, from anywhere (fallback; Ctrl+X is polled, see toggle_keys)
VK_SHIFT, VK_CONTROL, VK_MENU = 0x10, 0x11, 0x12
TOGGLE_KEY = "X"  # Ctrl+X while the client, either panel window or the game is in front
KEY_POLL = 1 / 30  # seconds; in-game key check rate
UPDATE_EVERY = 20 * 60  # seconds between update checks
CTRL_C_EVENT, CTRL_BREAK_EVENT, CTRL_CLOSE_EVENT = 0, 1, 2
# one live copy at a time: a second launch signals SHOW_EVENT and exits
INSTANCE_MUTEX, SHOW_EVENT = "Local\\LoLCompanion", "Local\\LoLCompanion.Show"
ERROR_ALREADY_EXISTS, EVENT_MODIFY_STATE = 183, 0x0002
HANDLER_ROUTINE = ctypes.WINFUNCTYPE(w.BOOL, w.DWORD)

log = logging.getLogger("app")


class Api:
    """Exposed to the page as window.pywebview.api."""

    def __init__(self, app: "App"):
        self._app = app

    def set_tier(self, tier: str) -> None:
        self._app.ctl.set_tier(tier)

    def set_enemy_lane(self, champion_id: int, lane: str) -> None:
        self._app.ctl.set_enemy_lane(int(champion_id), lane)

    def open_settings(self) -> None:
        self._app.open_settings()

    def explore_start(self) -> dict:
        return self._app.explore.start()

    def explore_get(self, role: str, tier: str) -> dict:
        return self._app.explore.get(role, tier)


@dataclass
class Tracked:
    """A window the tracker keeps up to date: its handle once found, whether it's shown, and where."""
    title: str
    window: webview.Window
    hwnd: int = 0  # pywebview creates the native window lazily
    shown: bool = False
    last: tracker.Placement | None = None
    view: str = ""  # in-game window: the last setScale/setMode sent to its page
    rect: tracker.Rect | None = None  # panel window: its last position and size in this context
    wide: bool = False  # panel window: showing the out-of-game (explorer) rect


def raise_window(win: webview.Window) -> bool:
    """Bring an open window to the front; False if it's gone (make a new one)."""
    try:
        win.restore()
        win.show()
        win.on_top = True  # raise above other windows, then back to normal
        win.on_top = False
        return True
    except Exception:
        log.exception("raise window")
        return False


class App:
    def __init__(self, args):
        self.args = args
        self._settings_lock = threading.Lock()
        self.always_show = bool(args.replay or args.simulate)
        self.window: webview.Window | None = None  # client panel
        self.game_window: webview.Window | None = None  # in-game screen
        self._raise = False  # bring the panel window to the front at the next tracker tick
        self._quitting = False  # closing the panel window quits instead of hiding it
        live = not self.always_show
        self.ctl = Controller(load_config(args.config), self.push, Store() if live else None)
        self.explore_up = True  # the panel starts out of game: the wide explorer window
        self.explore = explore_ui.ExploreApi(
            self.blind_report, lambda: self.ctl.rec.tier if self.ctl.rec else "master_plus", self.main_role)
        self.updater = updater.Updater(on_change=lambda: self.tray.on_update())
        self.settings_window: webview.Window | None = None
        self.tray = Tray(self.toggle_panel, self.quit, self.updater, self.restart, self.open_settings)

    def set_hidden(self, hidden: bool) -> None:
        """Hide the panel window, or show it and bring it to the front."""
        self.ctl.user_hidden = hidden
        if not hidden:
            self._raise = True

    def toggle_panel(self) -> None:
        """Hide the panel if it's up; show it if it's hidden or minimized."""
        hwnd = tracker.find_window(TITLE)
        self.set_hidden(not self.ctl.user_hidden and not (hwnd and tracker.is_minimized(hwnd)))

    def _on_panel_closing(self) -> bool:
        """The panel window's close button hides it to the tray; only Quit closes it."""
        if self._quitting:
            return True
        log.info("%s closed: hidden to the tray", TITLE)
        self.set_hidden(True)
        return False

    def quit(self) -> None:
        self._quitting = True
        self.tray.stop()  # before the windows close, so no ghost icon is left behind
        self.updater.apply_on_exit()
        for win in (self.game_window, self.settings_window, self.window):  # webview.start returns, run() exits
            if win:
                try:
                    win.destroy()
                except Exception:
                    log.exception("close window")

    def restart(self) -> None:
        if self.updater.state != "ready":
            return
        self.tray.stop()  # the new version adds its own icon
        self.updater.apply_and_restart()
        if self.updater.state == "error":  # still running: the update failed, bring the tray back
            self.tray = Tray(self.toggle_panel, self.quit, self.updater, self.restart, self.open_settings)
            self.tray.start()

    def open_settings(self) -> None:
        """The settings window: one at a time, brought to front if already open."""
        with self._settings_lock:
            if self.settings_window and raise_window(self.settings_window):
                return
            api = settings_ui.SettingsApi(Path(self.args.config) if self.args.config else config_path(),
                                          self.ctl.reload_config, self.version_text)
            win = webview.create_window(
                SETTINGS_TITLE, url=str(RESOURCES / "panel" / "settings.html"), js_api=api,
                width=480, height=480, min_size=(420, 360), resizable=True, background_color="#010a13",
            )
            api._window = win
            win.events.closed += lambda: setattr(self, "settings_window", None)
            self.settings_window = win

    def blind_report(self, role: str, tier: str) -> dict:
        if not self.ctl.rec:
            raise RuntimeError("stats not ready")
        return self.ctl.rec.blind_report(role, tier)

    def main_role(self) -> str | None:
        """Your most played role in recent games, else your last draft's role."""
        if self.ctl.store:
            try:
                top = self.ctl.store.top_roles(limit=1)
                if top:
                    return top[0]
            except Exception:
                log.exception("most played role")
        ev = self.ctl.last_eval
        return ev.role if ev else self.ctl.last_role or None

    def version_text(self) -> str:
        """For the settings window: "Version 0.1.4", plus a downloaded update if there is one."""
        u = self.updater
        text = f"Version {u.current_version()}"
        if u.state == "ready" and u.version:
            text += f" · {u.version} ready (restart to update)"
        return text

    def wait_show_requests(self) -> None:
        """Another copy was started: show this copy's panel instead."""
        k32 = ctypes.windll.kernel32
        k32.CreateEventW.restype = w.HANDLE
        event = k32.CreateEventW(None, False, False, SHOW_EVENT)  # auto-reset
        while k32.WaitForSingleObject(event, 0xFFFFFFFF) == 0:
            log.info("second copy started: showing panel")
            self.set_hidden(False)

    def check_updates(self) -> None:
        while True:
            self.updater.check_async()
            time.sleep(UPDATE_EVERY)

    # -- page updates -----------------------------------------------------
    def js(self, win: webview.Window | None, code: str) -> None:
        if win:
            win.evaluate_js(code)

    def push(self, state: dict) -> None:
        if state.get("view") == "client":
            # between games the panel opens wide for the explorer; drafts (and games) use the
            # narrow rect, so the panel keeps out of the client's and the game's way
            self.explore_up = bool(state.get("explore")) and state.get("stage") in WIDE_STAGES
        win = self.game_window if state.get("view") == "game" else self.window
        self.js(win, f"window.updateState&&updateState({json.dumps(state)})")

    # -- draft sources ----------------------------------------------------
    def run_live(self) -> None:
        ctl = self.ctl
        pickable: set[int] = set()
        record: Path | None = self.args.record
        if record:
            record.parent.mkdir(parents=True, exist_ok=True)

        def on_session(session: dict | None) -> None:
            nonlocal pickable
            if record and session:
                with record.open("a", encoding="utf-8") as f:
                    f.write(json.dumps({"t": time.time(), "session": session}) + "\n")
            if not session:
                pickable = set()
            elif not pickable:
                try:
                    client = LcuClient(read_lockfile(ctl.cfg.lockfile))
                    pickable = client.get_pickable_ids()
                    client.close()
                except Exception:
                    pass
            ctl.on_session(session, pickable)

        ctl.lcu = lambda: LcuClient(read_lockfile(ctl.cfg.lockfile))

        def on_phase(phase: str) -> None:
            ctl.idle_message = "Waiting for champ select..."  # connected (the panel is hidden when idle)
            ctl.on_phase(phase)

        ctl.render()
        asyncio.run(watch_session(lambda: read_lockfile(ctl.cfg.lockfile), on_session, on_phase=on_phase))

    def run_replay(self, path: Path) -> None:
        entries = [json.loads(l) for l in path.read_text(encoding="utf-8-sig").splitlines() if l.strip()]
        for e in entries:
            self.ctl.on_session(e["session"])
            time.sleep(self.args.delay)

    def run_simulate(self) -> None:
        ctl = self.ctl
        ids = lambda csv: [c for n in csv.split(",") if n.strip() and (c := ctl.rec.champs.id_from_slug(n))]
        allies = [Slot(c, "", False, False) for c in ids(self.args.allies)]
        enemies = ids(self.args.enemies)
        draft = DraftState(
            my_position=self.args.role, allies=allies, enemies=enemies,
            bans=set(ids(self.args.bans)), picked={s.champion_id for s in allies} | set(enemies),
        )
        with ctl.lock:
            if self.args.players:  # every player's rank and form, from the fixtures
                from data.players import FixtureClient, game_refs, load_all
                client = FixtureClient(RESOURCES / "tests" / "fixtures" / "players")
                _, refs = game_refs(client)
                ctl.players = (1, load_all(client, refs))
            if self.args.items:  # your build, from these items
                lanes = ["bottom", "support", "middle", "jungle", "top"]
                names = [ctl.rec.champs.name(c) for c in enemies]
                ctl.live = LiveGame(self.args.champion, self.args.role, self.args.level, self.args.gold,
                                    self.args.items, self.args.level * 120.0, {}, dict(zip(lanes, names)))
            if self.args.players or self.args.items:  # a game in progress: the in-game screen, as after Ctrl+X
                ctl.session.phase, ctl.ingame_shown = "InProgress", True
            else:
                ctl.session.draft = draft
            ctl.render()

    # -- window placement -------------------------------------------------
    def _game_screen(self) -> bool:
        return self.ctl.session.stage == Stage.INGAME and self.ctl.ingame_shown

    def _panel_visible(self) -> bool:
        if self.always_show and self._game_screen():
            return False  # the game window shows instead
        return not self.ctl.user_hidden

    def _game_target(self, game: int, panel: int, front: int) -> tuple[tracker.Placement | None, bool]:
        """Where the in-game screen should be and whether it may show there: only after Ctrl+X, over
        the game's right edge."""
        if not self._game_screen():
            return None, False
        scale = self.ctl.cfg.ingame_scale
        if self.always_show:
            return tracker.place_ingame(tracker.primary_work_area(), scale), True
        if not game or tracker.is_minimized(game):  # no game window yet: the loading screen
            return None, False
        return tracker.place_ingame(tracker.window_rect(game), scale), front in (game, panel)

    def track(self) -> None:
        """Keep both windows placed and shown/hidden; never dies (errors are logged, loop continues)."""
        panel, game = Tracked(TITLE, self.window), Tracked(GAME_TITLE, self.game_window)
        while True:
            try:
                self._track_once(panel, game)
            except Exception:
                log.exception("track")
                time.sleep(1.0)
            time.sleep(0.25)

    def _track_once(self, panel: Tracked, game_panel: Tracked) -> None:
        if not game_panel.hwnd and (hwnd := tracker.find_window(GAME_TITLE)):
            tracker.set_no_activate(hwnd, True)  # over the game, it never takes the keyboard
            game_panel.hwnd = hwnd
        if not panel.hwnd and (hwnd := tracker.find_window(TITLE)):
            panel.hwnd, panel.rect, panel.wide = hwnd, self._panel_rect(self.explore_up), self.explore_up
            tracker.set_rect(hwnd, panel.rect)  # before the first show, so it opens there
        game, front = tracker.find_game(), tracker.foreground_root()
        if game_panel.hwnd:
            self._apply(game_panel, *self._game_target(game, game_panel.hwnd, front))
        if panel.hwnd:
            self._apply_panel(panel)

    def _panel_rect(self, wide: bool) -> tracker.Rect:
        """Where the panel window was left last time in this context (wide: out of game), if
        that's still on a monitor."""
        saved = load_settings().get("explore_rect" if wide else "panel_rect")
        if isinstance(saved, list) and len(saved) == 4 and all(isinstance(v, int) for v in saved):
            rect = tracker.Rect(*saved)
            if tracker.on_screen(rect):
                return rect
        area = tracker.primary_work_area()
        return tracker.explore_default(area) if wide else tracker.panel_default(area)

    def _apply_panel(self, t: Tracked) -> None:
        """Show, raise or hide the panel window. Between games it takes the wide explorer rect,
        in champ select (and in game) the narrow one; each is remembered where you leave it."""
        if t.wide != self.explore_up:  # champ select came or went: switch to that context's window
            t.wide = self.explore_up
            t.rect = self._panel_rect(t.wide)
            if t.hwnd and t.shown and tracker.is_normal(t.hwnd):
                tracker.set_rect(t.hwnd, t.rect)
        visible = self._panel_visible()
        if visible and (not t.shown or self._raise):
            if not t.shown:
                log.info("%s shown", t.title)
                t.window.show()  # via pywebview so WinForms/WebView2 know they're visible
                tracker.set_rect(t.hwnd, t.rect)  # again: WinForms resizes it on its first show
                t.shown = True
            if self._raise:  # asked for (tray, hotkey, second launch): also un-minimize and raise
                tracker.bring_to_front(t.hwnd)
            self._raise = False
        elif not visible and t.shown:
            log.info("%s hidden", t.title)
            t.window.hide()
            t.shown = False
        if t.shown and tracker.is_normal(t.hwnd) and (rect := tracker.window_rect(t.hwnd)) != t.rect:
            t.rect = rect
            save_settings(**{"explore_rect" if t.wide else "panel_rect":
                             [rect.left, rect.top, rect.right, rect.bottom]})

    def _apply(self, t: Tracked, placement: tracker.Placement | None, focused: bool) -> None:
        """Move, rescale and show or hide one window."""
        visible = bool(placement and focused)
        if visible and (not t.shown or placement != t.last):
            view = (f"setScale({placement.scale / tracker.dpi_scale(placement):.3f});"
                    f"setMode({json.dumps(placement.mode)})")
            if view != t.view:  # also when the game moves to a monitor with other display scaling
                self.js(t.window, view)
                t.view = view
            if not t.shown:
                log.info("%s shown (%s)", t.title, placement.mode)
                t.window.show()  # via pywebview so WinForms/WebView2 know they're visible
            tracker.move(t.hwnd, placement)
            t.shown, t.last = True, placement
        elif not visible and t.shown:
            log.info("%s hidden", t.title)
            t.window.hide()
            t.shown = False

    def hotkey(self) -> None:
        for hid, key in HOTKEYS.items():
            if not ctypes.windll.user32.RegisterHotKey(None, hid, MOD_CONTROL | MOD_SHIFT | MOD_NOREPEAT, ord(key)):
                log.warning("Ctrl+Shift+%s is taken by another app (another copy running?); that hotkey is disabled", key)
            else:
                log.info("hotkey Ctrl+Shift+%s registered", key)
        msg = w.MSG()
        while ctypes.windll.user32.GetMessageW(ctypes.byref(msg), None, 0, 0) > 0:
            if msg.message != WM_HOTKEY:
                continue
            log.info("hotkey Ctrl+Shift+%s (stage %s)", HOTKEYS.get(msg.wParam, "?"), self.ctl.session.stage.value)
            if msg.wParam == 1:
                self.toggle_panel()

    def toggle_keys(self) -> None:
        """Ctrl+X while League is in front: in game it toggles the in-game screen, otherwise the
        panel. The game reads the keyboard in a mode that turns off other programs' hotkeys, so
        RegisterHotKey never fires there; poll the key state instead. Polling doesn't consume the
        key, and it's only checked while the client, a panel window or the game is the foreground
        window, so Ctrl+X stays cut everywhere else."""
        state = ctypes.windll.user32.GetAsyncKeyState
        down = lambda vk: bool(state(vk) & 0x8000)
        was = False  # the combo was held at the last check
        while True:
            time.sleep(KEY_POLL)
            try:
                game, front = tracker.find_game(), tracker.foreground_root()
                if not front or front not in (game, tracker.find_client(), tracker.find_window(TITLE),
                                              tracker.find_window(GAME_TITLE)):
                    was = False
                    continue
                now = (down(VK_CONTROL) and not down(VK_MENU) and not down(VK_SHIFT)
                       and down(ord(TOGGLE_KEY)))
                if now and not was:
                    log.info("key Ctrl+%s (stage %s)", TOGGLE_KEY, self.ctl.session.stage.value)
                    if game:
                        threading.Thread(target=self.ctl.toggle_ingame, daemon=True).start()
                    else:
                        self.toggle_panel()
                was = now
            except Exception:
                log.exception("in-game keys")
                time.sleep(1.0)

    # -- startup ----------------------------------------------------------
    def boot(self) -> None:
        for win in (self.window, self.game_window):
            win.events.loaded.wait(15)
        for target in (self.track, self.hotkey, self.toggle_keys, self.ctl.live_loop):
            if target == self.ctl.live_loop and (self.args.items or self.args.players):
                continue  # simulated game: polling would replace it with "no game"
            threading.Thread(target=target, daemon=True).start()
        self.tray.start()
        if not self.always_show:
            threading.Thread(target=self.check_updates, daemon=True).start()
        try:
            self.ctl.boot()
            if self.args.simulate:
                self.run_simulate()
            elif self.args.replay:
                self.run_replay(self.args.replay)
            else:
                self.run_live()
        except Exception:
            log.exception("fatal")
            self.ctl.fail("Something went wrong (see app.log)")

    def run(self) -> None:
        tracker.make_dpi_aware()
        self.window = webview.create_window(  # placed by the tracker where you left it
            TITLE, url=str(RESOURCES / "panel" / "index.html"), js_api=Api(self),
            x=0, y=0,  # any position: makes WinForms keep the tracker's instead of centering on show
            width=300, height=800, min_size=(tracker.MIN_WIDTH, 360), hidden=True,
            background_color="#010a13",
        )
        self.window.events.closing += self._on_panel_closing
        self.game_window = webview.create_window(  # sized and placed by the tracker over the game
            GAME_TITLE, url=str(RESOURCES / "panel" / "game.html"),
            width=620, height=600, frameless=True, on_top=True, focus=False,
            resizable=False, hidden=True, background_color="#010a13", easy_drag=False,
        )
        self._install_quit_handler()
        # persistent storage so the settings page can remember things (e.g. the comfort weight);
        # its page cache is dropped first, or an updated app could keep showing old panel files
        for cache in ("Cache", "Code Cache"):
            shutil.rmtree(DATA_DIR / "webview" / "EBWebView" / "Default" / cache, ignore_errors=True)
        webview.start(self.boot, private_mode=False, storage_path=str(DATA_DIR / "webview"))
        os._exit(0)  # worker threads may be blocked in network/GetMessage calls

    def _install_quit_handler(self) -> None:
        """Ctrl+C / closing the terminal quits. Python's SIGINT handler can't run while the main
        thread sits in the native window loop, but Windows calls this on its own thread."""
        @HANDLER_ROUTINE
        def on_console_event(event: int) -> bool:
            if event in (CTRL_C_EVENT, CTRL_BREAK_EVENT, CTRL_CLOSE_EVENT):
                self.quit()  # same as the tray's Quit, so a downloaded update is still applied
                return True
            return False

        self._quit_handler = on_console_event  # keep a reference so ctypes doesn't free it
        ctypes.windll.kernel32.SetConsoleCtrlHandler(on_console_event, True)


def main() -> None:
    updater.startup()  # Velopack hooks first; may exit the process
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--config")
    ap.add_argument("--record", type=Path, help="append each live session update to this .jsonl file")
    ap.add_argument("--replay", type=Path, help="replay a recorded .jsonl draft")
    ap.add_argument("--delay", type=float, default=1.0)
    ap.add_argument("--simulate", action="store_true")
    ap.add_argument("--role", choices=LANES, help="simulate: your assigned role")
    ap.add_argument("--allies", default="")
    ap.add_argument("--enemies", default="")
    ap.add_argument("--bans", default="")
    ap.add_argument("--items", type=lambda s: [int(i) for i in s.split(",")],
                    help="simulate: item ids you hold; shows the in-game screen's build instead of the draft")
    ap.add_argument("--players", action="store_true",
                    help="simulate: shows the in-game screen's players (from tests/fixtures/players)")
    ap.add_argument("--champion", default="Leona", help="simulate in game: your champion")
    ap.add_argument("--level", type=int, default=9)
    ap.add_argument("--gold", type=float, default=650)
    args = ap.parse_args()
    if args.simulate and not args.role:
        ap.error("--simulate needs --role")
    DATA_DIR.mkdir(parents=True, exist_ok=True)
    logging.basicConfig(
        filename=DATA_DIR / "app.log", level=logging.INFO,
        format="%(asctime)s %(levelname)s %(name)s: %(message)s",
    )
    live = not (args.replay or args.simulate)
    if live and not claim_single_instance():
        log.info("already running: showed the other copy's panel")
        return
    app = App(args)
    if live:
        threading.Thread(target=app.wait_show_requests, daemon=True).start()
    app.run()


def claim_single_instance() -> bool:
    """True if this is the only live copy. Otherwise ask the running copy to show its panel."""
    k32 = ctypes.windll.kernel32
    k32.CreateMutexW.restype = k32.CreateEventW.restype = k32.OpenEventW.restype = w.HANDLE
    global _instance_mutex  # held until the process exits
    _instance_mutex = k32.CreateMutexW(None, False, INSTANCE_MUTEX)
    if k32.GetLastError() != ERROR_ALREADY_EXISTS:
        return True
    event = k32.OpenEventW(EVENT_MODIFY_STATE, False, SHOW_EVENT)
    if event:
        k32.SetEvent(event)
        k32.CloseHandle(event)
    return False


if __name__ == "__main__":
    main()
