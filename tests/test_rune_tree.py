"""The build view's rune page carries both trees row by row (runesReforged.json order), every rune
marked chosen or not, and degrades to the chosen runes only when the tree isn't known."""
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent.parent))

from data.builds import Build, Option, RunePage
from data.ddragon import SHARD_ROWS, Assets
from draft import views
from draft.engine import BuildEvaluation


def _tree(sid, name, rows):
    return {"id": sid, "key": name, "name": name, "icon": f"{name}.png",
            "slots": [{"runes": [{"id": r, "name": f"r{r}", "icon": f"{r}.png"} for r in row]} for row in rows]}


# runesReforged.json shape: a 4-keystone tree and a 3-keystone one
RUNES = [
    _tree(8000, "Precision", [[8005, 8008, 8021, 8010], [9101, 9111, 8009], [9104, 9105, 9103], [8014, 8017, 8299]]),
    _tree(8400, "Resolve", [[8437, 8439, 8465], [8446, 8463, 8401], [8429, 8444, 8473], [8451, 8453, 8242]]),
]


class Champs:
    def name(self, c): return f"c{c}"
    def icon_url(self, c): return ""


def build(assets):
    page = RunePage([8010, 9111, 9104, 8299], [8444, 8453], [5005, 5008, 5011], 52.0, 1000)
    runes = {"8010": Option("8010", 52.0, 60.0, 600), "8008": Option("8008", 50.5, 30.0, 300),
             "5008f": Option("5008f", 51.0, 80.0, 800), "5010f": Option("5010f", 49.0, 5.0, 50)}
    b = Build(222, "bottom", 51.0, 1000, {"pick": page}, runes, {})
    return views.build(BuildEvaluation(222, "bottom", b, {}), Champs(), assets)


def test_full_trees_in_client_order():
    out = build(Assets("1", {}, RUNES, {}))
    t = out["pages"][0]["tree"]
    prim, sec, shards = t["primary"], t["secondary"], t["shards"]
    assert [[r["id"] for r in row] for row in prim] == [[8005, 8008, 8021, 8010], [9101, 9111, 8009], [9104, 9105, 9103], [8014, 8017, 8299]]
    assert [[r["id"] for r in row if r["chosen"]] for row in prim] == [[8010], [9111], [9104], [8299]]
    # secondary: the minor rows only, the two picks lit
    assert [[r["id"] for r in row] for row in sec] == [[8446, 8463, 8401], [8429, 8444, 8473], [8451, 8453, 8242]]
    assert [r["id"] for row in sec for r in row if r["chosen"]] == [8444, 8453]
    # every rune has a name and icon; stats only where the source has them
    key = {r["id"]: r for r in prim[0]}
    assert key[8010]["name"] == "r8010" and key[8010]["icon"].endswith("8010.png")
    assert key[8008]["pr"] == 30.0 and key[8008]["wr"] == 50.5 and key[8008]["games"] == 300 and not key[8008]["chosen"]
    assert "wr" not in key[8005]
    # shards: the three fixed rows from data/ddragon.py, the flex row's stats keyed "<id>f"
    assert [[r["id"] for r in row] for row in shards] == SHARD_ROWS
    assert [[r["chosen"] for r in row] for row in shards] == [[False, True, False], [True, False, False], [True, False, False]]
    assert shards[1][0]["pr"] == 80.0 and shards[1][1]["pr"] == 5.0
    assert shards[0][0]["name"] == "Adaptive Force" and "pr" not in shards[0][0]  # offense row isn't the flex key
    # the chosen-only fields stay as they were
    p = out["pages"][0]
    assert [r["id"] for r in p["primary"]] == [8010, 9111, 9104, 8299] and len(p["secondary"]) == 2 and len(p["shards"]) == 3


def test_no_tree_data_degrades_to_chosen_runes():
    out = build(Assets("1", {}, [], {}))  # runesReforged.json unavailable
    p = out["pages"][0]
    assert p["tree"]["primary"] == [] and p["tree"]["secondary"] == []
    assert [r["id"] for r in p["primary"]] == [8010, 9111, 9104, 8299]
    assert [r["name"] for r in p["primary"]] == ["#8010", "#9111", "#9104", "#8299"]
    assert len(p["tree"]["shards"]) == 3  # shard rows are fixed game data


def test_rune_outside_known_tree_drops_that_tree():
    a = Assets("1", {}, RUNES, {})
    a.trees[8400] = a.trees[8400][:2]  # a stale tree missing the row of a picked rune
    t = build(a)["pages"][0]["tree"]
    assert t["primary"] and t["secondary"] == []


def test_unknown_shard_shows_alone():
    a = Assets("1", {}, RUNES, {})
    b = build(a)
    assert b["pages"][0]["tree"]["shards"][0][1]["id"] == 5005
    page = RunePage([8010, 9111, 9104, 8299], [8444, 8453], [5002, 5008, 5011], 52.0, 1000)
    bb = Build(222, "bottom", 51.0, 1000, {"pick": page}, {}, {})
    rows = views.build(BuildEvaluation(222, "bottom", bb, {}), Champs(), a)["pages"][0]["tree"]["shards"]
    assert [[r["id"] for r in row] for row in rows][0] == [5002] and rows[0][0]["chosen"]
