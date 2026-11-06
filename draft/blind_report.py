"""Blind-pick report for one role, for the between-games explorer window: every champion played
enough in the role, with its strength and how it holds up if countered."""
from __future__ import annotations

from data.champions import Champions
from draft.scoring import DEFAULT_BLIND, ChampStats, rank_blind, recommend, shrink


def opponents_of(s: ChampStats, role: str, pool_pr: dict[int, float], k: float, tail: float) -> list[dict]:
    """Every opponent in the role's pool: shrunk matchup delta, raw delta, games, pick rate, and
    whether it falls in the counter-pick zone (the worst `tail` share of picks with data)."""
    pairs = s.matchups.get(role, {})
    rows = []
    for cid, pr in pool_pr.items():
        if cid == s.champion_id:
            continue
        p = pairs.get(cid)
        raw = None if p is None else (p.wr - s.wr if p.delta is None else p.delta)
        d = 0.0 if p is None else shrink(raw, p.games, k)
        rows.append({"id": cid, "pr": pr, "d": d, "raw": raw, "games": 0 if p is None else p.games})
    rows.sort(key=lambda r: r["d"])
    known = [r for r in rows if r["games"]]  # the zone is among opponents with data, as in the score
    need, got = tail * sum(r["pr"] for r in known), 0.0
    for r in rows:
        r["tail"] = bool(r["games"]) and got < need
        got += r["pr"] if r["games"] else 0.0
    return rows


def role_report(pool: dict[int, ChampStats], pool_pr: dict[int, float], role: str, weights: dict,
                k: float, blind_cfg: dict | None = None) -> list[dict]:
    """Champions in `pool` ranked by blind score. pool_pr: enemy id -> pick rate in the role
    (who you might face). Nothing about the current draft: bans and teams aren't known here."""
    cfg = DEFAULT_BLIND | (blind_cfg or {})
    base = {r.champion_id: r.base for r in recommend(list(pool.values()), {}, {}, weights, k)}
    out = []
    for b in rank_blind([(s, base.get(cid, 0.0)) for cid, s in pool.items()], role, pool_pr, k, cfg):
        s = pool[b.champion_id]
        opps = opponents_of(s, role, pool_pr, k, cfg["tail"])
        out.append({"id": b.champion_id, "score": b.score, "base": base.get(b.champion_id, 0.0), "mean": b.mean,
                    "tail": b.tail, "coverage": b.coverage, "opps": opps})
    return out


def payload(report: list[dict], champs: Champions, role: str, tier: str, days: int) -> dict:
    """JSON for the explorer page: rounded numbers, names and icons."""
    r2 = lambda v: None if v is None else round(v, 2)
    who = lambda cid: {"id": cid, "name": champs.name(cid), "icon": champs.icon_url(cid)}
    return {
        "role": role, "tier": tier, "days": days,
        "champs": [
            who(c["id"]) | {k: r2(c[k]) for k in ("score", "base", "mean", "tail")}
            | {"coverage": round(c["coverage"] * 100),
               "opps": [who(o["id"]) | {"pr": r2(o["pr"]), "d": r2(o["d"]), "raw": r2(o["raw"]),
                                        "games": o["games"], "tail": o["tail"]} for o in c["opps"]]}
            for c in report
        ],
    }
