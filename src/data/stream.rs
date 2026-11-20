use serde::{Deserialize, Serialize};
use sha1::{Digest, Sha1};
use std::collections::{BTreeMap, HashMap};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Mutex;

use crate::domain::config::StreamConfig;
use crate::domain::lcu::DraftState;
use crate::domain::state::Stage;

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct StreamAllySlot {
    pub cell_id: i32,
    pub position: String,
    pub champion_id: i32,
    pub tentative: bool,
    pub locked: bool,
    pub is_host: bool,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct StreamDraftPayload {
    pub stage: String,
    pub tier: String,
    pub my_cell_id: i32,
    pub my_position: String,
    pub allies: Vec<StreamAllySlot>,
    pub enemies: Vec<i32>,
    pub my_team_bans: Vec<i32>,
    pub their_team_bans: Vec<i32>,
    pub bans: Vec<i32>,
    pub picked: Vec<i32>,
    pub active_cells: Vec<i32>,
    pub enemy_lanes: BTreeMap<String, String>,
}

impl StreamDraftPayload {
    pub fn from_draft(
        stage: Stage,
        tier: &str,
        draft: &DraftState,
        enemy_lanes: &HashMap<i32, String>,
    ) -> Self {
        let mut bans: Vec<i32> = draft.bans.iter().copied().collect();
        bans.sort_unstable();

        let mut picked: Vec<i32> = draft.picked.iter().copied().collect();
        picked.sort_unstable();

        let allies = draft
            .allies
            .iter()
            .map(|s| StreamAllySlot {
                cell_id: s.cell_id,
                position: s.position.clone(),
                champion_id: s.champion_id,
                tentative: s.tentative,
                locked: s.locked,
                is_host: s.is_me,
            })
            .collect();

        let enemy_lanes_map: BTreeMap<String, String> = enemy_lanes
            .iter()
            .map(|(cid, lane)| (cid.to_string(), lane.clone()))
            .collect();

        Self {
            stage: stage.as_str().to_string(),
            tier: tier.to_string(),
            my_cell_id: draft.my_cell_id,
            my_position: draft.my_position.clone(),
            allies,
            enemies: draft.enemies.clone(),
            my_team_bans: draft.my_team_bans.clone(),
            their_team_bans: draft.their_team_bans.clone(),
            bans,
            picked,
            active_cells: draft.active_cells.clone(),
            enemy_lanes: enemy_lanes_map,
        }
    }
}

#[derive(Debug, Clone)]
struct ActiveRoom {
    room_id: String,
    url: String,
    write_token: String,
}

#[derive(Default)]
struct StreamInner {
    active: Option<ActiveRoom>,
    last_hash: Option<[u8; 20]>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CreateRoomResponse {
    room_id: String,
    url: String,
    write_token: String,
}

pub struct LiveStreamer {
    client: reqwest::Client,
    inner: Arc<Mutex<StreamInner>>,
}

impl Default for LiveStreamer {
    fn default() -> Self {
        Self::new()
    }
}

impl LiveStreamer {
    pub fn new() -> Self {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(5))
            .build()
            .unwrap_or_else(|_| reqwest::Client::new());
        Self {
            client,
            inner: Arc::new(Mutex::new(StreamInner::default())),
        }
    }

    pub async fn start_room(&self, cfg: &StreamConfig, tier: &str) -> Result<String, String> {
        let base = cfg.endpoint.trim().trim_end_matches('/');
        if base.is_empty() {
            return Err("stream endpoint is empty".to_string());
        }
        let url = format!("{base}/api/rooms");
        let mut req = self
            .client
            .post(&url)
            .json(&serde_json::json!({ "tier": tier }));
        let api_key = cfg.api_key.trim();
        if !api_key.is_empty() {
            req = req.header("X-API-Key", api_key);
        }
        let resp = req
            .send()
            .await
            .map_err(|e| format!("POST {url} failed: {e}"))?;
        let status = resp.status();
        if !status.is_success() {
            let body = resp.text().await.unwrap_or_default();
            return Err(format!("POST {url} returned {status}: {body}"));
        }
        let parsed: CreateRoomResponse = resp
            .json()
            .await
            .map_err(|e| format!("decode create room response: {e}"))?;
        let room_url = parsed.url.clone();
        let mut inner = self.inner.lock().await;
        inner.active = Some(ActiveRoom {
            room_id: parsed.room_id,
            url: parsed.url,
            write_token: parsed.write_token,
        });
        inner.last_hash = None;
        Ok(room_url)
    }

    pub async fn push_state(
        &self,
        cfg: &StreamConfig,
        payload: &StreamDraftPayload,
    ) -> Result<bool, String> {
        let bytes = serde_json::to_vec(payload)
            .map_err(|e| format!("serialize stream payload: {e}"))?;
        let hash: [u8; 20] = Sha1::digest(&bytes).into();

        let active = {
            let inner = self.inner.lock().await;
            if inner.last_hash == Some(hash) {
                return Ok(false);
            }
            match &inner.active {
                Some(a) => a.clone(),
                None => return Ok(false),
            }
        };

        let base = cfg.endpoint.trim().trim_end_matches('/');
        if base.is_empty() {
            return Err("stream endpoint is empty".to_string());
        }
        let url = format!("{base}/api/rooms/{}/state", active.room_id);
        let resp = self
            .client
            .post(&url)
            .header("Authorization", format!("Bearer {}", active.write_token))
            .header("Content-Type", "application/json")
            .body(bytes)
            .send()
            .await
            .map_err(|e| format!("POST {url} failed: {e}"))?;
        let status = resp.status();
        if !status.is_success() {
            let body = resp.text().await.unwrap_or_default();
            return Err(format!("POST {url} returned {status}: {body}"));
        }

        let mut inner = self.inner.lock().await;
        if inner
            .active
            .as_ref()
            .is_some_and(|a| a.room_id == active.room_id)
        {
            inner.last_hash = Some(hash);
        }
        Ok(true)
    }

    pub async fn end_room(&self, cfg: &StreamConfig) -> Result<(), String> {
        let active = {
            let mut inner = self.inner.lock().await;
            inner.last_hash = None;
            inner.active.take()
        };
        let Some(active) = active else {
            return Ok(());
        };
        let base = cfg.endpoint.trim().trim_end_matches('/');
        if base.is_empty() {
            return Ok(());
        }
        let url = format!("{base}/api/rooms/{}/end", active.room_id);
        let resp = self
            .client
            .post(&url)
            .header("Authorization", format!("Bearer {}", active.write_token))
            .json(&serde_json::json!({}))
            .send()
            .await
            .map_err(|e| format!("POST {url} failed: {e}"))?;
        let status = resp.status();
        if !status.is_success() {
            let body = resp.text().await.unwrap_or_default();
            return Err(format!("POST {url} returned {status}: {body}"));
        }
        Ok(())
    }

    #[allow(dead_code)]
    pub async fn current_url(&self) -> Option<String> {
        self.inner.lock().await.active.as_ref().map(|a| a.url.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::config::Config;
    use crate::domain::lcu::Slot;
    use std::collections::HashSet;

    #[test]
    fn test_stream_payload_deterministic_and_sorted() {
        let draft = DraftState {
            my_cell_id: 2,
            my_position: "support".into(),
            allies: vec![
                Slot {
                    cell_id: 0,
                    champion_id: 86,
                    position: "top".into(),
                    tentative: false,
                    locked: true,
                    is_me: false,
                },
                Slot {
                    cell_id: 2,
                    champion_id: 412,
                    position: "support".into(),
                    tentative: true,
                    locked: false,
                    is_me: true,
                },
            ],
            enemies: vec![51, 111],
            my_team_bans: vec![238, 777],
            their_team_bans: vec![157, 103],
            bans: HashSet::from([777, 103, 238, 157]),
            picked: HashSet::from([222, 51, 111]),
            active_cells: vec![2],
            my_locked: 0,
            my_turn: true,
        };
        let enemy_lanes = HashMap::from([(111, "support".to_string()), (51, "bottom".to_string())]);

        let p1 = StreamDraftPayload::from_draft(Stage::Pick, "master_plus", &draft, &enemy_lanes);
        let p2 = StreamDraftPayload::from_draft(Stage::Pick, "master_plus", &draft, &enemy_lanes);

        assert_eq!(p1.bans, vec![103, 157, 238, 777]);
        assert_eq!(p1.picked, vec![51, 111, 222]);
        assert_eq!(p1.allies[0].is_host, false);
        assert_eq!(p1.allies[1].is_host, true);

        let j1 = serde_json::to_string(&p1).unwrap();
        let j2 = serde_json::to_string(&p2).unwrap();
        assert_eq!(j1, j2);

        let val: serde_json::Value = serde_json::from_str(&j1).unwrap();
        assert_eq!(val["stage"], "pick");
        assert_eq!(val["tier"], "master_plus");
        assert_eq!(val["myCellId"], 2);
        assert_eq!(val["myPosition"], "support");
        assert_eq!(val["allies"][0]["cellId"], 0);
        assert_eq!(val["allies"][0]["isHost"], false);
        assert_eq!(val["allies"][1]["isHost"], true);
        assert_eq!(val["myTeamBans"], serde_json::json!([238, 777]));
        assert_eq!(val["theirTeamBans"], serde_json::json!([157, 103]));
        assert_eq!(val["bans"], serde_json::json!([103, 157, 238, 777]));
        assert_eq!(val["picked"], serde_json::json!([51, 111, 222]));
        assert_eq!(val["activeCells"], serde_json::json!([2]));
        assert_eq!(val["enemyLanes"]["111"], "support");
        assert_eq!(val["enemyLanes"]["51"], "bottom");
    }

    #[test]
    fn test_config_stream_parsing() {
        let default_cfg: Config = toml::from_str("").unwrap();
        assert!(!default_cfg.stream.enabled);
        assert_eq!(default_cfg.stream.endpoint, "");
        assert_eq!(default_cfg.stream.api_key, "");
        assert!(!default_cfg.stream.auto_copy_link);

        let custom: Config = toml::from_str(
            r#"
[stream]
enabled = true
endpoint = "https://draft.example.com/"
api_key = "secret-key"
auto_copy_link = true
"#,
        )
        .unwrap();
        assert!(custom.stream.enabled);
        assert_eq!(custom.stream.endpoint, "https://draft.example.com/");
        assert_eq!(custom.stream.api_key, "secret-key");
        assert!(custom.stream.auto_copy_link);
    }

    #[tokio::test]
    async fn test_streamer_current_url_default_none() {
        let s = LiveStreamer::new();
        assert!(s.current_url().await.is_none());
    }
}
