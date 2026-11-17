use reqwest::Client;
use serde_json::{Value, json};
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::PathBuf;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use crate::domain::live::{LiveGame, parse_live};

pub const LIVE_URL: &str = "https://127.0.0.1:2999/liveclientdata/allgamedata";

pub struct LiveClient {
    http: Client,
}

impl LiveClient {
    pub fn new(timeout: Duration) -> Self {
        let http = Client::builder()
            .timeout(timeout)
            .danger_accept_invalid_certs(true)
            .build()
            .expect("live reqwest client");
        Self { http }
    }

    pub async fn raw(&self) -> Option<Value> {
        let resp = self.http.get(LIVE_URL).send().await.ok()?;
        if !resp.status().is_success() {
            return None;
        }
        let val: Value = resp.json().await.ok()?;
        if val.is_object() { Some(val) } else { None }
    }

    pub async fn game(&self) -> Option<LiveGame> {
        let raw = self.raw().await?;
        parse_live(&raw)
    }
}

pub struct GameRecorder {
    pub folder: PathBuf,
    pub every: f64,
    pub path: Option<PathBuf>,
    pub last: f64,
}

impl GameRecorder {
    pub fn new(folder: impl Into<PathBuf>, every: f64) -> Self {
        Self {
            folder: folder.into(),
            every,
            path: None,
            last: 0.0,
        }
    }

    fn now_secs() -> f64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs_f64())
            .unwrap_or(0.0)
    }

    pub fn due(&self) -> bool {
        Self::now_secs() - self.last >= self.every
    }

    pub fn save(&mut self, data: &Value) {
        let now = Self::now_secs();
        if self.path.is_none() {
            let _ = fs::create_dir_all(&self.folder);
            self.path = Some(self.folder.join(format!("{}.jsonl", now as u64)));
        }
        if let Some(p) = &self.path
            && let Ok(mut f) = OpenOptions::new().create(true).append(true).open(p)
        {
            let line = json!({"t": (now * 10.0).round() / 10.0, "data": data});
            let _ = writeln!(f, "{line}");
            self.last = now;
        }
    }

    pub fn end(&mut self) {
        self.path = None;
        self.last = 0.0;
    }
}
