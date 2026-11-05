"""Save the League client's raw answers for every player in the game in progress, in the format
data.players.FixtureClient reads, so the parsers can be checked against real responses.

  uv run scripts/dump_players.py [out folder]   (default: drafts/players-<time>)
"""
import json
import sys
import time
from pathlib import Path
from urllib.parse import quote

sys.path.insert(0, str(Path(__file__).resolve().parent.parent))

from config import load_config
from data import players
from data.lcu import LcuClient, LcuError, read_lockfile
from data.live import LiveClient


def main() -> int:
    out = Path(sys.argv[1]) if len(sys.argv) > 1 else Path("drafts") / time.strftime("players-%Y%m%d-%H%M%S")
    try:
        client = LcuClient(read_lockfile(load_config().lockfile))
        session = client.get("/lol-gameflow/v1/session").json()
        me = client.get("/lol-summoner/v1/current-summoner").json()
    except (LcuError, Exception) as e:
        print(f"client not running or unreachable: {e}")
        return 1
    refs = players.parse_teams(session, str(me.get("puuid") or ""))
    if not refs:
        print("no teams in the gameflow session; trying the game's player list")
        refs = players.refs_from_live(LiveClient().raw() or {}, lambda _: None)
        for r in refs:
            try:
                players.resolve(client, r)
            except Exception as e:
                print(f"  {r.name}#{r.tag}: puuid lookup failed ({e})")
    if not refs:
        print("not in a game")
        return 0
    found = {"ranked": {}, "history": {}, "mastery": {}}
    paths = {"ranked": "/lol-ranked/v1/ranked-stats/{}",
             "history": "/lol-match-history/v1/products/lol/{}/matches?begIndex=0&endIndex=19",
             "mastery": "/lol-champion-mastery/v1/{}/champion-mastery"}
    for r in refs:
        if not r.puuid:
            continue
        line = []
        for key, path in paths.items():
            resp = client.get(path.format(quote(r.puuid)))
            line.append(f"{key} {resp.status_code}")
            if resp.status_code == 200:
                found[key][r.puuid] = resp.json()
        print(f"{r.name}#{r.tag} ({'me' if r.is_me else r.team}): " + ", ".join(line))
    out.mkdir(parents=True, exist_ok=True)
    session["_me"] = me.get("puuid", "")
    (out / "session.json").write_text(json.dumps(session, indent=1), encoding="utf-8")
    for key, data in found.items():
        (out / f"{key}.json").write_text(json.dumps(data, indent=1), encoding="utf-8")
    print(f"saved to {out}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
