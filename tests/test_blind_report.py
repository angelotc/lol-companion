import sys
from pathlib import Path

import pytest

sys.path.insert(0, str(Path(__file__).resolve().parent.parent))

from desktop.explore_ui import ExploreApi
from draft.blind_report import opponents_of, role_report
from draft.scoring import ChampStats, Pair

W = {"base": 1.0, "enemy": {}, "ally": {}}


def champ(cid, wr, deltas):
    return ChampStats(cid, "support", wr, 20000, avg_wr=50.0,
                      matchups={"support": {e: Pair(50, 20000, d) for e, d in deltas.items()}})


def test_counter_zone_follows_pick_rate():
    s = champ(1, 50, {10: -6.0, 11: 0.0, 12: 2.0})
    rows = opponents_of(s, "support", {10: 10.0, 11: 80.0, 12: 10.0}, k=500, tail=0.15)
    assert [r["id"] for r in rows] == [10, 11, 12]
    assert [r["tail"] for r in rows] == [True, True, False]  # 10% isn't 15%: the next one is partly in


def test_role_report_ranks_safe_over_risky_at_equal_strength():
    pool = {1: champ(1, 52, {10: 0.0, 11: 0.0}), 2: champ(2, 52, {10: 4.0, 11: -6.0}), 10: champ(10, 50, {}),
            11: champ(11, 50, {})}
    rep = role_report(pool, {10: 50.0, 11: 50.0}, "support", W, k=500)
    ids = [c["id"] for c in rep]
    assert ids.index(1) < ids.index(2)
    risky = next(c for c in rep if c["id"] == 2)
    assert risky["tail"] < 0 and risky["base"] > 0


def test_explore_api_validates_and_reports_errors():
    def boom(role, tier):
        raise TimeoutError
    api = ExploreApi(lambda r, t: {"role": r, "champs": []}, lambda: "master_plus", lambda: "middle")
    assert api.start()["role"] == "middle"
    assert [x["id"] for x in api.start()["tiers"]] == ["master_plus"]
    assert ExploreApi(boom, lambda: "master_plus").start()["role"] == "support"
    assert "error" in api.get("mid", "master_plus")
    assert "error" in api.get("top", "grandmaster_plus")  # dropped tier
    assert api.get("top", "master_plus")["tier_label"] == "Master+"
    assert "TimeoutError" in ExploreApi(boom, lambda: "master_plus").get("top", "master_plus")["error"]


def test_no_matchup_data_is_not_safe():
    opp = {10: 50.0, 11: 50.0}
    pool = {1: champ(1, 52, {10: -3.0, 11: -1.0}), 2: champ(2, 52, {10: -4.0, 11: 0.0}),
            3: champ(3, 52, {}), 10: champ(10, 50, {}), 11: champ(11, 50, {})}
    rep = {c["id"]: c for c in role_report(pool, opp, "support", W, k=500)}
    nodata = rep[3]
    assert nodata["coverage"] == 0
    assert nodata["tail"] < 0  # takes the typical counter risk, not "even"
    assert min(rep[1]["tail"], rep[2]["tail"]) <= nodata["tail"] <= max(rep[1]["tail"], rep[2]["tail"])
