"""Render the panel's two screens from a simulated draft, through the real controller and lolalytics data.

  uv run scripts/simulate_views.py              writes docs/views/{bans,blind,picks,build,ingame,explore}.png
  uv run scripts/simulate_views.py --out DIR

No League client needed: the draft, lock-in, game and lobby are simulated; the in-game players come
from tests/fixtures/players, re-cast onto the simulated draft's champions. Bans, blind, picks and
build are champ select's four views (1a-1d), explore the screen between games: all the client
screen (index.html), in the panel window's default size; ingame is the game screen (game.html).
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
ENEMIES = [("caitlyn", "bottom"), ("nautilus", "support"), ("hwei", "middle"), ("viego", "jungle")]
BANS = ["pyke", "blitzcrank", "yunara", "zed", "aatrox"]
LOCK = "leona"
POSITIONS = {"top": "TOP", "jungle": "JUNGLE", "middle": "MIDDLE", "bottom": "BOTTOM", "support": "UTILITY"}


def recast(client: FixtureClient, ours: dict[str, int], theirs: dict[str, int]) -> None:
    """Put the fixture's players on the draft's champions (lane -> champion id per team; you on
    ROLE): each player keeps their own rank and record, moved onto their new champion."""
    game, me = client.session["gameData"], client.session["_me"]
    you = next(p for p in game["teamOne"] if p["puuid"] == me)
    other = next((p for p in game["teamOne"] if p["selectedPosition"] == POSITIONS[ROLE]), you)
    other["selectedPosition"], you["selectedPosition"] = you["selectedPosition"], POSITIONS[ROLE]
    for team, cast in (("teamOne", ours), ("teamTwo", theirs)):
        for p in game[team]:
            lane = next((l for l, pos in POSITIONS.items() if pos == p["selectedPosition"]), None)
            new, old = cast.get(lane), p["championId"]
            if not new:
                continue
            p["championId"] = new
            for g in client.by["history"].get(p["puuid"], {}).get("games", {}).get("games", []):
                for part in g["participants"]:
                    if part["championId"] == old:
                        part["championId"] = new
            for m in client.by["mastery"].get(p["puuid"], []):
                if m["championId"] == old:
                    m["championId"] = new


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
    w, h = tracker.PAGE_SIZE  # the panel window's default page; it zooms itself to fit (client.js)
    enemy_ids = ids(*(e[0] for e in ENEMIES))

    # 1a ban phase
    allies = [Slot(c, lane, tent, False) for c, (_, lane, tent) in zip(ids(*(a[0] for a in ALLIES)), ALLIES)]
    me = Slot(0, ROLE, False, True)
    draft = DraftState(my_position=ROLE, allies=allies[:2] + [me], bans=set(ids(*BANS[:3])))
    ctl.session.phase, ctl.session.draft, ctl.session.banning = "ChampSelect", draft, True
    ctl.render()
    shots["bans"] = dict(state=states[-1], width=w, height=h)

    # picks, with the draft win estimate (load the 10 champions' stats first, as the app does in the background)
    draft = DraftState(my_position=ROLE, allies=allies + [me], enemies=enemy_ids, bans=set(ids(*BANS)),
                       picked=set(ids(*(a[0] for a in ALLIES if not a[2]))) | set(enemy_ids))
    ctl.session.draft, ctl.session.banning = draft, False
    ctl.render()
    ev = ctl.last_eval
    ours, theirs = ctl.rec.odds_inputs(ev)
    for lane, (cid, _) in list(ours.items()) + list(theirs.items()):
        ctl.rec.lane_stats(cid, lane)

    # 1b blind pick: the enemy support (your lane opponent) not picked yet
    lane_opp = [c for c, (_, lane) in zip(enemy_ids, ENEMIES) if lane == ROLE]
    ctl.session.draft = replace(draft, enemies=[c for c in enemy_ids if c not in lane_opp],
                                picked=draft.picked - set(lane_opp))
    ctl.render()
    shots["blind"] = dict(state=states[-1], width=w, height=h)

    # 1c best picks; hovering a matchup cell of the top pick: its column lights up, the tooltip explains the cell
    ctl.session.draft = draft
    ctl.render()
    shots["picks"] = dict(state=states[-1], width=w, height=h, hover="#recs .mr:nth-child(5) .mc[data-c='0'] .mval")

    # 1d locked in: build with matchup deltas (the picks from lock-in stay under 1c)
    leona = ids(LOCK)[0]
    locked = replace(draft, allies=allies + [Slot(leona, ROLE, False, True)], my_locked=leona, picked=draft.picked | {leona})
    ctl.session.draft, ctl.session.frozen = locked, replace(locked, picked=locked.picked - {leona})
    assert ctl.session.stage == Stage.LOCKED
    ctl.render()
    shots["build"] = dict(state=states[-1], width=w, height=h)

    # in game (Ctrl+X): you as Leona at level 13 with boots, Knight's Vow and Locket beside the 10
    # players of this draft; hovering a matchup cell. Double density for the README (the page's
    # own 1440p scale, doubled): displayed at ~700px it downsamples crisp
    ctl.session.draft = ctl.session.frozen = None
    ctl.session.phase, ctl.ingame_shown = "InProgress", True
    champ_name = ctl.rec.champs.name
    ctl.live = LiveGame(champ_name(leona), ROLE, 13, 400, args.items, 1500, {},
                        {lane: champ_name(c) for c, (_, lane) in zip(enemy_ids, ENEMIES)})
    client = FixtureClient(ROOT / "tests" / "fixtures" / "players")
    recast(client, {lane: c for c, (_, lane, _) in zip(ids(*(a[0] for a in ALLIES)), ALLIES)} | {ROLE: leona},
           {lane: c for c, (_, lane) in zip(enemy_ids, ENEMIES)})
    _, refs = game_refs(client)
    ctl.players = (1, load_all(client, refs))
    ctl.render()
    shots["ingame"] = dict(state=states[-1], page="game.html", width=1654, height=1612, scale=2.667, mode="ingame",
                           hover=".ig-opt .ig-vs b[title]")

    # between games: the explorer section, same window
    ctl.session.phase, ctl.ingame_shown, ctl.live = "Lobby", False, None
    ctl.render()
    explore = ctl.rec.blind_report(ROLE, ctl.tier) | {"tier_label": views.TIERS[ctl.tier]}
    shots["explore"] = dict(state=states[-1], explore=explore, width=w, height=h)

    for name, kw in shots.items():
        path = render(out_dir / f"{name}.png", **kw)
        print(name, kw["state"].get("view"), path)


if __name__ == "__main__":
    main()
