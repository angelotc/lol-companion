use serde_json::Value;
use std::collections::{HashMap, HashSet};

use super::lcu::{DraftState, parse_session};
use super::scoring::LANES;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Stage {
    Idle,
    Lobby,
    Queue,
    Ready,
    Ban,
    Pick,
    Locked,
    Loading,
    Ingame,
    Postgame,
}

impl Stage {
    pub fn as_str(self) -> &'static str {
        match self {
            Stage::Idle => "idle",
            Stage::Lobby => "lobby",
            Stage::Queue => "queue",
            Stage::Ready => "ready",
            Stage::Ban => "ban",
            Stage::Pick => "pick",
            Stage::Locked => "locked",
            Stage::Loading => "loading",
            Stage::Ingame => "ingame",
            Stage::Postgame => "postgame",
        }
    }

    pub fn is_draft(self) -> bool {
        matches!(self, Stage::Ban | Stage::Pick | Stage::LOCKED_ALIAS)
    }

    pub fn is_pre_draft(self) -> bool {
        matches!(self, Stage::Lobby | Stage::Queue | Stage::Ready)
    }
}

impl Stage {
    const LOCKED_ALIAS: Stage = Stage::Locked;
}

pub fn stage_from_gameflow(phase: &str) -> Stage {
    match phase {
        "Lobby" => Stage::Lobby,
        "Matchmaking" | "CheckedIntoTournament" => Stage::Queue,
        "ReadyCheck" => Stage::Ready,
        "GameStart" => Stage::Loading,
        "InProgress" | "Reconnect" => Stage::Ingame,
        "WaitingForStats" | "PreEndOfGame" | "EndOfGame" => Stage::Postgame,
        _ => Stage::Idle,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Event {
    Phase,
    DraftStarted,
    DraftChanged,
    LockedIn,
    DraftEnded,
}

pub fn in_ban_phase(session: &Value) -> bool {
    if session
        .get("timer")
        .and_then(|t| t.get("phase"))
        .and_then(|v| v.as_str())
        == Some("PLANNING")
    {
        return true;
    }
    let mut actions = Vec::new();
    if let Some(arr) = session.get("actions").and_then(|v| v.as_array()) {
        for group in arr {
            if let Some(inner) = group.as_array() {
                for a in inner {
                    if a.is_object() {
                        actions.push(a);
                    }
                }
            } else if group.is_object() {
                actions.push(group);
            }
        }
    }
    let bans: Vec<&&Value> = actions
        .iter()
        .filter(|a| a.get("type").and_then(|v| v.as_str()) == Some("ban"))
        .collect();
    if bans.iter().any(|a| {
        a.get("isInProgress").and_then(|v| v.as_bool()).unwrap_or(false)
            && !a.get("completed").and_then(|v| v.as_bool()).unwrap_or(false)
    }) {
        return true;
    }
    let started = actions.iter().any(|a| {
        a.get("completed").and_then(|v| v.as_bool()).unwrap_or(false)
            || a.get("isInProgress").and_then(|v| v.as_bool()).unwrap_or(false)
    });
    !bans.is_empty() && !started
}

#[derive(Debug, Clone, Default)]
pub struct Session {
    pub phase: Option<String>,
    pub draft: Option<DraftState>,
    pub pickable: Option<HashSet<i32>>,
    pub banning: bool,
    pub frozen: Option<DraftState>,
    pub enemy_lanes: HashMap<i32, String>,
}

impl Session {
    pub fn stage(&self) -> Stage {
        let phase_str = self.phase.as_deref();
        if let Some(draft) = &self.draft
            && matches!(phase_str, None | Some("ChampSelect"))
        {
            if draft.my_locked != 0 {
                return Stage::Locked;
            }
            return if self.banning { Stage::Ban } else { Stage::Pick };
        }
        if phase_str == Some("ChampSelect") {
            return Stage::Pick;
        }
        stage_from_gameflow(phase_str.unwrap_or("None"))
    }

    pub fn in_draft(&self) -> bool {
        self.draft.is_some()
    }

    pub fn on_phase(&mut self, phase: &str) -> Vec<Event> {
        let mut events = Vec::new();
        if self.phase.as_deref() != Some(phase) {
            self.phase = Some(phase.to_string());
            events.push(Event::Phase);
        }
        if phase != "ChampSelect" && self.draft.is_some() {
            events.extend(self.on_session(None, None));
        }
        events
    }

    pub fn on_session(&mut self, session: Option<&Value>, pickable: Option<HashSet<i32>>) -> Vec<Event> {
        let Some(session) = session else {
            if self.draft.is_none() {
                return Vec::new();
            }
            self.draft = None;
            self.frozen = None;
            self.pickable = None;
            self.banning = false;
            self.enemy_lanes.clear();
            return vec![Event::DraftEnded];
        };

        let mut events = Vec::new();
        if self.draft.is_none() {
            self.frozen = None;
            self.enemy_lanes.clear();
            events.push(Event::DraftStarted);
        }
        let draft = parse_session(session);
        self.banning = in_ban_phase(session);
        if let Some(p) = pickable
            && !p.is_empty()
        {
            self.pickable = Some(p);
        }
        if draft.my_locked != 0 && self.frozen.is_none() {
            let mut frozen = draft.clone();
            frozen.picked.remove(&draft.my_locked);
            self.frozen = Some(frozen);
            events.push(Event::LockedIn);
        } else if draft.my_locked == 0 {
            self.frozen = None;
        }
        self.draft = Some(draft);
        events.push(Event::DraftChanged);
        events
    }

    pub fn set_enemy_lane(&mut self, champion_id: i32, lane: &str) {
        self.enemy_lanes
            .retain(|&c, l| c != champion_id && l != lane);
        if LANES.contains(&lane) {
            self.enemy_lanes.insert(champion_id, lane.to_string());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    fn fixture() -> Value {
        let p = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/session.json");
        serde_json::from_str(&std::fs::read_to_string(p).unwrap()).unwrap()
    }

    fn make_session(ban_in_progress: bool, my_pick: i32) -> Value {
        let mut s = fixture();
        s["actions"][0][2]["isInProgress"] = Value::Bool(ban_in_progress);
        if my_pick != 0 {
            s["actions"][1][2]["completed"] = Value::Bool(true);
            s["actions"][1][2]["championId"] = Value::Number(my_pick.into());
        }
        s
    }

    #[test]
    fn test_stage_follows_gameflow_before_draft() {
        let mut s = Session::default();
        assert_eq!(s.stage(), Stage::Idle);
        for (phase, stage) in [
            ("Lobby", Stage::Lobby),
            ("Matchmaking", Stage::Queue),
            ("ReadyCheck", Stage::Ready),
        ] {
            assert_eq!(s.on_phase(phase), vec![Event::Phase]);
            assert_eq!(s.stage(), stage);
        }
        assert!(s.on_phase("ReadyCheck").is_empty());
    }

    #[test]
    fn test_draft_ban_pick_lock_and_end() {
        let mut s = Session::default();
        s.on_phase("ChampSelect");
        assert_eq!(
            s.on_session(Some(&make_session(true, 0)), None),
            vec![Event::DraftStarted, Event::DraftChanged]
        );
        assert_eq!(s.stage(), Stage::Ban);
        assert_eq!(
            s.on_session(Some(&make_session(false, 0)), None),
            vec![Event::DraftChanged]
        );
        assert_eq!(s.stage(), Stage::Pick);
        assert_eq!(
            s.on_session(Some(&make_session(false, 412)), None),
            vec![Event::LockedIn, Event::DraftChanged]
        );
        assert_eq!(s.stage(), Stage::Locked);
        assert!(!s.frozen.as_ref().unwrap().picked.contains(&412));
        assert_eq!(
            s.on_session(Some(&make_session(false, 412)), None),
            vec![Event::DraftChanged]
        );
        assert_eq!(
            s.on_phase("InProgress"),
            vec![Event::Phase, Event::DraftEnded]
        );
        assert_eq!(s.stage(), Stage::Ingame);
        assert!(s.draft.is_none());
        assert!(s.frozen.is_none());
    }

    #[test]
    fn test_enemy_lane_overrides_reset_each_draft() {
        let mut s = Session::default();
        let ses = make_session(false, 0);
        s.on_session(Some(&ses), None);
        s.set_enemy_lane(111, "bottom");
        s.set_enemy_lane(103, "bottom");
        assert_eq!(s.enemy_lanes, HashMap::from([(103, "bottom".into())]));
        s.on_session(None, None);
        s.on_session(Some(&ses), None);
        assert!(s.enemy_lanes.is_empty());
    }
}
