"""Season ranked record per champion for one player, from op.gg (unofficial: read from the page's
own data, so it may break when op.gg changes its site).

op.gg's numbers are a snapshot from the player's last refresh on op.gg, usually close to current.
Cached for TTL, so each player costs at most one page load per few hours.
"""
from __future__ import annotations

import json
import logging
from pathlib import Path
from urllib.parse import quote

import httpx

from data.cache import JsonCache
from paths import CACHE_DIR

log = logging.getLogger(__name__)
BASE = "https://op.gg"
TTL = 6 * 3600
HEADERS = {"User-Agent": "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) "
                         "Chrome/124.0 Safari/537.36", "Accept": "text/html"}
KEY = '"my_champion_stats":'


def _array_at(s: str, start: int) -> list | None:
    """The JSON array starting at the first '[' after `start` (string-aware bracket match)."""
    j = s.find("[", start)
    if j < 0:
        return None
    depth, in_str, esc = 0, False, False
    for k in range(j, len(s)):
        ch = s[k]
        if in_str:
            if esc:
                esc = False
            elif ch == "\\":
                esc = True
            elif ch == '"':
                in_str = False
        elif ch == '"':
            in_str = True
        elif ch == "[":
            depth += 1
        elif ch == "]":
            depth -= 1
            if depth == 0:
                return json.loads(s[j:k + 1])
    return None


def parse_champions(html: str) -> dict[int, list[int]] | None:
    """{champion id: [games, wins]} from an op.gg champions page; None if the data isn't there."""
    s = html.replace('\\"', '"')
    i = s.find(KEY)
    if i < 0:
        return None
    try:
        arr = _array_at(s, i + len(KEY))
    except ValueError:
        return None
    if arr is None:
        return None
    out = {}
    for a in arr:
        cid = int(a.get("champion_id") or a.get("id") or 0) if isinstance(a, dict) else 0
        if cid:  # id 0 is the all-champions total
            out[cid] = [int(a.get("play") or 0), int(a.get("win") or 0)]
    return out


class OpGG:
    def __init__(self, cache_dir: Path = CACHE_DIR / "opgg", ttl: float = TTL, timeout: float = 10.0):
        self.cache = JsonCache(cache_dir, ttl)
        self.http = httpx.Client(headers=HEADERS, timeout=timeout, follow_redirects=True)

    def _slug(self, name: str, tag: str, region: str) -> str | None:
        """op.gg's own path for a Riot ID (handles spaces and odd characters)."""
        r = self.http.get(f"{BASE}/api/lol/summoners/resolve", params={"q": f"{name}#{tag}", "region": region},
                          headers={"Accept": "application/json"})
        if r.status_code != 200:
            return None
        for line in r.text.splitlines():
            try:
                hit = json.loads(line)
            except ValueError:
                continue
            if hit.get("type") == "summoner" and hit.get("href"):
                return hit["href"]
        return None

    def _fetch(self, name: str, tag: str, region: str) -> dict | None:
        direct = f"/summoners/{region}/{quote(name)}-{quote(tag)}"
        for path in (direct, None):
            path = path or self._slug(name, tag, region)
            if not path:
                return None
            r = self.http.get(f"{BASE}/lol{path}/champions", params={"queue_type": "SOLORANKED"})
            if r.status_code == 200:
                champs = parse_champions(r.text)
                if champs is not None:
                    return {"champions": {str(k): v for k, v in champs.items()}}
        return None

    def season(self, name: str, tag: str, region: str) -> dict[int, list[int]] | None:
        """{champion id: [games, wins]} this season in solo queue, or None if op.gg can't say."""
        if not name or not tag or not region:
            return None
        region = region.lower()
        data = self.cache.get(f"{region}/{name}#{tag}".lower(), lambda: self._fetch(name, tag, region), "season")
        if "champions" not in data:
            return None
        return {int(k): v for k, v in data["champions"].items()}

    def close(self) -> None:
        self.http.close()
