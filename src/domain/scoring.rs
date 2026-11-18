use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

use super::config::{ComfortConfig, RoleWeights};

pub const LANES: &[&str] = &["top", "jungle", "middle", "bottom", "support"];

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub struct Pair {
    pub wr: f64,
    pub games: u32,
    pub delta: Option<f64>,
}

impl Pair {
    pub fn new(wr: f64, games: u32) -> Self {
        Self {
            wr,
            games,
            delta: None,
        }
    }

    pub fn with_delta(wr: f64, games: u32, delta: f64) -> Self {
        Self {
            wr,
            games,
            delta: Some(delta),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChampStats {
    pub champion_id: i32,
    pub lane: String,
    pub wr: f64,
    pub games: u32,
    pub avg_wr: Option<f64>,
    pub matchups: HashMap<String, HashMap<i32, Pair>>,
    pub synergies: HashMap<String, HashMap<i32, Pair>>,
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]
pub struct Comfort {
    pub games: u32,
    pub wins: u32,
    pub mastery: u64,
    pub eff_games: Option<f64>,
    pub eff_wins: Option<f64>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Contribution {
    pub label: String, // "vs", "with", "you", "mastery"
    pub champion_id: i32,
    pub lane: String,
    pub delta: f64,
    pub games: u32,
    pub weighted: f64,
    pub tentative: bool,
}

#[derive(Debug, Clone)]
pub struct Recommendation {
    pub champion_id: i32,
    pub score: f64,
    pub base: f64,
    pub contributions: Vec<Contribution>,
}

#[inline]
pub fn shrink(delta: f64, games: f64, k: f64) -> f64 {
    if games + k > 0.0 {
        delta * games / (games + k)
    } else {
        0.0
    }
}

pub fn comfort_terms(s: &ChampStats, c: Option<&Comfort>, w: &ComfortConfig) -> Vec<Contribution> {
    let Some(c) = c else {
        return Vec::new();
    };
    if w.weight == 0.0 {
        return Vec::new();
    }
    let mut out = Vec::new();
    let eff = c.eff_games.unwrap_or(c.games as f64);
    if c.games > 0 && c.games >= w.min_games && eff > 0.0 {
        let wins = if c.eff_games.is_some() {
            c.eff_wins.unwrap_or(0.0)
        } else {
            c.wins as f64
        };
        let delta = 100.0 * wins / eff - s.wr;
        let weighted = shrink(delta, eff, w.k) * w.history * w.weight;
        out.push(Contribution {
            label: "you".to_string(),
            champion_id: s.champion_id,
            lane: s.lane.clone(),
            delta,
            games: c.games,
            weighted,
            tentative: false,
        });
    }
    if c.mastery > 0 && w.mastery != 0.0 {
        let bonus = ((1.0 + (c.mastery as f64) / 1000.0).ln() * w.mastery).min(w.mastery_cap);
        out.push(Contribution {
            label: "mastery".to_string(),
            champion_id: s.champion_id,
            lane: s.lane.clone(),
            delta: c.mastery as f64,
            games: 0,
            weighted: bonus * w.weight,
            tentative: false,
        });
    }
    out
}

pub fn recommend(
    candidates: &[&ChampStats],
    allies: &HashMap<String, (i32, bool)>,
    enemies: &HashMap<String, i32>,
    weights: &RoleWeights,
    k: f64,
    avg_wr: f64,
    tentative_factor: f64,
    comfort: Option<&HashMap<i32, Comfort>>,
) -> Vec<Recommendation> {
    let mut recs = Vec::with_capacity(candidates.len());
    for &s in candidates {
        let base = shrink(s.wr - s.avg_wr.unwrap_or(avg_wr), s.games as f64, k) * weights.base;
        let mut contribs = Vec::new();

        for (lane, &cid) in enemies {
            let Some(pair) = s.matchups.get(lane).and_then(|m| m.get(&cid)) else {
                continue;
            };
            let delta = pair.delta.unwrap_or(pair.wr - s.wr);
            let ew = weights.enemy.get(lane).copied().unwrap_or(0.0);
            let w = shrink(delta, pair.games as f64, k) * ew;
            contribs.push(Contribution {
                label: "vs".to_string(),
                champion_id: cid,
                lane: lane.clone(),
                delta,
                games: pair.games,
                weighted: w,
                tentative: false,
            });
        }

        for (lane, &(cid, tent)) in allies {
            let Some(pair) = s.synergies.get(lane).and_then(|m| m.get(&cid)) else {
                continue;
            };
            let delta = pair.delta.unwrap_or(pair.wr - s.wr);
            let aw = weights.ally.get(lane).copied().unwrap_or(0.0);
            let mut w = shrink(delta, pair.games as f64, k) * aw;
            if tent {
                w *= tentative_factor;
            }
            contribs.push(Contribution {
                label: "with".to_string(),
                champion_id: cid,
                lane: lane.clone(),
                delta,
                games: pair.games,
                weighted: w,
                tentative: tent,
            });
        }

        if let Some(cmap) = comfort {
            contribs.extend(comfort_terms(s, cmap.get(&s.champion_id), &weights.comfort));
        }

        let score = base + contribs.iter().map(|c| c.weighted).sum::<f64>();
        recs.push(Recommendation {
            champion_id: s.champion_id,
            score,
            base,
            contributions: contribs,
        });
    }
    recs.sort_by(|a, b| b.score.total_cmp(&a.score));
    recs
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct BlindConfig {
    pub p: f64,
    pub tail: f64,
    pub weight: f64,
}

impl Default for BlindConfig {
    fn default() -> Self {
        Self {
            p: 0.5,
            tail: 0.15,
            weight: 1.0,
        }
    }
}

#[derive(Debug, Clone)]
pub struct Blind {
    pub champion_id: i32,
    pub score: f64,
    pub value: f64,
    pub mean: f64,
    pub tail: f64,
    pub coverage: f64,
    pub counters: Vec<(i32, f64, u32)>,
    pub synergy: f64,
    pub known_tail: f64,
}

pub fn blind_safety(
    s: &ChampStats,
    lane: &str,
    opponents: &HashMap<i32, f64>,
    k: f64,
    base: f64,
    cfg: &BlindConfig,
    n_counters: usize,
    prior_tail: f64,
) -> Option<Blind> {
    let pairs = s.matchups.get(lane);
    let mut rows: Vec<(f64, f64, i32, u32)> = Vec::new();
    let mut total = 0.0;
    for (&cid, &pr) in opponents {
        if cid == s.champion_id || pr <= 0.0 {
            continue;
        }
        total += pr;
        let Some(pair) = pairs.and_then(|m| m.get(&cid)) else {
            continue;
        };
        if pair.games == 0 {
            continue;
        }
        let delta = pair.delta.unwrap_or(pair.wr - s.wr);
        rows.push((shrink(delta, pair.games as f64, k), pr, cid, pair.games));
    }
    if total == 0.0 {
        return None;
    }
    let covered: f64 = rows.iter().map(|r| r.1).sum();
    let cov = covered / total;
    let (mut mean, mut worst) = (0.0, 0.0);
    if covered > 0.0 {
        rows.sort_by(|a, b| a.0.total_cmp(&b.0));
        mean = rows.iter().map(|r| r.0 * r.1).sum::<f64>() / covered;
        let need = cfg.tail.max(1e-9) * covered;
        let (mut acc, mut got) = (0.0, 0.0);
        for &(d, pr, _, _) in &rows {
            let w = pr.min(need - got);
            acc += d * w;
            got += w;
            if got >= need {
                break;
            }
        }
        worst = acc / got;
    }
    let known = worst;
    mean *= cov;
    worst = cov * worst + (1.0 - cov) * prior_tail;
    let value = cfg.weight * ((1.0 - cfg.p) * mean + cfg.p * worst);
    let counters = rows
        .iter()
        .filter(|r| r.0 < 0.0)
        .take(n_counters)
        .map(|&(d, _, cid, g)| (cid, d, g))
        .collect();
    Some(Blind {
        champion_id: s.champion_id,
        score: base + value,
        value,
        mean,
        tail: worst,
        coverage: cov,
        counters,
        synergy: 0.0,
        known_tail: known,
    })
}

pub fn typical_tail(blinds: &[Blind], min_coverage: f64) -> f64 {
    let mut tails: Vec<f64> = blinds
        .iter()
        .filter(|b| b.coverage >= min_coverage)
        .map(|b| b.known_tail)
        .collect();
    if tails.is_empty() {
        return 0.0;
    }
    tails.sort_by(|a, b| a.total_cmp(b));
    tails[tails.len() / 2]
}

pub fn rank_blind(
    items: &[(&ChampStats, f64)],
    lane: &str,
    opponents: &HashMap<i32, f64>,
    k: f64,
    cfg: &BlindConfig,
) -> Vec<Blind> {
    let pass1: Vec<Blind> = items
        .iter()
        .filter_map(|&(s, base)| blind_safety(s, lane, opponents, k, base, cfg, 3, 0.0))
        .collect();
    let prior = typical_tail(&pass1, 0.5);
    let mut pass2: Vec<Blind> = items
        .iter()
        .filter_map(|&(s, base)| blind_safety(s, lane, opponents, k, base, cfg, 3, prior))
        .collect();
    pass2.sort_by(|a, b| b.score.total_cmp(&a.score));
    pass2
}

fn permutations_rec(
    lanes: &[&'static str],
    ids: &[i32],
    lane_rates: &HashMap<i32, HashMap<String, f64>>,
    used_mask: u8,
    idx: usize,
    cur_total: f64,
    cur_assign: &mut [usize; 5],
    best: &mut Option<(f64, [usize; 5])>,
) {
    if idx == ids.len() {
        if best.as_ref().is_none_or(|b| cur_total > b.0) {
            *best = Some((cur_total, *cur_assign));
        }
        return;
    }
    let cid = ids[idx];
    let rates = lane_rates.get(&cid);
    for (li, &lane) in lanes.iter().enumerate() {
        if (used_mask & (1 << li)) != 0 {
            continue;
        }
        let r = rates.and_then(|m| m.get(lane)).copied().unwrap_or(0.0);
        cur_assign[idx] = li;
        permutations_rec(
            lanes,
            ids,
            lane_rates,
            used_mask | (1 << li),
            idx + 1,
            cur_total + r,
            cur_assign,
            best,
        );
    }
}

pub fn infer_enemy_lanes(
    enemy_ids: &[i32],
    lane_rates: &HashMap<i32, HashMap<String, f64>>,
    lanes: &[&'static str],
) -> HashMap<String, i32> {
    let n = enemy_ids.len().min(lanes.len()).min(5);
    let ids = &enemy_ids[..n];
    let mut cur = [0usize; 5];
    let mut best = None;
    permutations_rec(lanes, ids, lane_rates, 0, 0, 0.0, &mut cur, &mut best);
    let Some((_, assign)) = best else {
        return HashMap::new();
    };
    (0..n)
        .map(|i| (lanes[assign[i]].to_string(), ids[i]))
        .collect()
}

pub fn assign_enemy_lanes(
    enemy_ids: &[i32],
    lane_rates: &HashMap<i32, HashMap<String, f64>>,
    fixed: Option<&HashMap<i32, String>>,
) -> HashMap<String, i32> {
    let mut pinned: HashMap<String, i32> = HashMap::new();
    if let Some(f) = fixed {
        for (&cid, lane) in f {
            if enemy_ids.contains(&cid) && LANES.contains(&lane.as_str()) {
                pinned.insert(lane.clone(), cid);
            }
        }
    }
    let pinned_cids: HashSet<i32> = pinned.values().copied().collect();
    let rest: Vec<i32> = enemy_ids
        .iter()
        .copied()
        .filter(|c| !pinned_cids.contains(c))
        .collect();
    let free_lanes: Vec<&'static str> = LANES
        .iter()
        .copied()
        .filter(|&l| !pinned.contains_key(l))
        .collect();
    pinned.extend(infer_enemy_lanes(&rest, lane_rates, &free_lanes));
    pinned
}

#[derive(Debug, Clone)]
pub struct OddsTerm {
    pub kind: String,
    pub side: String,
    pub champions: Vec<i32>,
    pub lanes: Vec<String>,
    pub value: f64,
}

#[derive(Debug, Clone)]
pub struct DraftOdds {
    pub win: f64,
    pub terms: Vec<OddsTerm>,
    pub known: usize,
    pub pending: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct OddsConfig {
    pub strength: f64,
    pub same_lane: f64,
    pub duo: f64,
    pub cross: f64,
    pub synergy: f64,
    pub clamp: f64,
    pub cal_a: Option<f64>,
    pub cal_b: Option<f64>,
}

impl Default for OddsConfig {
    fn default() -> Self {
        Self {
            strength: 0.5,
            same_lane: 1.0,
            duo: 0.7,
            cross: 0.35,
            synergy: 0.5,
            clamp: 25.0,
            cal_a: None,
            cal_b: None,
        }
    }
}

#[inline]
pub fn sigmoid(x: f64) -> f64 {
    1.0 / (1.0 + (-x).exp())
}

fn pair_weight(a: &str, b: &str, w: &OddsConfig) -> f64 {
    if a == b {
        return w.same_lane;
    }
    let is_duo = |l: &str| l == "bottom" || l == "support";
    if is_duo(a) && is_duo(b) {
        return w.duo;
    }
    w.cross
}

pub fn draft_odds(
    ours: &HashMap<String, (&ChampStats, bool)>,
    theirs: &HashMap<String, (&ChampStats, bool)>,
    k: f64,
    w: &OddsConfig,
    tentative_factor: f64,
) -> DraftOdds {
    let mut terms = Vec::new();
    let tf = |a: bool, b: bool| if a || b { tentative_factor } else { 1.0 };

    for (team, sign) in [(ours, 1.0), (theirs, -1.0)] {
        for (lane, &(s, tent)) in team {
            let v = shrink(s.wr - s.avg_wr.unwrap_or(50.0), s.games as f64, k)
                * w.strength
                * tf(tent, false)
                * sign;
            terms.push(OddsTerm {
                kind: "strength".into(),
                side: if v >= 0.0 { "us" } else { "them" }.into(),
                champions: vec![s.champion_id],
                lanes: vec![lane.clone()],
                value: v,
            });
        }
        let entries: Vec<(&String, &(&ChampStats, bool))> = team.iter().collect();
        for i in 0..entries.len() {
            let (la, &(a, ta)) = entries[i];
            for j in (i + 1)..entries.len() {
                let (lb, &(b, tb)) = entries[j];
                let mut vals = Vec::new();
                for (x, y, ly) in [(a, b, lb.as_str()), (b, a, la.as_str())] {
                    if let Some(pair) = x.synergies.get(ly).and_then(|m| m.get(&y.champion_id)) {
                        let d = pair.delta.unwrap_or(pair.wr - x.wr);
                        vals.push(shrink(d, pair.games as f64, k));
                    }
                }
                if !vals.is_empty() {
                    let avg = vals.iter().sum::<f64>() / vals.len() as f64;
                    let v = avg * w.synergy * tf(ta, tb) * sign;
                    terms.push(OddsTerm {
                        kind: "synergy".into(),
                        side: if v >= 0.0 { "us" } else { "them" }.into(),
                        champions: vec![a.champion_id, b.champion_id],
                        lanes: vec![la.clone(), lb.clone()],
                        value: v,
                    });
                }
            }
        }
    }

    for (la, &(a, ta)) in ours {
        for (lb, &(b, tb)) in theirs {
            let mut vals = Vec::new();
            if let Some(pa) = a.matchups.get(lb).and_then(|m| m.get(&b.champion_id)) {
                vals.push(shrink(pa.delta.unwrap_or(pa.wr - a.wr), pa.games as f64, k));
            }
            if let Some(pb) = b.matchups.get(la).and_then(|m| m.get(&a.champion_id)) {
                vals.push(-shrink(pb.delta.unwrap_or(pb.wr - b.wr), pb.games as f64, k));
            }
            if !vals.is_empty() {
                let avg = vals.iter().sum::<f64>() / vals.len() as f64;
                let v = avg * pair_weight(la, lb, w) * tf(ta, tb);
                terms.push(OddsTerm {
                    kind: "matchup".into(),
                    side: if v >= 0.0 { "us" } else { "them" }.into(),
                    champions: vec![a.champion_id, b.champion_id],
                    lanes: vec![la.clone(), lb.clone()],
                    value: v,
                });
            }
        }
    }

    let total: f64 = terms.iter().map(|t| t.value).sum();
    let win = if let Some(cal_a) = w.cal_a {
        100.0 * sigmoid(cal_a * total + w.cal_b.unwrap_or(0.0))
    } else {
        (50.0 + total).clamp(50.0 - w.clamp, 50.0 + w.clamp)
    };
    terms.sort_by(|a, b| b.value.abs().total_cmp(&a.value.abs()));
    DraftOdds {
        win,
        terms,
        known: ours.len() + theirs.len(),
        pending: 0,
    }
}

#[derive(Debug, Clone)]
pub struct TierRow {
    pub pr: f64,
    pub games: u32,
    pub avg_wr_delta: f64,
}

#[derive(Debug, Clone)]
pub struct Threat {
    pub champion_id: i32,
    pub lane: String,
    pub score: f64,
    pub pick_rate: f64,
    pub strength: f64,
    pub against: Vec<Contribution>,
}

pub fn ban_threats(
    picks: &[&ChampStats],
    enemy_rows: &HashMap<String, HashMap<i32, TierRow>>,
    weights: &RoleWeights,
    k: f64,
    exclude: &HashSet<i32>,
    min_pick_rate: f64,
    allies: &[(&ChampStats, RoleWeights)],
) -> Vec<Threat> {
    let mut threats: HashMap<i32, Threat> = HashMap::new();
    let mut best: HashMap<i32, f64> = HashMap::new();

    for (lane, rows) in enemy_rows {
        let lw = weights.enemy.get(lane).copied().unwrap_or(0.0);
        if lw <= 0.0 {
            continue;
        }
        for (&cid, row) in rows {
            if exclude.contains(&cid) || row.pr < min_pick_rate {
                continue;
            }
            let mut against = Vec::new();
            for &s in picks {
                if let Some(pair) = s.matchups.get(lane).and_then(|m| m.get(&cid)) {
                    let delta = pair.delta.unwrap_or(pair.wr - s.wr);
                    against.push(Contribution {
                        label: "vs".into(),
                        champion_id: s.champion_id,
                        lane: lane.clone(),
                        delta,
                        games: pair.games,
                        weighted: shrink(delta, pair.games as f64, k) * lw,
                        tentative: false,
                    });
                }
            }
            let mut harm = if picks.is_empty() {
                0.0
            } else {
                -against.iter().map(|c| c.weighted).sum::<f64>() / picks.len() as f64
            };
            for (a, aw) in allies {
                let alw = aw.enemy.get(lane).copied().unwrap_or(0.0);
                if alw <= 0.0 {
                    continue;
                }
                if let Some(pair) = a.matchups.get(lane).and_then(|m| m.get(&cid)) {
                    let delta = pair.delta.unwrap_or(pair.wr - a.wr);
                    let c = Contribution {
                        label: "vs".into(),
                        champion_id: a.champion_id,
                        lane: lane.clone(),
                        delta,
                        games: pair.games,
                        weighted: shrink(delta, pair.games as f64, k) * alw,
                        tentative: false,
                    };
                    harm -= c.weighted;
                    against.push(c);
                }
            }
            let strength = shrink(row.avg_wr_delta, row.games as f64, k);
            let lane_score = row.pr / 100.0 * (harm + strength * lw);
            match threats.get_mut(&cid) {
                None => {
                    threats.insert(
                        cid,
                        Threat {
                            champion_id: cid,
                            lane: lane.clone(),
                            score: lane_score,
                            pick_rate: row.pr,
                            strength,
                            against,
                        },
                    );
                    best.insert(cid, lane_score);
                }
                Some(t) => {
                    if lane_score > best[&cid] {
                        t.lane = lane.clone();
                        t.strength = strength;
                        t.against = against;
                        best.insert(cid, lane_score);
                    }
                    t.score += lane_score;
                    t.pick_rate += row.pr;
                }
            }
        }
    }

    let mut out: Vec<Threat> = threats.into_values().collect();
    out.sort_by(|a, b| b.score.total_cmp(&a.score));
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::config::default_weights;

    fn stats(cid: i32, wr: f64, games: u32) -> ChampStats {
        ChampStats {
            champion_id: cid,
            lane: "support".into(),
            wr,
            games,
            avg_wr: None,
            matchups: HashMap::new(),
            synergies: HashMap::new(),
        }
    }

    #[test]
    fn test_shrink() {
        assert!((shrink(10.0, 500.0, 500.0) - 5.0).abs() < 1e-9);
        assert_eq!(shrink(10.0, 0.0, 500.0), 0.0);
        assert!((shrink(10.0, 1e9, 500.0) - 10.0).abs() < 1e-3);
    }

    #[test]
    fn zero_game_matchups_do_not_count_toward_blind_coverage() {
        let mut s = stats(40, 53.0, 28000);
        s.matchups.insert("support".into(), HashMap::from([
            (111, Pair::with_delta(51.0, 2000, -2.0)),
            (412, Pair::with_delta(100.0, 0, 50.0)),
        ]));
        let opponents = HashMap::from([(111, 10.0), (412, 10.0)]);
        let blind = blind_safety(&s, "support", &opponents, 500.0, 0.0, &BlindConfig::default(), 3, -3.0).unwrap();
        assert_eq!(blind.coverage, 0.5);
        assert!((blind.tail - (-2.3)).abs() < 1e-9);
    }

    #[test]
    fn test_ordering_and_missing_data() {
        let w = default_weights("support");
        let a = stats(1, 52.0, 10000);
        let mut b = stats(2, 50.0, 10000);
        b.matchups.insert("support".into(), HashMap::from([(99, Pair::new(55.0, 5000))]));
        let c = stats(3, 51.0, 10000);
        let enemies = HashMap::from([("support".into(), 99)]);
        let recs = recommend(&[&a, &b, &c], &HashMap::new(), &enemies, &w, 500.0, 50.0, 0.5, None);
        let ids: Vec<i32> = recs.iter().map(|r| r.champion_id).collect();
        assert_eq!(ids, vec![2, 1, 3]);
        assert!(recs[1].contributions.is_empty());
    }

    #[test]
    fn test_shrinkage_prefers_large_sample() {
        let w = default_weights("support");
        let mut tiny = stats(1, 50.0, 10000);
        tiny.matchups.insert("support".into(), HashMap::from([(9, Pair::new(80.0, 5))]));
        let mut big = stats(2, 50.0, 10000);
        big.matchups.insert("support".into(), HashMap::from([(9, Pair::new(56.0, 5000))]));
        let enemies = HashMap::from([("support".into(), 9)]);
        let recs = recommend(&[&tiny, &big], &HashMap::new(), &enemies, &w, 500.0, 50.0, 0.5, None);
        assert_eq!(recs[0].champion_id, 2);
    }

    #[test]
    fn test_tentative_factor() {
        let w = default_weights("support");
        let mut s = stats(1, 50.0, 10000);
        s.synergies.insert("bottom".into(), HashMap::from([(7, Pair::new(54.0, 2000))]));
        let firm = &recommend(&[&s], &HashMap::from([("bottom".into(), (7, false))]), &HashMap::new(), &w, 500.0, 50.0, 0.5, None)[0];
        let tent = &recommend(&[&s], &HashMap::from([("bottom".into(), (7, true))]), &HashMap::new(), &w, 500.0, 50.0, 0.5, None)[0];
        assert!(((tent.score - tent.base) - (firm.score - firm.base) * 0.5).abs() < 1e-9);
        assert!(tent.contributions[0].tentative);
    }

    #[test]
    fn test_infer_enemy_lanes() {
        let rates = HashMap::from([
            (1, HashMap::from([("top".into(), 0.9), ("middle".into(), 0.1)])),
            (2, HashMap::from([("top".into(), 0.8), ("jungle".into(), 0.9)])),
            (3, HashMap::from([("bottom".into(), 0.95)])),
            (4, HashMap::from([("support".into(), 0.9), ("bottom".into(), 0.3)])),
        ]);
        let res = infer_enemy_lanes(&[1, 2, 3, 4, 5], &rates, LANES);
        assert_eq!(res["top"], 1);
        assert_eq!(res["jungle"], 2);
        assert_eq!(res["bottom"], 3);
        assert_eq!(res["support"], 4);
        assert_eq!(res["middle"], 5);
        assert_eq!(res.values().collect::<HashSet<_>>().len(), 5);
    }

    #[test]
    fn test_infer_unknown_gets_leftover() {
        let rates = HashMap::from([(1, HashMap::from([("top".into(), 1.0)]))]);
        let res = infer_enemy_lanes(&[1, 2], &rates, LANES);
        assert_eq!(res["top"], 1);
        let mut vals: Vec<i32> = res.values().copied().collect();
        vals.sort();
        assert_eq!(vals, vec![1, 2]);
    }
}
