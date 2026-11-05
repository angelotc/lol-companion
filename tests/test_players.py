import json
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent.parent))

import httpx

from data.players import (
    FixtureClient, PlayerRef, PlayerStats, game_refs, load, load_all, parse_mastery, parse_ranked,
    parse_teams, refs_from_live, summarize_history, tags,
)
from draft import views

FIXTURES = Path(__file__).parent / "fixtures"
PLAYERS = FIXTURES / "players"
LIVE = json.loads((FIXTURES / "live.json").read_text(encoding="utf-8"))


class Champs:
    def name(self, c): return f"c{c}"
    def icon_url(self, c): return ""


def ref(**kw) -> PlayerRef:
    base = dict(puuid="p", name="Name", tag="NA1", champion_id=1, team=100, position="top")
    base.update(kw)
    return PlayerRef(**base)


# --- parse_teams ---------------------------------------------------------

def test_parse_teams_maps_teams_lanes_me_and_picks():
    session = {"gameData": {
        "teamOne": [
            {"puuid": "a", "gameName": "Ann", "tagLine": "NA1", "championId": 0, "selectedPosition": "UTILITY"},
            {"puuid": "b", "riotIdGameName": "Bob", "riotIdTagLine": "X", "championId": 5, "selectedPosition": "TOP"},
            {"gameName": "NoPuuid", "championId": 9, "selectedPosition": "JUNGLE"},  # skipped
        ],
        "teamTwo": [
            {"puuid": "c", "summonerName": "Cy", "championId": 0, "selectedPosition": "BOTTOM"},
        ],
        "playerChampionSelections": [{"puuid": "a", "championId": 99}, {"puuid": "c", "championId": 77}],
    }}
    refs = parse_teams(session, "b")
    assert len(refs) == 3  # the puuid-less entry is skipped
    assert [r.team for r in refs] == [100, 100, 200]
    a, b, c = refs[0], refs[1], refs[2]
    assert a.position == "support" and a.name == "Ann" and a.tag == "NA1"
    assert a.champion_id == 99 and c.champion_id == 77  # champion filled from picks
    assert b.name == "Bob" and b.tag == "X" and b.champion_id == 5
    assert b.is_me and not a.is_me and not c.is_me
    assert c.position == "bottom" and c.name == "Cy"


def test_parse_teams_empty_session():
    assert parse_teams({}) == []
    assert parse_teams(None) == []


# --- parse_ranked --------------------------------------------------------

def test_parse_ranked_solo_normalises_case():
    data = {"queueMap": {"RANKED_SOLO_5x5": {"tier": "gold", "division": "ii", "leaguePoints": 5,
                                             "wins": 3, "losses": 2}}}
    assert parse_ranked(data) == {"tier": "GOLD", "division": "II", "lp": 5, "wins": 3, "losses": 2, "queue": "solo"}


def test_parse_ranked_flex_fallback_when_solo_unranked():
    data = {"queueMap": {
        "RANKED_SOLO_5x5": {"tier": "NONE", "division": "NA", "leaguePoints": 0, "wins": 0, "losses": 0},
        "RANKED_FLEX_SR": {"tier": "BRONZE", "division": "I", "leaguePoints": 70, "wins": 6, "losses": 4},
    }}
    out = parse_ranked(data)
    assert out["queue"] == "flex" and out["tier"] == "BRONZE" and out["division"] == "I" and out["lp"] == 70


def test_parse_ranked_unranked_is_empty():
    assert parse_ranked({"queueMap": {}}) == {}
    assert parse_ranked({"queueMap": {"RANKED_SOLO_5x5": {"tier": "NONE"}}}) == {}
    assert parse_ranked(None) == {}


def test_parse_ranked_apex_has_blank_division():
    data = {"queueMap": {"RANKED_SOLO_5x5": {"tier": "MASTER", "division": "I", "leaguePoints": 150}}}
    out = parse_ranked(data)
    assert out["tier"] == "MASTER" and out["division"] == "" and out["lp"] == 150


# --- summarize_history ---------------------------------------------------

def _game(qid, created, champ, win, k=0, d=0, a=0, extra_parts=None):
    parts = [{"championId": champ, "stats": {"win": win, "kills": k, "deaths": d, "assists": a}}]
    if extra_parts:
        parts = extra_parts
    return {"queueId": qid, "gameCreation": created, "participants": parts}


def test_summarize_history_filters_queue_orders_and_sums():
    data = {"games": {"games": [
        _game(450, 900, 1, True, 30, 0, 0),          # ARAM: ignored
        _game(420, 100, 1, False, 2, 4, 6),          # oldest
        _game(420, 200, 1, True, 5, 1, 3),           # newest on champ 1
        _game(420, 150, 2, True, 9, 9, 9),           # other champion
        _game(420, 50, 1, True, 1, 1, 1, extra_parts=[  # two participants: skipped
            {"championId": 1, "stats": {"win": True}}, {"championId": 3, "stats": {"win": False}}]),
    ]}}
    out = summarize_history(data, 1)
    assert out["games"] == 3
    assert out["recent"] == [True, True, False]  # newest first
    assert out["champ_games"] == 2 and out["champ_wins"] == 1
    assert (out["kills"], out["deaths"], out["assists"]) == (7, 5, 9)


def test_summarize_history_empty():
    out = summarize_history({}, 1)
    assert out["games"] == 0 and out["recent"] == [] and out["champ_games"] == 0


# --- parse_mastery -------------------------------------------------------

def test_parse_mastery():
    assert parse_mastery(None, 1) is None
    assert parse_mastery({"championId": 1}, 1) is None  # not a list
    data = [{"championId": 1, "championPoints": 500}, {"championId": 2, "championPoints": 7}]
    assert parse_mastery(data, 1) == 500
    assert parse_mastery(data, 3) == 0  # full list, champion absent: never played


# --- tags ----------------------------------------------------------------

def test_tags_win_streak():
    s = PlayerStats(ref(), recent=[True, True, True, False], mastery=50_000)
    assert tags(s) == ["3 wins in a row"]


def test_tags_loss_streak():
    s = PlayerStats(ref(), recent=[False] * 4 + [True], mastery=50_000)
    assert tags(s) == ["4 losses in a row"]


def test_tags_no_streak_below_three():
    s = PlayerStats(ref(), recent=[True, True, False, True], mastery=50_000)
    assert tags(s) == []


def test_tags_first_time_needs_known_low_mastery():
    assert tags(PlayerStats(ref(), champ_games=0, mastery=0)) == ["First time"]
    assert tags(PlayerStats(ref(), champ_games=0, mastery=None)) == []  # unknown mastery: no claim
    assert tags(PlayerStats(ref(), champ_games=0, mastery=10_000)) == []


def test_tags_main_on_recent_champion_games():
    assert tags(PlayerStats(ref(), champ_games=5, mastery=20_000)) == ["Main"]


def test_tags_empty_recent_has_no_streak():
    assert tags(PlayerStats(ref(), recent=[], champ_games=0, mastery=50_000)) == []


# --- PlayerStats.kda -----------------------------------------------------

def test_kda_none_without_champion_games():
    assert PlayerStats(ref()).kda is None


def test_kda_zero_deaths_does_not_divide_by_zero():
    s = PlayerStats(ref(), champ_games=2, kills=3, deaths=0, assists=2)
    assert s.kda == 5.0


def test_kda_ratio():
    s = PlayerStats(ref(), champ_games=2, kills=3, deaths=2, assists=1)
    assert s.kda == 2.0


# --- refs_from_live ------------------------------------------------------

def test_refs_from_live_order_and_me():
    champs = {"Taric": 44, "Dr. Mundo": 36}
    refs = refs_from_live(LIVE, lambda n: champs.get(n))
    assert len(refs) == 10
    assert [r.team for r in refs] == [100] * 5 + [200] * 5
    me = [r for r in refs if r.is_me]
    assert len(me) == 1 and me[0].name == "Me" and me[0].tag == "NA1"
    assert me[0].champion_id == 44 and me[0].position == "support"
    assert refs[0].champion_id == 36 and refs[0].position == "top"
    assert all(r.puuid == "" for r in refs)


# --- end to end with FixtureClient --------------------------------------

def _fixture_stats():
    client = FixtureClient(PLAYERS)
    game_id, refs = game_refs(client)
    assert game_id == 5123456789
    return load_all(client, refs)


def _by_name(stats):
    return {s.ref.name: s for s in stats}


def test_fixture_end_to_end_facts():
    stats = _fixture_stats()
    assert len(stats) == 10
    by = _by_name(stats)

    me = [s for s in stats if s.ref.is_me]
    assert len(me) == 1 and me[0].ref.name == "Me"
    assert me[0].ref.team == 100 and me[0].ref.position == "bottom"
    assert me[0].tier == "GOLD" and me[0].division == "I" and me[0].lp == 67 and me[0].queue == "solo"

    assert by["Mundo Goes"].tags == ["First time"]
    assert by["Hook City"].tier == "BRONZE" and by["Hook City"].queue == "flex"
    assert by["Orbs"].tier == "MASTER" and by["Orbs"].division == ""
    assert "Main" in by["Foxfire"].tags and "3 wins in a row" in by["Foxfire"].tags
    assert "4 losses in a row" in by["Moon"].tags

    windy = by["Windy"]
    assert windy.error is True and windy.champ_games == 0
    assert windy.tier == "PLATINUM" and windy.division == "II"


# --- draft.views.players -------------------------------------------------

def test_views_players_payload_shape():
    stats = _fixture_stats()
    out = views.players(views.header("bottom", "master_plus", 14), stats, Champs())
    assert out["view"] == "players" and out["status"] == "active"
    ally, enemy = out["teams"]
    assert ally["side"] == "ally" and ally["label"] == "Your team"
    assert enemy["side"] == "enemy" and enemy["label"] == "Enemy team"
    assert len(ally["players"]) == 5 and len(enemy["players"]) == 5

    order = ["top", "jungle", "middle", "bottom", "support"]
    assert [p["lane"] for p in ally["players"]] == order
    assert [p["lane"] for p in enemy["players"]] == order

    me = next(p for p in ally["players"] if p["me"])
    assert me["name"] == "Me"
    assert me["rank"] == "G1 67 LP" and me["rank_full"] == "Gold I 67 LP"
    assert isinstance(me["wr"], int)

    for p in ally["players"] + enemy["players"]:
        assert len(p["recent"]) <= 10
        assert set(p["recent"]) <= {"W", "L"}


def test_views_players_empty_stats_render():
    bare = PlayerStats(PlayerRef(puuid="", name="Lone", tag="", champion_id=0, team=200, position=""))
    out = views.players(views.header(None, "master_plus", 14), [bare], Champs())
    enemy = next(t for t in out["teams"] if t["side"] == "enemy")
    p = enemy["players"][0]
    assert p["rank"] == "" and p["rank_full"] == "Unranked"
    assert p["wr"] is None and p["kda"] is None
    assert p["champ"] is None and p["recent"] == ""


# --- load() failure paths ------------------------------------------------

class NotFoundClient:
    def get(self, path, timeout=None):
        return httpx.Response(404)


class RaisingClient:
    def get(self, path, timeout=None):
        raise httpx.ConnectError("client gone")


def test_load_404_everywhere_sets_error_without_raising():
    s = load(NotFoundClient(), ref(puuid="", name="Ghost", tag="NA1"))
    assert s.error is True and s.tier == "" and s.champ_games == 0


def test_load_404_with_puuid_sets_error_without_raising():
    s = load(NotFoundClient(), ref(puuid="pz"))
    assert s.error is True and s.tier == "" and s.mastery is None


def test_load_raising_client_sets_error_without_raising():
    s = load(RaisingClient(), ref(puuid="", name="Ghost", tag="NA1"))
    assert s.error is True
    s = load(RaisingClient(), ref(puuid="pz"))
    assert s.error is True


def test_parse_ranked_losses_unknown_below_master():
    # the client reports 0 losses for other players below Master
    q = lambda tier, w, l: {"queueMap": {"RANKED_SOLO_5x5": {"tier": tier, "division": "II", "leaguePoints": 9, "wins": w, "losses": l}}}
    assert parse_ranked(q("DIAMOND", 289, 0))["losses"] is None
    assert parse_ranked(q("DIAMOND", 30, 25))["losses"] == 25
    assert parse_ranked(q("MASTER", 50, 0))["losses"] == 0


def test_view_without_losses_shows_wins_only():
    s = PlayerStats(PlayerRef("p", "A", "1", 1, 100, "top"), tier="DIAMOND", division="II", wins=289, losses=None)
    row = views.players({}, [s], Champs())["teams"][0]["players"][0]
    assert row["wr"] is None and row["wins"] == 289 and row["season_games"] == 0


def test_history_counts_rift_matched_games_by_map():
    game = lambda q, m, mode, champ=7: {"queueId": q, "mapId": m, "gameMode": mode, "gameType": "MATCHED_GAME",
                                        "gameCreation": 1, "participants": [{"championId": champ, "stats": {"win": True}}]}
    data = {"games": {"games": [game(710, 11, "CLASSIC"), game(2400, 12, "KIWI"), game(450, 12, "ARAM"), game(1750, 30, "CHERRY")]}}
    out = summarize_history(data, 7)
    assert out["games"] == 1 and out["champ_games"] == 1


def test_opgg_parse_champions():
    from data.opgg import parse_champions
    # the page embeds its data as an escaped JSON string; entries nest arrays (match_up_stats)
    entries = [{"id": 0, "play": 50, "win": 25}, {"id": 112, "champion_id": 112, "play": 49, "win": 27,
               "match_up_stats": [[1, "x]"]], "name": "Viktor"}, {"champion_id": 134, "play": 3, "win": 1}]
    html = '<script>self.__next_f.push([1,"{\\"my_champion_stats\\":' + json.dumps(entries).replace('"', '\\"') + '}"])</script>'
    assert parse_champions(html) == {112: [49, 27], 134: [3, 1]}
    assert parse_champions("<html>nothing</html>") is None


def test_load_with_season_from_opgg():
    class Season:
        def __init__(self, data): self.data = data
        def season(self, name, tag, region): return self.data
    c = FixtureClient(FIXTURES / "players")
    _, refs = game_refs(c)
    me = next(r for r in refs if r.is_me)
    s = load(c, me, Season({me.champion_id: [40, 22]}), "na")
    assert (s.season_games, s.season_wins) == (40, 22) and "Main" in s.tags
    row = views.players({}, [s], Champs())["teams"][0]["players"][0]
    assert row["champ_season_games"] == 40 and row["champ_season_wr"] == 55
    s = load(c, me, Season({}), "na")  # op.gg knows the player, no games on this champion
    assert (s.season_games, s.season_wins) == (0, 0)
    s = load(c, me, Season(None), "na")  # op.gg couldn't say
    assert s.season_games is None
