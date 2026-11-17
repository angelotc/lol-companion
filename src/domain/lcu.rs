use serde_json::Value;
use std::collections::HashSet;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LcuCreds {
    pub port: u16,
    pub password: String,
    pub protocol: String,
    pub pid: u32,
    pub name: String,
}

pub fn parse_lockfile_text(text: &str) -> Option<LcuCreds> {
    let parts: Vec<&str> = text.trim().split(':').collect();
    if parts.len() < 5 {
        return None;
    }
    let pid = parts[1].parse::<u32>().ok()?;
    let port = parts[2].parse::<u16>().ok()?;
    Some(LcuCreds {
        name: parts[0].to_string(),
        pid,
        port,
        password: parts[3].to_string(),
        protocol: parts[4].to_string(),
    })
}

pub fn normalize_position(pos: &str) -> String {
    let lower = pos.to_ascii_lowercase();
    if lower == "utility" {
        "support".to_string()
    } else {
        lower
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Slot {
    pub champion_id: i32,
    pub position: String,
    pub tentative: bool,
    pub is_me: bool,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DraftState {
    pub my_cell_id: i32,
    pub my_position: String,
    pub allies: Vec<Slot>,
    pub enemies: Vec<i32>,
    pub bans: HashSet<i32>,
    pub picked: HashSet<i32>,
    pub my_locked: i32,
    pub my_turn: bool,
}

fn id_val(v: Option<&Value>) -> i32 {
    v.and_then(|x| x.as_i64()).unwrap_or(0) as i32
}

pub fn parse_session(session: &Value) -> DraftState {
    let my_cell = session
        .get("localPlayerCellId")
        .and_then(|v| v.as_i64())
        .map(|n| n as i32)
        .unwrap_or(-1);
    let mut st = DraftState {
        my_cell_id: my_cell,
        ..Default::default()
    };

    if let Some(team) = session.get("myTeam").and_then(|v| v.as_array()) {
        for p in team {
            let cid = id_val(p.get("championId"));
            let intent = id_val(p.get("championPickIntent"));
            let is_me = p.get("cellId").and_then(|v| v.as_i64()).map(|n| n as i32) == Some(st.my_cell_id);
            let pos = normalize_position(p.get("assignedPosition").and_then(|v| v.as_str()).unwrap_or(""));
            if is_me {
                st.my_position = pos.clone();
            }
            st.allies.push(Slot {
                champion_id: if cid != 0 { cid } else { intent },
                position: pos,
                tentative: cid == 0 && intent != 0,
                is_me,
            });
        }
    }

    if let Some(team) = session.get("theirTeam").and_then(|v| v.as_array()) {
        for p in team {
            let c = id_val(p.get("championId"));
            if c != 0 {
                st.enemies.push(c);
            }
        }
    }

    if let Some(bans) = session.get("bans") {
        for key in ["myTeamBans", "theirTeamBans"] {
            if let Some(arr) = bans.get(key).and_then(|v| v.as_array()) {
                for v in arr {
                    let c = id_val(Some(v));
                    if c > 0 {
                        st.bans.insert(c);
                    }
                }
            }
        }
    }

    if let Some(actions) = session.get("actions").and_then(|v| v.as_array()) {
        for group in actions {
            let iter: Vec<&Value> = if let Some(arr) = group.as_array() {
                arr.iter().collect()
            } else {
                vec![group]
            };
            for a in iter {
                if !a.is_object() {
                    continue;
                }
                let typ = a.get("type").and_then(|v| v.as_str()).unwrap_or("");
                let in_prog = a.get("isInProgress").and_then(|v| v.as_bool()).unwrap_or(false);
                let actor = a.get("actorCellId").and_then(|v| v.as_i64()).map(|n| n as i32);
                if typ == "pick" && in_prog && actor == Some(st.my_cell_id) {
                    st.my_turn = true;
                }
                let completed = a.get("completed").and_then(|v| v.as_bool()).unwrap_or(false);
                if !completed {
                    continue;
                }
                let cid = id_val(a.get("championId"));
                if cid <= 0 {
                    continue;
                }
                if typ == "ban" {
                    st.bans.insert(cid);
                } else if typ == "pick" {
                    st.picked.insert(cid);
                    if actor == Some(st.my_cell_id) {
                        st.my_locked = cid;
                    }
                }
            }
        }
    }

    for s in &st.allies {
        if s.champion_id != 0 && !s.tentative {
            st.picked.insert(s.champion_id);
        }
    }
    for &c in &st.enemies {
        st.picked.insert(c);
    }

    st
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn test_parse_session_fixture() {
        let p = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/session.json");
        let txt = std::fs::read_to_string(p).unwrap();
        let val: Value = serde_json::from_str(&txt).unwrap();
        let d = parse_session(&val);
        assert_eq!(d.my_cell_id, 2);
        assert_eq!(d.my_position, "support");
        let by_pos: std::collections::HashMap<&str, &Slot> = d
            .allies
            .iter()
            .map(|s| (s.position.as_str(), s))
            .collect();
        assert_eq!(by_pos["bottom"].champion_id, 222);
        assert!(!by_pos["bottom"].tentative);
        assert_eq!(by_pos["jungle"].champion_id, 64);
        assert!(by_pos["jungle"].tentative);
        let me = d.allies.iter().find(|s| s.is_me).unwrap();
        assert_eq!(me.position, "support");
        let mut enemies = d.enemies.clone();
        enemies.sort();
        assert_eq!(enemies, vec![51, 111]);
        assert_eq!(d.bans, HashSet::from([238, 777, 157, 103]));
        assert!(d.picked.contains(&222));
        assert!(d.picked.contains(&111));
    }

    #[test]
    fn test_parse_session_defensive() {
        let d = parse_session(&serde_json::json!({}));
        assert!(d.allies.is_empty());
        assert!(d.bans.is_empty());
        assert_eq!(d.my_position, "");
    }
}
