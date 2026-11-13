import copy
import json
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent.parent))

from config import Config
from draft.controller import Controller
from data.ddragon import Assets
from data.store import Store
from draft.engine import BuildEvaluation, Evaluation
from draft.scoring import Recommendation
from draft.state import Event, Session, Stage, in_ban_phase

FIXTURE = json.loads((Path(__file__).parent / "fixtures" / "session.json").read_text())


def session(ban_in_progress=False, my_pick=0):
    s = copy.deepcopy(FIXTURE)
    s["actions"][0][2]["isInProgress"] = ban_in_progress
    if my_pick:
        s["actions"][1][2].update(completed=True, championId=my_pick)  # cell 2 is the local player
    return s


def test_stage_follows_gameflow_before_draft():
    s = Session()
    assert s.stage == Stage.IDLE
    for phase, stage in [("Lobby", Stage.LOBBY), ("Matchmaking", Stage.QUEUE), ("ReadyCheck", Stage.READY)]:
        assert s.on_phase(phase) == [Event.PHASE]
        assert s.stage == stage
    assert s.on_phase("ReadyCheck") == []  # repeats are not events


def test_draft_ban_pick_lock_and_end():
    s = Session()
    s.on_phase("ChampSelect")
    events = s.on_session(session(ban_in_progress=True))
    assert events == [Event.DRAFT_STARTED, Event.DRAFT_CHANGED]
    assert s.stage == Stage.BAN
    assert s.on_session(session()) == [Event.DRAFT_CHANGED]
    assert s.stage == Stage.PICK
    assert s.on_session(session(my_pick=412)) == [Event.LOCKED_IN, Event.DRAFT_CHANGED]
    assert s.stage == Stage.LOCKED
    assert 412 not in s.frozen.picked  # your own champion stays rankable in the frozen snapshot
    assert s.on_session(session(my_pick=412)) == [Event.DRAFT_CHANGED]  # locked in only once
    events = s.on_phase("InProgress")
    assert events == [Event.PHASE, Event.DRAFT_ENDED]
    assert s.stage == Stage.INGAME and s.draft is None and s.frozen is None


def test_enemy_lane_overrides_reset_each_draft():
    s = Session()
    s.on_session(session())
    s.set_enemy_lane(111, "bottom", ("top", "jungle", "middle", "bottom", "support"))
    s.set_enemy_lane(103, "bottom", ("top", "jungle", "middle", "bottom", "support"))
    assert s.enemy_lanes == {103: "bottom"}  # one champion per lane
    s.on_session(None)
    s.on_session(session())
    assert s.enemy_lanes == {}


def test_ban_phase_detection():
    assert in_ban_phase(session(ban_in_progress=True))
    assert not in_ban_phase(session())


def test_planning_counts_as_ban_phase():
    s = session()
    assert in_ban_phase(s | {"timer": {"phase": "PLANNING"}})
    assert not in_ban_phase(s | {"timer": {"phase": "BAN_PICK"}})
    fresh = copy.deepcopy(s)  # no timer reading: nothing banned or picked yet
    for group in fresh["actions"]:
        for a in group:
            a.update(completed=False, isInProgress=False)
    assert in_ban_phase(fresh)
    ses = Session()
    ses.on_phase("ChampSelect")
    ses.on_session(fresh)
    assert ses.stage == Stage.BAN


class StubChamps:
    def name(self, cid): return f"c{cid}"
    def icon_url(self, cid): return ""
    def team_damage(self, cids): return None


class StubRec:
    tier = "master_plus"
    champs = StubChamps()
    assets = Assets("1", {}, [], {})

    @staticmethod
    def role_for(draft):
        return draft.my_position or None

    def evaluate(self, draft, pickable=None, enemy_lanes=None):
        return Evaluation(draft, draft.my_position, [Recommendation(412, 2.5, 1.0, [])], {})

    def odds_inputs(self, ev):
        return {}, {}

    def build(self, champion_id, role, enemies):
        return BuildEvaluation(champion_id, role, None, enemies)


def test_controller_records_draft(tmp_path):
    pushed = []
    store = Store(tmp_path / "s.sqlite")
    ctl = Controller(Config(), pushed.append, store)
    ctl.rec = StubRec()
    ctl.on_phase("ChampSelect")
    ctl.on_session(session())
    ctl.on_session(session())  # identical snapshot is skipped
    assert pushed[-1]["view"] == "client" and pushed[-1]["picks"] and pushed[-1]["role"] == "support"
    ctl.on_session(session(my_pick=412))
    ctl.on_phase("InProgress")
    assert pushed[-1]["view"] == "client" and pushed[-1]["picks"] is None and pushed[-1]["build"] is None
    (draft,) = store.drafts()
    assert draft["role"] == "support" and draft["my_champ"] == 412 and draft["ended"]
    assert len(store.draft_snapshots(draft["id"])) == 2
    assert store.draft_recs(draft["id"], "locked")["recs"][0]["id"] == 412
    assert store.draft_recs(draft["id"], "final") is not None


def test_controller_status_messages():
    pushed = []
    ctl = Controller(Config(), pushed.append)
    ctl.on_phase("Matchmaking")
    s = pushed[-1]
    assert s["view"] == "client" and s["picks"] is None and s["bans"] is None and s["build"] is None
    assert "queue" in s["message"].lower()


class StubBanRec(StubRec):
    def bans(self, draft, pickable=None):
        from draft.engine import BanEvaluation
        from draft.scoring import Threat
        return BanEvaluation(draft, draft.my_position, [412], [Threat(555, "bottom", 0.3, 10.0, 1.0, [])])


def test_ban_phase_shows_bans_and_picks_together():
    pushed = []
    ctl = Controller(Config(), pushed.append)
    ctl.rec = StubBanRec()
    ctl.on_phase("ChampSelect")
    ctl.on_session(session(ban_in_progress=True))
    s = pushed[-1]
    assert s["view"] == "client" and s["bans"]["threats"][0]["id"] == 555 and s["picks"]
    ctl.on_session(session())  # ban phase over: picks only
    assert pushed[-1]["bans"] is None and pushed[-1]["picks"]


def test_preload_runs_once_per_role(monkeypatch):
    ctl = Controller(Config(), lambda s: None)
    loaded = []

    class Rec(StubRec):
        def has_pool(self, role, tier=None): return False
        def pool(self, role, progress=None, tier=None): loaded.append(role); progress("Loading x 1/1...")
    ctl.rec = Rec()
    ctl.lobby_roles = lambda: ["support", "bottom"]
    started = []
    monkeypatch.setattr("draft.controller.threading.Thread", lambda target, args, **kw: started.append((target, args)) or type("T", (), {"start": lambda self: target(*args)})())
    ctl.on_phase("Lobby")
    ctl.on_phase("Matchmaking")  # same roles: nothing new to load
    assert loaded == ["support", "bottom"] and len(started) == 1
    assert ctl.preload == "Stats ready: support, bottom"


def no_picks_yet(my_turn=False):
    s = session()
    for a in s["actions"][1]:
        a.update(completed=False, championId=0, isInProgress=False)
    s["actions"][1][2]["isInProgress"] = my_turn  # cell 2 is you
    for p in s["myTeam"] + s["theirTeam"]:
        p["championId"] = 0
    return s


def test_picks_show_as_soon_as_bans_are_done():
    pushed = []
    ctl = Controller(Config(), pushed.append)
    ctl.rec = StubRec()
    ctl.on_phase("ChampSelect")
    ctl.on_session(no_picks_yet())  # bans done, nobody has picked: blind-pick time
    assert pushed[-1]["picks"]
    ctl.on_session(no_picks_yet(my_turn=True))
    assert pushed[-1]["picks"]
    ctl.on_session(session())  # someone has locked in
    assert pushed[-1]["picks"]


def test_lock_in_shows_build_with_recommendations_from_before_you_picked():
    pushed = []
    ctl = Controller(Config(), pushed.append)
    ctl.rec = StubRec()
    ctl.on_phase("ChampSelect")
    ctl.on_session(session())
    ctl.on_session(session(my_pick=412))
    s = pushed[-1]
    assert s["view"] == "client" and s["build"]["champ"]["name"] == "c412"
    assert s["picks"]["recs"][0]["name"] == "c412"  # your champion is still ranked in the frozen list
    assert s["picks"]["locked"]["name"] == "c412"


def test_preload_when_idle_uses_your_usual_roles(tmp_path, monkeypatch):
    store = Store(tmp_path / "s.sqlite")
    for gid, role in enumerate(["support", "support", "bottom", "bottom", "top", ""]):
        store.add_game(gid, 1000.0 + gid, 420, 412, role, True, 1800, {})
    assert store.top_roles() == ["support", "bottom"]
    ctl = Controller(Config(), lambda s: None, store)
    loaded = []

    class Rec(StubRec):
        def has_pool(self, role, tier=None): return False
        def pool(self, role, progress=None, tier=None): loaded.append(role)
    ctl.rec = Rec()
    ctl.lobby_roles = lambda: []  # not in a lobby
    monkeypatch.setattr("draft.controller.threading.Thread", lambda target, args, **kw: type("T", (), {"start": lambda self: target(*args)})())
    ctl.on_phase("None")
    assert loaded == ["support", "bottom"]
    ctl.on_phase("Lobby")  # lobby roles empty, nothing new
    assert loaded == ["support", "bottom"]
