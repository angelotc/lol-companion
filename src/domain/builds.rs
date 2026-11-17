use serde_json::Value;
use std::collections::HashMap;

use super::scoring::shrink;

pub const MIN_SHARE: f64 = 1.0;

#[inline]
pub fn is_hidden(key: &str) -> bool {
    matches!(key, "0" | "9999" | "1001")
}

#[derive(Debug, Clone, PartialEq)]
pub struct VsDelta {
    pub enemy: i32,
    pub lane: String,
    pub delta: f64,
    pub games: u32,
    pub weighted: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct BuildOption {
    pub key: String,
    pub wr: f64,
    pub pr: f64,
    pub games: u32,
    pub vs: Vec<VsDelta>,
}

impl BuildOption {
    pub fn ids(&self) -> Vec<i32> {
        self.key
            .split('_')
            .filter_map(|x| x.parse::<i32>().ok())
            .collect()
    }

    pub fn matchup(&self) -> f64 {
        self.vs.iter().map(|v| v.weighted).sum()
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct RunePage {
    pub primary: Vec<i32>,
    pub secondary: Vec<i32>,
    pub shards: Vec<i32>,
    pub wr: f64,
    pub games: u32,
}

pub fn shard_keys(shards: &[i32]) -> Vec<String> {
    shards
        .iter()
        .enumerate()
        .map(|(i, s)| {
            if i == 1 {
                format!("{s}f")
            } else {
                s.to_string()
            }
        })
        .collect()
}

#[derive(Debug, Clone)]
pub struct Build {
    pub champion_id: i32,
    pub lane: String,
    pub wr: f64,
    pub games: u32,
    pub pages: HashMap<String, RunePage>,
    pub runes: HashMap<String, BuildOption>,
    pub sections: HashMap<String, Vec<BuildOption>>,
    pub vs: Vec<(i32, String, u32, f64)>, // (enemy, lane, games, wr)
}

fn as_f64(v: &Value) -> f64 {
    v.as_f64()
        .or_else(|| v.as_i64().map(|x| x as f64))
        .unwrap_or(0.0)
}

fn as_u32(v: &Value) -> u32 {
    v.as_u64()
        .or_else(|| v.as_f64().map(|x| x.max(0.0) as u64))
        .unwrap_or(0) as u32
}

fn as_i32(v: &Value) -> Option<i32> {
    v.as_i64()
        .map(|x| x as i32)
        .or_else(|| v.as_str().and_then(|s| s.parse::<i32>().ok()))
}

fn key_str(v: &Value) -> String {
    if let Some(s) = v.as_str() {
        s.to_string()
    } else if let Some(n) = v.as_i64() {
        n.to_string()
    } else if let Some(f) = v.as_f64() {
        (f as i64).to_string()
    } else {
        String::new()
    }
}

fn parse_options(rows: Option<&Value>, min_share: f64, relative: bool) -> Vec<BuildOption> {
    let Some(arr) = rows.and_then(|v| v.as_array()) else {
        return Vec::new();
    };
    let valid: Vec<&Vec<Value>> = arr
        .iter()
        .filter_map(|r| r.as_array())
        .filter(|r| r.len() >= 4)
        .collect();
    let total: f64 = if relative {
        valid.iter().map(|r| as_f64(&r[2])).sum()
    } else {
        0.0
    };
    let scale = if total > 0.0 { 100.0 / total } else { 1.0 };
    let mut out = Vec::new();
    for r in valid {
        let key = key_str(&r[0]);
        let wr = as_f64(&r[1]);
        let pr = as_f64(&r[2]) * scale;
        let games = as_u32(&r[3]);
        if pr >= min_share && games > 0 && !is_hidden(&key) {
            out.push(BuildOption {
                key,
                wr,
                pr,
                games,
                vs: Vec::new(),
            });
        }
    }
    out
}

fn parse_page(p: Option<&Value>) -> Option<RunePage> {
    let p = p?;
    let s = p.get("set")?;
    if !s.is_object() {
        return None;
    }
    let list = |k: &str| -> Vec<i32> {
        s.get(k)
            .and_then(|v| v.as_array())
            .map(|a| a.iter().filter_map(as_i32).collect())
            .unwrap_or_default()
    };
    let pri = list("pri");
    let sec = list("sec");
    let mod_ = list("mod");
    if pri.is_empty() && sec.is_empty() && mod_.is_empty() {
        return None;
    }
    Some(RunePage {
        primary: pri,
        secondary: sec,
        shards: mod_,
        wr: p.get("wr").map_or(0.0, as_f64),
        games: p.get("n").map_or(0, as_u32),
    })
}

pub fn parse_build(raw: &Value) -> Option<Build> {
    let h = raw.get("header")?;
    let cid = h.get("cid").and_then(as_i32).filter(|&c| c != 0)?;
    let n = h.get("n").map_or(0, as_u32);
    if n == 0 {
        return None;
    }
    let lane = h
        .get("lane")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    let wr = h.get("wr").map_or(0.0, as_f64);

    let summary = raw.get("summary");
    let mut pages = HashMap::new();
    for kind in ["pick", "win"] {
        if let Some(pg) = parse_page(summary.and_then(|s| s.get(kind)).and_then(|k| k.get("runes"))) {
            pages.insert(kind.to_string(), pg);
        }
    }

    let mut runes = HashMap::new();
    if let Some(stats) = raw
        .get("runes")
        .and_then(|r| r.get("stats"))
        .and_then(|s| s.as_object())
    {
        for (rid, entries) in stats {
            let Some(arr) = entries.as_array() else {
                continue;
            };
            let best = arr
                .iter()
                .filter_map(|e| e.as_array())
                .filter(|e| e.len() >= 3)
                .max_by(|a, b| as_f64(&a[2]).total_cmp(&as_f64(&b[2])));
            if let Some(b) = best {
                let games = as_u32(&b[2]);
                if games > 0 {
                    runes.insert(
                        rid.clone(),
                        BuildOption {
                            key: rid.clone(),
                            wr: as_f64(&b[1]),
                            pr: as_f64(&b[0]),
                            games,
                            vs: Vec::new(),
                        },
                    );
                }
            }
        }
    }

    let mut sections = HashMap::new();
    sections.insert("spells".into(), parse_options(raw.get("spells"), MIN_SHARE, false));
    sections.insert("start".into(), parse_options(raw.get("startSet"), MIN_SHARE, false));
    sections.insert("boots".into(), parse_options(raw.get("boots"), MIN_SHARE, false));
    sections.insert("core".into(), parse_options(raw.get("builtBootSet3"), 0.5, false));
    sections.insert("skills".into(), parse_options(raw.get("skillOrder"), MIN_SHARE, false));
    sections.insert("skill_order".into(), parse_options(raw.get("skill15"), 0.5, false));
    for i in 1..=5 {
        let key = format!("item{i}");
        sections.insert(key.clone(), parse_options(raw.get(&key), MIN_SHARE, true));
    }

    Some(Build {
        champion_id: cid,
        lane,
        wr,
        games: n,
        pages,
        runes,
        sections,
        vs: Vec::new(),
    })
}

pub fn add_matchup(
    base: &mut Build,
    vs: &Build,
    enemy: i32,
    lane: &str,
    lane_weight: f64,
    k: f64,
    min_games: u32,
) {
    base.vs.push((enemy, lane.to_string(), vs.games, vs.wr));
    let base_wr = base.wr;
    let annotate = |mine: &mut BuildOption, theirs: Option<&BuildOption>| {
        let Some(th) = theirs else {
            return;
        };
        if th.games < min_games {
            return;
        }
        let delta = (th.wr - vs.wr) - (mine.wr - base_wr);
        mine.vs.push(VsDelta {
            enemy,
            lane: lane.to_string(),
            delta,
            games: th.games,
            weighted: shrink(delta, th.games as f64, k) * lane_weight,
        });
    };

    for (name, opts) in &mut base.sections {
        let theirs: HashMap<&str, &BuildOption> = vs
            .sections
            .get(name)
            .map(|v| v.iter().map(|o| (o.key.as_str(), o)).collect())
            .unwrap_or_default();
        for o in opts {
            annotate(o, theirs.get(o.key.as_str()).copied());
        }
    }
    for (rid, o) in &mut base.runes {
        annotate(o, vs.runes.get(rid));
    }
}

pub fn best_vs(options: &[BuildOption], min_edge: f64) -> Option<&BuildOption> {
    options
        .iter()
        .filter(|o| !o.vs.is_empty())
        .max_by(|a, b| a.matchup().total_cmp(&b.matchup()))
        .filter(|o| o.matchup() >= min_edge)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn raw(wr: f64, n: u32, boots: Value) -> Value {
        json!({
            "header": {"cid": 89, "lane": "support", "n": n, "wr": wr},
            "summary": {"pick": {"runes": {"wr": wr, "n": n, "set": {"pri": [8439, 8401, 8473, 8242], "sec": [8347, 8306], "mod": [5007, 5001, 5001]}}}},
            "runes": {"stats": {"8439": [[90, wr, n]], "5001f": [[60, wr, n / 2]]}},
            "boots": boots,
            "spells": [["4_14", wr, 60, n]],
            "skillOrder": [["QWE", wr, 80, n]],
        })
    }

    #[test]
    fn test_parse_keeps_sections_and_hides_noise() {
        let r = raw(51.0, 10000, json!([[3047, 52.0, 50, 5000, 12], [1001, 30.0, 6, 600, 5], [3111, 51.0, 0.5, 50, 14]]));
        let b = parse_build(&r).unwrap();
        let keys: Vec<&str> = b.sections["boots"].iter().map(|o| o.key.as_str()).collect();
        assert_eq!(keys, vec!["3047"]);
        assert_eq!(b.pages["pick"].primary[0], 8439);
        assert_eq!(shard_keys(&b.pages["pick"].shards), vec!["5007", "5001f", "5001"]);
        assert_eq!(b.sections["spells"][0].ids(), vec![4, 14]);
        assert!(parse_build(&json!({})).is_none());
    }

    #[test]
    fn test_matchup_delta_is_relative_to_each_baseline() {
        let mut base = parse_build(&raw(50.0, 50000, json!([[3047, 52.0, 50, 25000, 12], [3111, 50.0, 30, 15000, 14]]))).unwrap();
        let vs = parse_build(&raw(54.0, 5000, json!([[3047, 56.0, 50, 2500, 12], [3111, 57.0, 30, 1500, 14]]))).unwrap();
        add_matchup(&mut base, &vs, 202, "bottom", 1.0, 300.0, 100);
        let boots = &base.sections["boots"];
        assert!(boots[0].vs[0].delta.abs() < 1e-9);
        assert!((boots[1].vs[0].delta - 3.0).abs() < 1e-9);
        assert!((boots[1].matchup() - 3.0 * 1500.0 / 1800.0).abs() < 1e-9);
        assert_eq!(best_vs(boots, 0.3).unwrap().key, "3111");
    }

    #[test]
    fn test_late_item_pick_rates_are_a_share_of_that_slot() {
        let mut r = raw(50.0, 10000, json!([[3047, 52.0, 50, 5000, 12]]));
        r["item5"] = json!([[3109, 53.0, 0.7, 264, 36], [3075, 55.0, 0.3, 120, 36], [2524, 60.0, 0.005, 2, 36]]);
        let b = parse_build(&r).unwrap();
        let opts = &b.sections["item5"];
        let keys: Vec<&str> = opts.iter().map(|o| o.key.as_str()).collect();
        assert_eq!(keys, vec!["3109", "3075"]);
        assert!((opts[0].pr - 0.7 / 1.005 * 100.0).abs() < 1e-6);
    }
}
