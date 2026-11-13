"""Render the panel's two screens from a simulated draft, through the real controller and lolalytics data.

  uv run scripts/simulate_views.py              writes docs/views/{bans,picks,build,ingame,explore}.png
  uv run scripts/simulate_views.py --out DIR

No League client needed: the draft, lock-in, game and lobby are simulated; the in-game players come
from tests/fixtures/players. Bans, picks, build and explore are the client screen (index.html) at each
stage; ingame is the game screen (game.html).
"""
from __future__ import annotations

import argparse
import copy
import json
import sys
import tempfile
from dataclasses import replace
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
sys.path.insert(0, str(ROOT))
sys.path.insert(0, str(ROOT / "scripts"))

from config import load_config  # noqa: E402
from data.lcu import DraftState, Slot  # noqa: E402
from data.live import LiveGame  # noqa: E402
from data.players import FixtureClient, game_refs, load_all  # noqa: E402
from desktop import tracker  # noqa: E402
from draft import views  # noqa: E402
from draft.controller import Controller  # noqa: E402
from draft.state import Stage  # noqa: E402
from render_panel import render  # noqa: E402

ROLE = "support"
ALLIES = [("ksante", "top", True), ("leesin", "jungle", False), ("ahri", "middle", False), ("jinx", "bottom", False)]
ENEMIES = ["caitlyn", "nautilus", "hwei", "viego"]
BANS = ["pyke", "blitzcrank", "yunara", "zed", "aatrox"]
LOCK = "leona"


def main() -> None:
    ap = argparse.ArgumentParser()
    ap.add_argument("--out", type=Path, default=ROOT / "docs" / "views")
    ap.add_argument("--items", type=lambda s: [int(i) for i in s.split(",")], default=[3158, 3109, 3190],
                    help="in-game item ids you hold (default: Ionian Boots, Knight's Vow, Locket)")
    args = ap.parse_args()
    out_dir = args.out
    out_dir.mkdir(parents=True, exist_ok=True)
    states: list[dict] = []
    ctl = Controller(load_config(), states.append)
    ctl.boot()
    ids = lambda *names: [ctl.rec.champs.id_from_slug(n) for n in names]
    shots: dict[str, dict] = {}  # name -> render() keyword arguments

    # ban phase: bans above the picks so far
    allies = [Slot(c, lane, tent, False) for c, (_, lane, tent) in zip(ids(*(a[0] for a in ALLIES)), ALLIES)]
    me = Slot(0, ROLE, False, True)
    draft = DraftState(my_position=ROLE, allies=allies[:2] + [me], bans=set(ids(*BANS[:3])))
    ctl.session.phase, ctl.session.draft, ctl.session.banning = "ChampSelect", draft, True
    ctl.render()
    shots["bans"] = dict(state=states[-1], click=[".threat"], height=1570)

    # picks, with the draft win estimate (load the 10 champions' stats first, as the app does in the background)
    draft = DraftState(my_position=ROLE, allies=allies + [me], enemies=ids(*ENEMIES), bans=set(ids(*BANS)),
                       picked=set(ids(*(a[0] for a in ALLIES if not a[2]))) | set(ids(*ENEMIES)))
    ctl.session.draft, ctl.session.banning = draft, False
    ctl.render()
    ev = ctl.last_eval
    ours, theirs = ctl.rec.odds_inputs(ev)
    for lane, (cid, _) in list(ours.items()) + list(theirs.items()):
        ctl.rec.lane_stats(cid, lane)
    ctl.render()
    # the README shows champ select beside the other two views at one height: crop to the teams
    # and the first picks rather than the full scroll
    shots["picks"] = dict(state=states[-1], click=[".rec"], height=700)

    # locked in: build with matchup deltas, the picks from lock-in below it
    leona = ids(LOCK)[0]
    locked = replace(draft, allies=allies + [Slot(leona, ROLE, False, True)], my_locked=leona, picked=draft.picked | {leona})
    ctl.session.draft, ctl.session.frozen = locked, replace(locked, picked=locked.picked - {leona})
    assert ctl.session.stage == Stage.LOCKED
    ctl.render()
    shots["build"] = dict(state=states[-1], height=2870)

    # in game (Ctrl+X): Leona at level 13 with boots, Knight's Vow and Locket beside the 10 players; hovering a matchup cell
    ctl.session.draft = ctl.session.frozen = None
    ctl.session.phase, ctl.ingame_shown = "InProgress", True
    enemies = dict(zip(["bottom", "support", "middle", "jungle"], ["Caitlyn", "Nautilus", "Hwei", "Viego"]))
    ctl.live = LiveGame("Leona", ROLE, 13, 400, args.items, 1500, {}, enemies)
    client = FixtureClient(ROOT / "tests" / "fixtures" / "players")
    _, refs = game_refs(client)
    ctl.players = (1, load_all(client, refs))
    ctl.render()
    shots["ingame"] = dict(state=states[-1], page="game.html", width=827, height=806, scale=1.333, mode="ingame",
                           hover=".ig-opt .ig-vs b[title]")

    # between games: the explorer section in the wide out-of-game window (the League client's size;
    # 540 is the page's design height there, so scale = window height / 540)
    ctl.session.phase, ctl.ingame_shown, ctl.live = "Lobby", False, None
    ctl.render()
    explore = ctl.rec.blind_report(ROLE, ctl.tier) | {"tier_label": views.TIERS[ctl.tier]}
    w, h = tracker.EXPLORE_SIZE
    shots["explore"] = dict(state=states[-1], explore=explore, width=w, height=h, scale=h / 540)

    for name, kw in shots.items():
        path = render(out_dir / f"{name}.png", **kw)
        print(name, kw["state"].get("view"), path)


if __name__ == "__main__":
    main()
