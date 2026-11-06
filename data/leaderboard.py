"""Best players on a champion, from lolalytics' leaderboard page (lolalytics.com/lol/<slug>/leaderboard/).

The page is server-rendered HTML, one row per player: rank, tier, Riot ID, region, win rate, games,
and a link to the player's xdx.gg profile. No JSON endpoint is known, so the rows are scraped; if
lolalytics changes its markup this returns [] rather than guessing.

Prototype: `uv run python -m data.leaderboard` lists the top players on your most played champions
(from your stored games), or `... --champ rell --champ lulu` for any champions.
"""
from __future__ import annotations

import argparse
import html
import re
from dataclasses import dataclass
from urllib.parse import quote

from data.fetch import Fetcher

URL = "https://lolalytics.com/lol/{slug}/leaderboard/"
ROW = re.compile(r'<div class="flex h-\[52px\] justify-between[^"]*"[^>]*>')
TIERS = {"CH": "Challenger", "GM": "Grandmaster", "M": "Master", "D": "Diamond"}
# lolalytics region -> op.gg region (same short codes on both, listed so unknown ones are skipped)
OPGG = {"na", "euw", "eune", "kr", "jp", "br", "lan", "las", "oce", "tr", "ru", "vn", "tw", "sg", "ph", "th", "me"}


@dataclass
class Player:
    rank: int
    tier: str      # "CH", "GM", ...
    riot_id: str   # "Name#TAG"
    region: str    # "euw"
    wr: float      # %
    games: int
    profile: str = ""  # xdx.gg link from the page

    @property
    def opgg(self) -> str:
        if self.region not in OPGG or "#" not in self.riot_id:
            return ""
        name, tag = self.riot_id.rsplit("#", 1)
        return f"https://op.gg/lol/summoners/{self.region}/{quote(name)}-{quote(tag)}"


def _cells(row: str) -> list[str]:
    return [t for t in (html.unescape(c).strip() for c in re.findall(r">([^<>]+)<", row)) if t]


def parse(page: str) -> list[Player]:
    """Leaderboard rows from the page HTML, best first. Rows that don't look right are skipped."""
    starts = [m.start() for m in ROW.finditer(page)] + [len(page)]
    out = []
    for a, b in zip(starts, starts[1:]):
        row = page[a:b]
        c = _cells(row)
        link = re.search(r'<a href="([^"]+)"', row)
        try:
            rank, tier, rid, region, wr, games = c[0], c[1], c[2], c[3], c[4], c[5]
            out.append(Player(int(rank), tier, rid, region.lower(), float(wr.rstrip("%")), int(games),
                              link.group(1) if link else ""))
        except (IndexError, ValueError):
            continue
    return out


def top_players(slug: str, fetcher: Fetcher | None = None, n: int = 10) -> list[Player]:
    fetcher = fetcher or Fetcher()
    return parse(fetcher.get(URL.format(slug=slug)).text)[:n]


def main() -> None:
    ap = argparse.ArgumentParser(description="Top players on your best champions (lolalytics leaderboard)")
    ap.add_argument("--champ", action="append", help="champion slug, e.g. rell (repeatable)")
    ap.add_argument("--top", type=int, default=3, help="how many of your champions (default 3)")
    ap.add_argument("-n", type=int, default=5, help="players per champion (default 5)")
    ap.add_argument("--min-games", type=int, default=5, help="your games needed to count a champion")
    args = ap.parse_args()

    from data.champions import Champions
    champs = Champions.load()
    if args.champ:
        picks = [(s, "") for s in args.champ]
    else:
        from data.store import Store
        rec = Store().champ_record()
        best = sorted(((g, w, c) for c, (g, w, _, _) in rec.items() if g >= args.min_games), reverse=True)
        picks = [(champs.slug(c), f"you: {g} games, {100 * w / g:.0f}% WR") for g, w, c in best[: args.top]]
        if not picks:
            print("No stored games yet: pass --champ <slug>.")
            return
    fetcher = Fetcher()
    for slug, you in picks:
        print(f"\n{slug}  {you}".rstrip())
        players = top_players(slug, fetcher, args.n)
        if not players:
            print("  (no leaderboard rows found)")
        for p in players:
            print(f"  {p.rank:>2}. {p.tier:<2} {p.riot_id:<28} {p.region:<4} {p.wr:5.1f}% {p.games:>4} games  {p.opgg}")


if __name__ == "__main__":
    main()
