use serde_json::Value;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;
use tracing::debug;

use crate::domain::players::{
    PlayerRef, PlayerStats, compute_tags, parse_mastery, parse_ranked, parse_teams, refs_from_live,
    summarize_history,
};

use super::lcu::LcuClient;
use super::opgg::OpGG;

pub const LOOKUP_TIMEOUT: Duration = Duration::from_secs(30);

pub enum PlayerClient {
    Live(LcuClient),
    Fixture(FixtureClient),
}

impl PlayerClient {
    async fn get_json(&self, path: &str) -> Result<Value, String> {
        match self {
            PlayerClient::Live(c) => {
                let r = c
                    .get(path, Some(LOOKUP_TIMEOUT))
                    .await
                    .map_err(|e| e.to_string())?;
                if !r.status().is_success() {
                    return Err(format!("{path}: HTTP {}", r.status()));
                }
                r.json::<Value>().await.map_err(|e| e.to_string())
            }
            PlayerClient::Fixture(f) => f.get_json(path),
        }
    }
}

pub struct FixtureClient {
    session: Value,
    ranked: Value,
    history: Value,
    mastery: Value,
}

impl FixtureClient {
    pub fn load(folder: impl AsRef<Path>) -> Result<Self, String> {
        let dir: PathBuf = folder.as_ref().to_path_buf();
        let read = |name: &str| -> Result<Value, String> {
            let txt = fs::read_to_string(dir.join(name)).map_err(|e| e.to_string())?;
            serde_json::from_str(&txt).map_err(|e| e.to_string())
        };
        Ok(Self {
            session: read("session.json")?,
            ranked: read("ranked.json")?,
            history: read("history.json")?,
            mastery: read("mastery.json")?,
        })
    }

    fn get_json(&self, path: &str) -> Result<Value, String> {
        if path == "/lol-gameflow/v1/session" {
            return Ok(self.session.clone());
        }
        if path == "/lol-summoner/v1/current-summoner" {
            return Ok(serde_json::json!({"puuid": self.session.get("_me").and_then(|v| v.as_str()).unwrap_or("")}));
        }
        for (src, prefix) in [
            (&self.ranked, "/lol-ranked/v1/ranked-stats/"),
            (&self.history, "/lol-match-history/v1/products/lol/"),
            (&self.mastery, "/lol-champion-mastery/v1/"),
        ] {
            if let Some(rest) = path.strip_prefix(prefix) {
                let raw_puuid = rest.split('/').next().unwrap_or("").split('?').next().unwrap_or("");
                let puuid = urlencoding::decode(raw_puuid).unwrap_or_else(|_| raw_puuid.into());
                if let Some(val) = src.get(puuid.as_ref()) {
                    return Ok(val.clone());
                }
            }
        }
        Err(format!("{path}: HTTP 404"))
    }
}

pub async fn region(client: &PlayerClient) -> String {
    match client.get_json("/riotclient/region-locale").await {
        Ok(val) => val
            .get("region")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_ascii_lowercase(),
        Err(_) => String::new(),
    }
}

pub async fn game_refs(
    client: &PlayerClient,
    live: Option<&Value>,
    champion_id: impl Fn(&str) -> Option<i32>,
) -> Result<(i64, Vec<PlayerRef>), String> {
    let session = client.get_json("/lol-gameflow/v1/session").await?;
    let me = client
        .get_json("/lol-summoner/v1/current-summoner")
        .await
        .ok()
        .and_then(|v| v.get("puuid").and_then(|p| p.as_str()).map(String::from))
        .unwrap_or_default();
    let game_id = session
        .get("gameData")
        .and_then(|g| g.get("gameId"))
        .and_then(|v| v.as_i64())
        .unwrap_or(0);
    let mut refs = parse_teams(&session, &me);
    if refs.is_empty()
        && let Some(l) = live
    {
        refs = refs_from_live(l, champion_id);
    }
    Ok((game_id, refs))
}

async fn resolve_puuid(client: &PlayerClient, r: &mut PlayerRef) -> Result<(), String> {
    if !r.puuid.is_empty() || r.name.is_empty() || r.tag.is_empty() {
        return Ok(());
    }
    let url = format!(
        "/lol-summoner/v1/alias/lookup?gameName={}&tagLine={}",
        urlencoding::encode(&r.name),
        urlencoding::encode(&r.tag)
    );
    let data = client.get_json(&url).await?;
    r.puuid = data
        .get("puuid")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    Ok(())
}

pub async fn load_one(
    client: &PlayerClient,
    mut r: PlayerRef,
    opgg: Option<&OpGG>,
    reg: &str,
) -> PlayerStats {
    if let Err(e) = resolve_puuid(client, &mut r).await {
        debug!("puuid lookup failed for {}#{}: {e}", r.name, r.tag);
    }
    let mut s = PlayerStats::from_ref(r.clone());
    if r.puuid.is_empty() {
        s.error = true;
        return s;
    }
    let pid = urlencoding::encode(&r.puuid);
    let ranked_path = format!("/lol-ranked/v1/ranked-stats/{pid}");
    let hist_path = format!("/lol-match-history/v1/products/lol/{pid}/matches?begIndex=0&endIndex=19");
    let mast_path = format!("/lol-champion-mastery/v1/{pid}/champion-mastery");

    let (ranked_res, hist_res, mast_res) = tokio::join!(
        client.get_json(&ranked_path),
        client.get_json(&hist_path),
        client.get_json(&mast_path),
    );

    match ranked_res {
        Ok(d) => {
            if let Some(rk) = parse_ranked(&d) {
                s.tier = rk.tier;
                s.division = rk.division;
                s.lp = rk.lp;
                s.wins = rk.wins;
                s.losses = rk.losses;
                s.queue = rk.queue;
            }
        }
        Err(e) => {
            debug!("ranked lookup failed: {e}");
            s.error = true;
        }
    }

    match hist_res {
        Ok(d) => {
            let h = summarize_history(&d, r.champion_id);
            s.games = h.games;
            s.recent = h.recent;
            s.champ_games = h.champ_games;
            s.champ_wins = h.champ_wins;
            s.kills = h.kills;
            s.deaths = h.deaths;
            s.assists = h.assists;
        }
        Err(e) => {
            debug!("history lookup failed: {e}");
            s.error = true;
        }
    }

    match mast_res {
        Ok(d) => {
            s.mastery = parse_mastery(&d, r.champion_id);
        }
        Err(e) => {
            debug!("mastery lookup failed: {e}");
        }
    }

    if let Some(op) = opgg
        && r.champion_id != 0
        && let Some(season) = op.season(&r.name, &r.tag, reg).await
    {
        let pair = season.get(&r.champion_id).copied().unwrap_or([0, 0]);
        s.season_games = Some(pair[0]);
        s.season_wins = Some(pair[1]);
    }

    s.tags = compute_tags(&s);
    s
}

pub async fn load_all(
    client: Arc<PlayerClient>,
    refs: Vec<PlayerRef>,
    opgg: Option<Arc<OpGG>>,
    reg: String,
) -> Vec<PlayerStats> {
    let sem = Arc::new(tokio::sync::Semaphore::new(4));
    let mut handles = Vec::with_capacity(refs.len());
    for r in refs {
        let c = Arc::clone(&client);
        let o = opgg.clone();
        let rg = reg.clone();
        let s = Arc::clone(&sem);
        handles.push(tokio::spawn(async move {
            let _permit = s.acquire().await;
            load_one(&c, r, o.as_deref(), &rg).await
        }));
    }
    let mut out = Vec::with_capacity(handles.len());
    for h in handles {
        if let Ok(st) = h.await {
            out.push(st);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_fixture_client_loads_players() {
        let fixtures = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/players");
        let client = Arc::new(PlayerClient::Fixture(FixtureClient::load(fixtures).unwrap()));
        let (gid, refs) = game_refs(&client, None, |_| None).await.unwrap();
        assert!(gid != 0 || !refs.is_empty());
        let stats = load_all(client, refs, None, "".into()).await;
        assert!(!stats.is_empty());
    }
}
