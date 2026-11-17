use serde_json::{Value, json};
use std::collections::{HashMap, HashSet};
use tracing::info;

use super::lcu::LcuClient;
use super::store::Store;

pub const RANKED_QUEUES: &[i64] = &[420, 440];

#[derive(Debug, Clone, Copy, Default)]
pub struct Mastery {
    pub points: u64,
    pub level: u32,
    pub last_play: f64,
}

pub fn role_from_timeline(timeline: &Value) -> &'static str {
    let lane = timeline
        .get("lane")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_ascii_uppercase();
    let role = timeline
        .get("role")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_ascii_uppercase();
    match (lane.as_str(), role.as_str()) {
        ("TOP", _) => "top",
        ("JUNGLE", _) => "jungle",
        ("MIDDLE", _) | ("MID", _) => "middle",
        ("BOTTOM", "SUPPORT") | ("BOTTOM", "DUO_SUPPORT") | ("NONE", "SUPPORT") => "support",
        ("BOTTOM", "CARRY")
        | ("BOTTOM", "DUO_CARRY")
        | ("BOTTOM", "SOLO")
        | ("BOTTOM", "DUO") => "bottom",
        _ => "",
    }
}

pub async fn fetch_mastery(client: &LcuClient) -> HashMap<i32, Mastery> {
    let Ok(r) = client
        .get("/lol-champion-mastery/v1/local-player/champion-mastery", None)
        .await
    else {
        return HashMap::new();
    };
    if !r.status().is_success() {
        return HashMap::new();
    }
    let Ok(arr) = r.json::<Vec<Value>>().await else {
        return HashMap::new();
    };
    let mut out = HashMap::new();
    for m in arr {
        let Some(cid) = m.get("championId").and_then(|v| v.as_i64()).map(|n| n as i32) else {
            continue;
        };
        if cid == 0 {
            continue;
        }
        let points = m.get("championPoints").and_then(|v| v.as_u64()).unwrap_or(0);
        let level = m.get("championLevel").and_then(|v| v.as_u64()).unwrap_or(0) as u32;
        let last_play = m.get("lastPlayTime").and_then(|v| v.as_f64()).unwrap_or(0.0) / 1000.0;
        out.insert(cid, Mastery { points, level, last_play });
    }
    out
}

pub async fn sync_history(client: &LcuClient, store: &Store) -> usize {
    let queues: HashSet<i64> = RANKED_QUEUES.iter().copied().collect();
    let Ok(r) = client
        .get(
            "/lol-match-history/v1/products/lol/current-summoner/matches?begIndex=0&endIndex=20",
            None,
        )
        .await
    else {
        return 0;
    };
    if !r.status().is_success() {
        return 0;
    }
    let Ok(body) = r.json::<Value>().await else {
        return 0;
    };
    let Some(games) = body
        .get("games")
        .and_then(|g| g.get("games"))
        .and_then(|g| g.as_array())
    else {
        return 0;
    };

    let mut added = 0;
    for g in games {
        let Some(gid) = g.get("gameId").and_then(|v| v.as_i64()) else {
            continue;
        };
        let queue = g.get("queueId").and_then(|v| v.as_i64()).unwrap_or(0);
        if !queues.contains(&queue) || store.has_game(gid).unwrap_or(false) {
            continue;
        }
        let Some(parts) = g.get("participants").and_then(|v| v.as_array()) else {
            continue;
        };
        if parts.len() != 1 {
            continue;
        }
        let me = &parts[0];
        let mut detail = g.clone();
        if let Ok(full_resp) = client.get(&format!("/lol-match-history/v1/games/{gid}"), None).await
            && full_resp.status().is_success()
            && let Ok(full_json) = full_resp.json::<Value>().await
        {
            detail = full_json;
        }
        let created = g.get("gameCreation").and_then(|v| v.as_f64()).unwrap_or(0.0) / 1000.0;
        let champ = me.get("championId").and_then(|v| v.as_i64()).unwrap_or(0) as i32;
        let role = store
            .draft_role_for(champ, created)
            .unwrap_or_else(|| role_from_timeline(me.get("timeline").unwrap_or(&Value::Null)).to_string());
        let win = me
            .get("stats")
            .and_then(|s| s.get("win"))
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        let duration = g.get("gameDuration").and_then(|v| v.as_i64()).unwrap_or(0);
        let payload = json!({
            "participantId": me.get("participantId"),
            "game": detail,
        });
        if store
            .add_game(gid, created, queue, champ, &role, win, duration, &payload)
            .is_ok()
        {
            let _ = store.link_draft(gid, champ, created, win);
            added += 1;
        }
    }
    if added > 0 {
        info!("stored {added} new games");
    }
    added
}
