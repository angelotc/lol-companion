"""Every player's public profile stats for the game in progress, read from the League client (GET only).

These are the same numbers the client's own profile pages show (rank, recent games, mastery), so
nothing hidden about the game itself is used. The client keeps only about the last 20 games per
player, so the champion numbers cover recent form, not the whole season.
"""
from __future__ import annotations

import logging
from concurrent.futures import ThreadPoolExecutor
from dataclasses import dataclass, field
from typing import Callable
from urllib.parse import quote

from data.lcu import LcuClient
from data.live import POSITIONS

log = logging.getLogger(__name__)
# Summoner's Rift queues (normals, ranked, quickplay, clash); ARAM and modes don't count
SR_QUEUES = {400, 420, 430, 440, 480, 490, 700}
RANKED = ("RANKED_SOLO_5x5", "RANKED_FLEX_SR")
TIER_ORDER = ["IRON", "BRONZE", "SILVER", "GOLD", "PLATINUM", "EMERALD", "DIAMOND", "MASTER", "GRANDMASTER", "CHALLENGER"]
APEX = {"MASTER", "GRANDMASTER", "CHALLENGER"}  # no divisions
STREAK = 3  # this many wins or losses in a row is a streak
MAIN = 5    # this many of the recent games on the champion makes it their main
FIRST_TIME = 1_500  # fewer mastery points than this, and no recent games on it: a first-timer


@dataclass
class PlayerRef:
    puuid: str
    name: str
    tag: str
    champion_id: int
    team: int            # 100 or 200
    position: str        # lane name, "" if unknown
    is_me: bool = False


@dataclass
class PlayerStats:
    ref: PlayerRef
    tier: str = ""       # "GOLD", "" when unranked
    division: str = ""   # "II", "" for Master+
    lp: int = 0
    wins: int = 0        # this season, in the queue `queue`
    losses: int = 0
    queue: str = ""      # "solo" | "flex" | ""
    games: int = 0       # recent Summoner's Rift games read
    champ_games: int = 0
    champ_wins: int = 0
    kills: int = 0       # on this champion, summed over champ_games
    deaths: int = 0
    assists: int = 0
    recent: list[bool] = field(default_factory=list)  # newest first, all champions
    mastery: int | None = None  # points on this champion, None if the client didn't say
    tags: list[str] = field(default_factory=list)
    error: bool = False  # some lookups failed: numbers may be partial

    @property
    def kda(self) -> float | None:
        if not self.champ_games:
            return None
        return (self.kills + self.assists) / max(1, self.deaths)


def _lane(pos) -> str:
    return POSITIONS.get(str(pos or "").upper(), "")


def parse_teams(session: dict, my_puuid: str = "") -> list[PlayerRef]:
    """The 10 players from GET /lol-gameflow/v1/session (gameData.teamOne / teamTwo)."""
    gd = (session or {}).get("gameData") or {}
    out = []
    for team, key in ((100, "teamOne"), (200, "teamTwo")):
        for p in gd.get(key) or []:
            if not isinstance(p, dict) or not p.get("puuid"):
                continue
            out.append(PlayerRef(
                puuid=str(p["puuid"]),
                name=str(p.get("gameName") or p.get("riotIdGameName") or p.get("summonerName") or ""),
                tag=str(p.get("tagLine") or p.get("riotIdTagLine") or ""),
                champion_id=int(p.get("championId") or 0), team=team,
                position=_lane(p.get("selectedPosition")), is_me=bool(my_puuid) and p["puuid"] == my_puuid,
            ))
    # champions aren't always filled in yet; take them from the champ select picks
    picks = {str(c.get("puuid")): int(c.get("championId") or 0)
             for c in gd.get("playerChampionSelections") or [] if isinstance(c, dict)}
    for r in out:
        if not r.champion_id:
            r.champion_id = picks.get(r.puuid, 0)
    return out


def refs_from_live(data: dict, champion_id: Callable[[str], int | None]) -> list[PlayerRef]:
    """Fallback when the client has no teams: the game's own player list (no puuid yet)."""
    ap = (data or {}).get("activePlayer") or {}
    me = ap.get("riotId") or ap.get("summonerName")
    out = []
    for p in (data or {}).get("allPlayers") or []:
        rid = str(p.get("riotId") or p.get("summonerName") or "")
        name, _, tag = rid.partition("#")
        out.append(PlayerRef(
            puuid="", name=str(p.get("riotIdGameName") or name), tag=str(p.get("riotIdTagLine") or tag),
            champion_id=champion_id(p.get("championName", "")) or 0,
            team=100 if p.get("team") == "ORDER" else 200, position=_lane(p.get("position")), is_me=rid == me,
        ))
    return out


def parse_ranked(data: dict) -> dict:
    """Solo queue rank from GET /lol-ranked/v1/ranked-stats/{puuid}; flex when unranked in solo."""
    qmap = (data or {}).get("queueMap") or {}
    for key, label in zip(RANKED, ("solo", "flex")):
        q = qmap.get(key) or {}
        tier = str(q.get("tier") or "").upper()
        if tier in TIER_ORDER:
            div = str(q.get("division") or "").upper()
            return {"tier": tier, "division": "" if tier in APEX or div in ("NA", "NONE") else div,
                    "lp": int(q.get("leaguePoints") or 0), "wins": int(q.get("wins") or 0),
                    "losses": int(q.get("losses") or 0), "queue": label}
    return {}


def summarize_history(data: dict, champion_id: int) -> dict:
    """Recent Summoner's Rift games from GET /lol-match-history/v1/products/lol/{puuid}/matches:
    the win/loss run (newest first) and the record and K/D/A on `champion_id`."""
    games = ((data or {}).get("games") or {}).get("games") or []
    games = sorted((g for g in games if isinstance(g, dict) and g.get("queueId") in SR_QUEUES),
                   key=lambda g: -(g.get("gameCreation") or 0))
    out = {"games": 0, "recent": [], "champ_games": 0, "champ_wins": 0, "kills": 0, "deaths": 0, "assists": 0}
    for g in games:
        parts = g.get("participants") or []
        if len(parts) != 1:  # the list endpoint includes only the player asked for
            continue
        p = parts[0]
        st = p.get("stats") or {}
        win = bool(st.get("win"))
        out["games"] += 1
        out["recent"].append(win)
        if int(p.get("championId") or 0) == champion_id:
            out["champ_games"] += 1
            out["champ_wins"] += win
            for k in ("kills", "deaths", "assists"):
                out[k] += int(st.get(k) or 0)
    return out


def parse_mastery(data, champion_id: int) -> int | None:
    """Points on `champion_id` from GET /lol-champion-mastery/v1/{puuid}/champion-mastery."""
    if not isinstance(data, list):
        return None
    for m in data:
        if isinstance(m, dict) and int(m.get("championId") or 0) == champion_id:
            return int(m.get("championPoints") or 0)
    return 0  # a full list without the champion: never played it


def tags(s: PlayerStats) -> list[str]:
    out = []
    if s.champ_games == 0 and s.mastery is not None and s.mastery < FIRST_TIME:
        out.append("First time")
    elif s.champ_games >= MAIN:
        out.append("Main")
    run = 0
    for w in s.recent:
        if w != s.recent[0]:
            break
        run += 1
    if run >= STREAK:
        out.append(f"{run} {'wins' if s.recent[0] else 'losses'} in a row")
    return out


def _json(client: LcuClient, path: str):
    r = client.get(path)
    if r.status_code != 200:
        raise LookupError(f"{path}: HTTP {r.status_code}")
    return r.json()


def resolve(client: LcuClient, ref: PlayerRef) -> None:
    """Fill in a missing puuid from the Riot ID."""
    if ref.puuid or not ref.name or not ref.tag:
        return
    data = _json(client, f"/lol-summoner/v1/alias/lookup?gameName={quote(ref.name)}&tagLine={quote(ref.tag)}")
    ref.puuid = str((data or {}).get("puuid") or "")


def load(client: LcuClient, ref: PlayerRef) -> PlayerStats:
    """One player's stats; each lookup that fails leaves its numbers empty and sets `error`."""
    s = PlayerStats(ref)
    try:
        resolve(client, ref)
    except Exception:
        log.debug("puuid lookup failed for %s#%s", ref.name, ref.tag, exc_info=True)
    if not ref.puuid:
        s.error = True
        return s
    pid = quote(ref.puuid)
    steps = (
        (f"/lol-ranked/v1/ranked-stats/{pid}", lambda d: parse_ranked(d)),
        (f"/lol-match-history/v1/products/lol/{pid}/matches?begIndex=0&endIndex=19",
         lambda d: summarize_history(d, ref.champion_id)),
        (f"/lol-champion-mastery/v1/{pid}/champion-mastery", lambda d: {"mastery": parse_mastery(d, ref.champion_id)}),
    )
    for path, parse in steps:
        try:
            for k, v in parse(_json(client, path)).items():
                setattr(s, k, v)
        except Exception:
            log.debug("player lookup failed: %s", path, exc_info=True)
            if "mastery" not in path:  # other players' mastery may simply not be served
                s.error = True
    s.tags = tags(s)
    return s


def game_refs(client: LcuClient, live: dict | None = None,
              champion_id: Callable[[str], int | None] = lambda _: None) -> tuple[int, list[PlayerRef]]:
    """(game id, the 10 players), from the client's game session, else from the game's player list."""
    session = _json(client, "/lol-gameflow/v1/session") or {}
    try:
        me = str((_json(client, "/lol-summoner/v1/current-summoner") or {}).get("puuid") or "")
    except Exception:
        me = ""
    game_id = int(((session.get("gameData") or {}).get("gameId")) or 0)
    refs = parse_teams(session, me)
    if not refs and live:
        refs = refs_from_live(live, champion_id)
    return game_id, refs


def load_all(client: LcuClient, refs: list[PlayerRef], workers: int = 4) -> list[PlayerStats]:
    with ThreadPoolExecutor(workers) as pool:
        return list(pool.map(lambda r: load(client, r), refs))


class FixtureClient:
    """Answers the lookups above from saved JSON (`--simulate --players`, tests).

    <folder>/session.json is the gameflow session; ranked.json, history.json and mastery.json map
    puuid -> that endpoint's response. A missing puuid answers 404.
    """

    def __init__(self, folder):
        import json
        from pathlib import Path
        load = lambda n: json.loads((Path(folder) / n).read_text(encoding="utf-8"))
        self.session = load("session.json")
        self.by = {k: load(f"{k}.json") for k in ("ranked", "history", "mastery")}

    def get(self, path: str):
        import httpx
        from urllib.parse import unquote
        if path == "/lol-gameflow/v1/session":
            return httpx.Response(200, json=self.session)
        if path == "/lol-summoner/v1/current-summoner":
            return httpx.Response(200, json={"puuid": self.session.get("_me", "")})
        for key, prefix in (("ranked", "/lol-ranked/v1/ranked-stats/"),
                            ("history", "/lol-match-history/v1/products/lol/"),
                            ("mastery", "/lol-champion-mastery/v1/")):
            if path.startswith(prefix):
                puuid = unquote(path[len(prefix):].split("/")[0].split("?")[0])
                if puuid in self.by[key]:
                    return httpx.Response(200, json=self.by[key][puuid])
        return httpx.Response(404)

    def close(self) -> None:
        pass
