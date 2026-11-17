use reqwest::Client;
use serde_json::{Map, Value, json};
use std::collections::HashMap;
use std::path::PathBuf;
use std::time::Duration;

use crate::domain::config::cache_dir;

use super::cache::JsonCache;

pub const BASE: &str = "https://op.gg";
pub const TTL: Duration = Duration::from_secs(6 * 3600);
const UA: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/124.0 Safari/537.36";
const KEY: &str = "\"my_champion_stats\":";

fn array_at(s: &str, start: usize) -> Option<Vec<Value>> {
    let rest = s.get(start..)?;
    let rel = rest.find('[')?;
    let bytes = s.as_bytes();
    let j = start + rel;
    let mut depth = 0i32;
    let mut in_str = false;
    let mut esc = false;
    for (offset, &b) in bytes[j..].iter().enumerate() {
        if in_str {
            if esc {
                esc = false;
            } else if b == b'\\' {
                esc = true;
            } else if b == b'"' {
                in_str = false;
            }
        } else if b == b'"' {
            in_str = true;
        } else if b == b'[' {
            depth += 1;
        } else if b == b']' {
            depth -= 1;
            if depth == 0 {
                let slice = &s[j..=j + offset];
                return serde_json::from_str::<Vec<Value>>(slice).ok();
            }
        }
    }
    None
}

pub fn parse_champions(html: &str) -> Option<HashMap<i32, [u32; 2]>> {
    let s = html.replace("\\\"", "\"");
    let i = s.find(KEY)?;
    let arr = array_at(&s, i + KEY.len())?;
    let mut out = HashMap::new();
    for a in arr {
        let Some(obj) = a.as_object() else {
            continue;
        };
        let cid = obj
            .get("champion_id")
            .or_else(|| obj.get("id"))
            .and_then(|v| v.as_i64())
            .unwrap_or(0) as i32;
        if cid != 0 {
            let play = obj.get("play").and_then(|v| v.as_u64()).unwrap_or(0) as u32;
            let win = obj.get("win").and_then(|v| v.as_u64()).unwrap_or(0) as u32;
            out.insert(cid, [play, win]);
        }
    }
    Some(out)
}

pub struct OpGG {
    cache: JsonCache,
    http: Client,
}

impl OpGG {
    pub fn new() -> Self {
        Self::with_cache(cache_dir().join("opgg"), TTL, Duration::from_secs(10))
    }

    pub fn with_cache(dir: PathBuf, ttl: Duration, timeout: Duration) -> Self {
        let http = Client::builder()
            .user_agent(UA)
            .timeout(timeout)
            .build()
            .expect("opgg client");
        Self {
            cache: JsonCache::new(dir, ttl),
            http,
        }
    }

    async fn resolve_slug(&self, name: &str, tag: &str, region: &str) -> Option<String> {
        let q = format!("{name}#{tag}");
        let resp = self
            .http
            .get(format!("{BASE}/api/lol/summoners/resolve"))
            .query(&[("q", q.as_str()), ("region", region)])
            .header("Accept", "application/json")
            .send()
            .await
            .ok()?;
        if !resp.status().is_success() {
            return None;
        }
        let text = resp.text().await.ok()?;
        for line in text.lines() {
            let Ok(hit) = serde_json::from_str::<Value>(line) else {
                continue;
            };
            if hit.get("type").and_then(|v| v.as_str()) == Some("summoner")
                && let Some(href) = hit.get("href").and_then(|v| v.as_str())
            {
                return Some(href.to_string());
            }
        }
        None
    }

    async fn fetch(&self, name: &str, tag: &str, region: &str) -> Option<Value> {
        let direct = format!(
            "/summoners/{region}/{}-{}",
            urlencoding::encode(name),
            urlencoding::encode(tag)
        );
        for attempt in [Some(direct), None] {
            let path = match attempt {
                Some(p) => Some(p),
                None => self.resolve_slug(name, tag, region).await,
            };
            let Some(path) = path else {
                return None;
            };
            let Ok(resp) = self
                .http
                .get(format!("{BASE}/lol{path}/champions"))
                .query(&[("queue_type", "SOLORANKED")])
                .header("Accept", "text/html")
                .send()
                .await
            else {
                continue;
            };
            if resp.status().is_success()
                && let Ok(html) = resp.text().await
                && let Some(champs) = parse_champions(&html)
            {
                let mut map = Map::new();
                for (k, v) in champs {
                    map.insert(k.to_string(), json!([v[0], v[1]]));
                }
                return Some(json!({"champions": map}));
            }
        }
        None
    }

    pub async fn season(&self, name: &str, tag: &str, region: &str) -> Option<HashMap<i32, [u32; 2]>> {
        if name.is_empty() || tag.is_empty() || region.is_empty() {
            return None;
        }
        let region_l = region.to_ascii_lowercase();
        let key = format!("{region_l}/{name}#{tag}").to_ascii_lowercase();
        let data = self
            .cache
            .get(&key, "season", || self.fetch(name, tag, &region_l))
            .await;
        let obj = data.get("champions")?.as_object()?;
        let mut out = HashMap::new();
        for (k, v) in obj {
            let Ok(cid) = k.parse::<i32>() else {
                continue;
            };
            if let Some(arr) = v.as_array()
                && arr.len() >= 2
            {
                let g = arr[0].as_u64().unwrap_or(0) as u32;
                let w = arr[1].as_u64().unwrap_or(0) as u32;
                out.insert(cid, [g, w]);
            }
        }
        Some(out)
    }
}

impl Default for OpGG {
    fn default() -> Self {
        Self::new()
    }
}
