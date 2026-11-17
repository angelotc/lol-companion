use std::collections::HashMap;

use super::config::RoleWeights;
use super::scoring::{
    BlindConfig, ChampStats, rank_blind, recommend, shrink,
};

#[derive(Debug, Clone)]
pub struct BlindOpponentRow {
    pub id: i32,
    pub pr: f64,
    pub d: f64,
    pub raw: Option<f64>,
    pub games: u32,
    pub tail: bool,
}

#[derive(Debug, Clone)]
pub struct BlindReportEntry {
    pub id: i32,
    pub score: f64,
    pub base: f64,
    pub mean: f64,
    pub tail: f64,
    pub coverage: f64,
    pub opps: Vec<BlindOpponentRow>,
}

pub fn opponents_of(
    s: &ChampStats,
    role: &str,
    pool_pr: &HashMap<i32, f64>,
    k: f64,
    tail: f64,
) -> Vec<BlindOpponentRow> {
    let pairs = s.matchups.get(role);
    let mut rows: Vec<BlindOpponentRow> = Vec::new();
    for (&cid, &pr) in pool_pr {
        if cid == s.champion_id {
            continue;
        }
        let p = pairs.and_then(|m| m.get(&cid));
        let raw = p.map(|pair| pair.delta.unwrap_or(pair.wr - s.wr));
        let d = match (p, raw) {
            (Some(pair), Some(r)) => shrink(r, pair.games as f64, k),
            _ => 0.0,
        };
        rows.push(BlindOpponentRow {
            id: cid,
            pr,
            d,
            raw,
            games: p.map_or(0, |pair| pair.games),
            tail: false,
        });
    }
    rows.sort_by(|a, b| a.d.total_cmp(&b.d));
    let known_pr: f64 = rows.iter().filter(|r| r.games > 0).map(|r| r.pr).sum();
    let need = tail * known_pr;
    let mut got = 0.0;
    for r in &mut rows {
        if r.games > 0 {
            r.tail = got < need;
            got += r.pr;
        }
    }
    rows
}

pub fn role_report(
    pool: &HashMap<i32, ChampStats>,
    pool_pr: &HashMap<i32, f64>,
    role: &str,
    weights: &RoleWeights,
    k: f64,
    blind_cfg: &BlindConfig,
) -> Vec<BlindReportEntry> {
    let vals: Vec<&ChampStats> = pool.values().collect();
    let base: HashMap<i32, f64> = recommend(&vals, &HashMap::new(), &HashMap::new(), weights, k, 50.0, 0.5, None)
        .into_iter()
        .map(|r| (r.champion_id, r.base))
        .collect();
    let items: Vec<(&ChampStats, f64)> = pool
        .iter()
        .map(|(&cid, s)| (s, base.get(&cid).copied().unwrap_or(0.0)))
        .collect();
    let ranked = rank_blind(&items, role, pool_pr, k, blind_cfg);
    ranked
        .into_iter()
        .map(|b| {
            let s = &pool[&b.champion_id];
            let opps = opponents_of(s, role, pool_pr, k, blind_cfg.tail);
            BlindReportEntry {
                id: b.champion_id,
                score: b.score,
                base: base.get(&b.champion_id).copied().unwrap_or(0.0),
                mean: b.mean,
                tail: b.tail,
                coverage: b.coverage,
                opps,
            }
        })
        .collect()
}
