use regex::Regex;
use serde_json::{Map, Value};
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::{Arc, OnceLock};
use std::time::Duration;

use crate::domain::config::cache_dir;
use crate::domain::scoring::{ChampStats, LANES, Pair, TierRow};

use super::cache::JsonCache;
use super::fetch::Fetcher;

pub const BASE: &str = "https://a1.lolalytics.com/mega/";
pub const SITE: &str = "https://lolalytics.com/lol";
pub const TTL: Duration = Duration::from_secs(24 * 3600);
pub const LANE_RATE_TIER: &str = "master_plus";
pub const BUILD_KEYS: &[&str] = &["header", "summary", "runes", "spells", "boots", "startSet", "item1"];

fn qwik_regex() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r#"(?s)<script type="qwik/json">(.*?)</script>"#).expect("qwik regex"))
}

fn parse_base36(s: &str) -> Option<usize> {
    if s.is_empty() {
        return None;
    }
    usize::from_str_radix(s, 36).ok()
}

fn resolve_qwik(objs: &[Value], i: usize, seen: &mut HashSet<usize>) -> Value {
    if seen.contains(&i) || i >= objs.len() {
        return Value::Null;
    }
    seen.insert(i);
    let out = match &objs[i] {
        Value::String(s) => {
            if let Some(first) = s.bytes().next()
                && first < b' '
                && let Some(idx) = parse_base36(&s[1..])
            {
                resolve_qwik(objs, idx, seen)
            } else {
                Value::String(s.clone())
            }
        }
        Value::Object(map) => {
            let mut res = Map::new();
            for (k, v) in map {
                let val = if let Some(s) = v.as_str()
                    && let Some(idx) = parse_base36(s)
                {
                    resolve_qwik(objs, idx, seen)
                } else {
                    v.clone()
                };
                res.insert(k.clone(), val);
            }
            Value::Object(res)
        }
        Value::Array(arr) => {
            let mut res = Vec::with_capacity(arr.len());
            for v in arr {
                let val = if let Some(s) = v.as_str()
                    && let Some(idx) = parse_base36(s)
                {
                    resolve_qwik(objs, idx, seen)
                } else {
                    v.clone()
                };
                res.push(val);
            }
            Value::Array(res)
        }
        other => other.clone(),
    };
    seen.remove(&i);
    out
}

pub fn qwik_build(html: &str) -> Option<Value> {
    let caps = qwik_regex().captures(html)?;
    let root: Value = serde_json::from_str(caps.get(1)?.as_str()).ok()?;
    let objs = root.get("objs")?.as_array()?;
    for (i, o) in objs.iter().enumerate() {
        if let Some(map) = o.as_object()
            && BUILD_KEYS.iter().all(|k| map.contains_key(*k))
        {
            let mut seen = HashSet::new();
            return Some(resolve_qwik(objs, i, &mut seen));
        }
    }
    None
}

fn json_f64(v: &Value) -> Option<f64> {
    v.as_f64()
        .or_else(|| v.as_i64().map(|n| n as f64))
        .or_else(|| v.as_str().and_then(|s| s.trim().parse::<f64>().ok()))
}

fn json_u32(v: &Value) -> Option<u32> {
    v.as_u64()
        .map(|n| n as u32)
        .or_else(|| json_f64(v).map(|f| f.max(0.0).round() as u32))
}

fn games_estimate(stats: &Value) -> u32 {
    let analysed = stats.get("analysed").and_then(json_f64).unwrap_or(0.0);
    let pr = stats.get("pr").and_then(json_f64).unwrap_or(0.0);
    ((analysed * pr) / 100.0).round().max(0.0) as u32
}

#[derive(Clone)]
pub struct Lolalytics {
    pub tier: String,
    pub days: u32,
    pub fetch: Arc<Fetcher>,
    pub cache: Arc<JsonCache>,
}

impl Lolalytics {
    pub fn new(tier: impl Into<String>, days: u32) -> Self {
        Self {
            tier: tier.into(),
            days,
            fetch: Arc::new(Fetcher::new(8.0, Duration::from_secs(10))),
            cache: Arc::new(JsonCache::new(cache_dir().join("lolalytics"), TTL)),
        }
    }

    pub fn with_cache_dir(tier: impl Into<String>, days: u32, dir: PathBuf) -> Self {
        Self {
            tier: tier.into(),
            days,
            fetch: Arc::new(Fetcher::new(8.0, Duration::from_secs(10))),
            cache: Arc::new(JsonCache::new(dir, TTL)),
        }
    }

    pub fn at_tier(&self, tier: impl Into<String>) -> Self {
        Self {
            tier: tier.into(),
            days: self.days,
            fetch: Arc::clone(&self.fetch),
            cache: Arc::clone(&self.cache),
        }
    }

    async fn get_mega(&self, ep: &str, params: &[(&str, &str)]) -> Arc<Value> {
        let days_str = self.days.to_string();
        let mut q: Vec<(&str, &str)> = vec![
            ("ep", ep),
            ("p", "d"),
            ("v", "1"),
            ("patch", &days_str),
            ("tier", &self.tier),
            ("queue", "420"),
            ("region", "all"),
        ];
        // override tier/etc if provided in params
        for &(k, v) in params {
            if let Some(existing) = q.iter_mut().find(|(ek, _)| *ek == k) {
                existing.1 = v;
            } else {
                q.push((k, v));
            }
        }
        let key = q
            .iter()
            .filter(|(_, v)| !v.is_empty())
            .map(|(_, v)| *v)
            .collect::<Vec<_>>()
            .join("_");
        let c_param = q.iter().find(|(k, _)| *k == "c").map(|(_, v)| *v).unwrap_or("");
        let label = format!("{ep}_{c_param}");

        self.fetch
            .once(&key, || async {
                self.cache
                    .get(&key, &label, || async {
                        let resp = match self.fetch.get(BASE, &q, 3).await {
                            Ok(r) => r,
                            Err(_) => {
                                self.fetch.note_failure();
                                return None;
                            }
                        };
                        if !resp.status().is_success() {
                            self.fetch.note_failure();
                            return None;
                        }
                        let data: Value = match resp.json().await {
                            Ok(d) => d,
                            Err(_) => {
                                self.fetch.note_failure();
                                return None;
                            }
                        };
                        if let Some(status) = data.get("status").and_then(|v| v.as_i64()) {
                            if status == 404 {
                                return Some(Value::Object(Default::default()));
                            }
                            self.fetch.note_failure();
                            return None;
                        }
                        if data.is_object() {
                            Some(data)
                        } else {
                            Some(Value::Object(Default::default()))
                        }
                    })
                    .await
            })
            .await
    }

    pub async fn counters(&self, slug: &str, lane: &str, vslane: &str, tier_override: Option<&str>) -> Arc<Value> {
        let mut params = vec![("c", slug), ("lane", lane), ("vslane", vslane)];
        if let Some(t) = tier_override {
            params.push(("tier", t));
        }
        self.get_mega("counter", &params).await
    }

    pub async fn lane_rates(&self, slug: &str) -> HashMap<String, f64> {
        let data = self
            .counters(slug, "middle", "middle", Some(LANE_RATE_TIER))
            .await;
        let mut out = HashMap::new();
        if let Some(lanes) = data
            .get("stats")
            .and_then(|s| s.get("lanes"))
            .and_then(|l| l.as_object())
        {
            for (k, v) in lanes {
                if let Some(f) = json_f64(v) {
                    out.insert(k.clone(), f);
                }
            }
        }
        out
    }

    pub async fn tier_list(&self, lane: &str, tier: Option<&str>) -> HashMap<i32, TierRow> {
        let t = tier.unwrap_or(LANE_RATE_TIER);
        let data = self.get_mega("list", &[("lane", lane), ("tier", t)]).await;
        let mut out = HashMap::new();
        if let Some(rows) = data.get("cid").and_then(|v| v.as_object()) {
            for (cid_str, row) in rows {
                let Ok(cid) = cid_str.parse::<i32>() else {
                    continue;
                };
                if !row.is_object() {
                    continue;
                }
                let pr = row.get("pr").and_then(json_f64).unwrap_or(0.0);
                let games = row.get("games").and_then(json_u32).unwrap_or(0);
                let avg_wr_delta = row.get("avgWrDelta").and_then(json_f64).unwrap_or(0.0);
                out.insert(cid, TierRow { pr, games, avg_wr_delta });
            }
        }
        out
    }

    pub async fn build_full(&self, slug: &str, lane: &str) -> Arc<Value> {
        self.get_mega("build-full", &[("c", slug), ("lane", lane)]).await
    }

    pub async fn matchup_build(&self, slug: &str, lane: &str, vs_slug: &str, vslane: &str) -> Arc<Value> {
        let days_str = self.days.to_string();
        let params = [
            ("lane", lane),
            ("vslane", vslane),
            ("tier", self.tier.as_str()),
            ("patch", days_str.as_str()),
        ];
        let url = format!("{SITE}/{slug}/vs/{vs_slug}/build/");
        let key = format!(
            "{url}?{}",
            params
                .iter()
                .map(|(k, v)| format!("{k}={v}"))
                .collect::<Vec<_>>()
                .join("&")
        );
        let label = format!("vsbuild_{slug}_{vs_slug}");
        self.fetch
            .once(&key, || async {
                self.cache
                    .get(&key, &label, || async {
                        let resp = match self.fetch.get(&url, &params, 3).await {
                            Ok(r) => r,
                            Err(_) => {
                                self.fetch.note_failure();
                                return None;
                            }
                        };
                        if !resp.status().is_success() {
                            self.fetch.note_failure();
                            return None;
                        }
                        let html = match resp.text().await {
                            Ok(t) => t,
                            Err(_) => {
                                self.fetch.note_failure();
                                return None;
                            }
                        };
                        Some(qwik_build(&html).unwrap_or_else(|| Value::Object(Default::default())))
                    })
                    .await
            })
            .await
    }

    pub async fn champ_stats(&self, slug: &str, lane: &str) -> Option<ChampStats> {
        let mut matchups: HashMap<String, HashMap<i32, Pair>> = HashMap::new();
        let mut stats_val: Option<Value> = None;

        // Fetch all 5 enemy lane counter endpoints + build-team concurrently!
        let team_params = [("c", slug), ("lane", lane)];
        let (c0, c1, c2, c3, c4, team) = tokio::join!(
            self.counters(slug, lane, LANES[0], None),
            self.counters(slug, lane, LANES[1], None),
            self.counters(slug, lane, LANES[2], None),
            self.counters(slug, lane, LANES[3], None),
            self.counters(slug, lane, LANES[4], None),
            self.get_mega("build-team", &team_params),
        );
        let counter_results = [
            (LANES[0], c0),
            (LANES[1], c1),
            (LANES[2], c2),
            (LANES[3], c3),
            (LANES[4], c4),
        ];

        for (vs, data) in counter_results {
            if stats_val.is_none()
                && let Some(s) = data.get("stats")
                && s.is_object()
            {
                stats_val = Some(s.clone());
            }
            let mut map = HashMap::new();
            if let Some(arr) = data.get("counters").and_then(|v| v.as_array()) {
                for row in arr {
                    let Some(cid) = row.get("cid").and_then(|v| v.as_i64()).map(|n| n as i32) else {
                        continue;
                    };
                    let vs_wr = row.get("vsWr").and_then(json_f64).unwrap_or(50.0);
                    let n = row.get("n").and_then(json_u32).unwrap_or(0);
                    let d2 = row.get("d2").and_then(json_f64).unwrap_or(0.0);
                    map.insert(cid, Pair::with_delta(vs_wr, n, d2));
                }
            }
            matchups.insert(vs.to_string(), map);
        }

        let stats = stats_val?;
        let cid = stats.get("cid").and_then(|v| v.as_i64())? as i32;
        let wr = stats.get("wr").and_then(json_f64).unwrap_or(50.0);
        let avg_wr = stats.get("avgWr").and_then(json_f64).unwrap_or(50.0);

        let cols: Vec<&str> = team
            .get("team_h")
            .and_then(|v| v.as_array())
            .map(|a| a.iter().filter_map(|x| x.as_str()).collect())
            .unwrap_or_else(|| vec!["id", "wr", "d1", "d2", "pr", "n"]);
        let idx = |name: &str| cols.iter().position(|&c| c == name);
        let (Some(i_id), Some(i_wr), Some(i_d2), Some(i_n)) =
            (idx("id"), idx("wr"), idx("d2"), idx("n"))
        else {
            return None;
        };

        let mut synergies = HashMap::new();
        if let Some(tmap) = team.get("team").and_then(|v| v.as_object()) {
            for (ally_lane, rows) in tmap {
                let Some(arr) = rows.as_array() else {
                    continue;
                };
                let mut lane_syn = HashMap::new();
                for r in arr {
                    let Some(row) = r.as_array() else {
                        continue;
                    };
                    let max_i = i_id.max(i_wr).max(i_d2).max(i_n);
                    if row.len() <= max_i {
                        continue;
                    }
                    let aid = row[i_id].as_i64().unwrap_or(0) as i32;
                    let awr = json_f64(&row[i_wr]).unwrap_or(50.0);
                    let an = json_u32(&row[i_n]).unwrap_or(0);
                    let ad2 = json_f64(&row[i_d2]).unwrap_or(0.0);
                    lane_syn.insert(aid, Pair::with_delta(awr, an, ad2));
                }
                synergies.insert(ally_lane.clone(), lane_syn);
            }
        }

        let own_games: u32 = matchups
            .get(lane)
            .map(|m| m.values().map(|p| p.games).sum())
            .unwrap_or(0);
        let games = {
            let g = games_estimate(&stats);
            if g > 0 { g } else if own_games > 0 { own_games } else { 1 }
        };

        Some(ChampStats {
            champion_id: cid,
            lane: lane.to_string(),
            wr,
            games,
            avg_wr: Some(avg_wr),
            matchups,
            synergies,
        })
    }
}



#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_qwik_state_decoding() {
        let objs = vec![
            json!({"header": "1", "summary": "2", "runes": "3", "spells": "4", "boots": "4", "startSet": "4", "item1": "4"}),
            json!({"cid": "5", "n": "6"}),
            json!({}),
            json!("\x122"),
            json!(["7"]),
            json!(89),
            json!(4915),
            json!(["8", "9"]),
            json!("4_14"),
            json!(53.6),
        ];
        let html = format!("<script type=\"qwik/json\">{}</script>", json!({"objs": objs}));
        let b = qwik_build(&html).unwrap();
        assert_eq!(b["header"], json!({"cid": 89, "n": 4915}));
        assert_eq!(b["runes"], json!({}));
        assert_eq!(b["spells"], json!([["4_14", 53.6]]));
        assert!(qwik_build("<html></html>").is_none());
    }
}
