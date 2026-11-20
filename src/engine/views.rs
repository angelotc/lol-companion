use std::collections::{HashMap, HashSet};

use crate::domain::blind_report::BlindReportEntry;
use crate::domain::builds::{self, BuildOption, RunePage};
use crate::domain::champions::Champions;
use crate::domain::ddragon::{Assets, SHARD_ROWS};
use crate::domain::live::LiveGame;
use crate::domain::players::PlayerStats;
use crate::domain::scoring::{DraftOdds, LANES};
use crate::domain::state::Stage;

use super::recommender::{BanEvaluation, BuildEvaluation, Evaluation};

pub const TIERS: &[(&str, &str)] = &[("master_plus", "Master+")];
pub const LOW_DATA: f64 = 0.25;
pub const FLEX: f64 = 10.0;
pub const SUPPORT_ITEMS: &[i32] = &[3865, 3866, 3867, 3869, 3870, 3871, 3876, 3877];

pub fn stage_message(stage: Stage) -> &'static str {
    match stage {
        Stage::Idle => "Waiting for champ select...",
        Stage::Lobby => "In lobby - waiting for you to queue...",
        Stage::Queue => "In queue - waiting for a match...",
        Stage::Ready => "Match found!",
        Stage::Loading => "Game starting...",
        Stage::Ingame => "In game",
        Stage::Postgame => "Game over",
        _ => "",
    }
}

#[derive(Debug, Clone)]
pub struct ChampRef {
    pub id: i32,
    pub name: String,
    pub icon: String,
}

impl ChampRef {
    pub fn from_id(cid: i32, champs: &Champions) -> Self {
        Self {
            id: cid,
            name: champs.name(cid),
            icon: champs.icon_url(cid),
        }
    }
}

#[derive(Debug, Clone)]
pub struct HeaderView {
    pub role: String,
    pub tier: String,
    pub tier_id: String,
    pub days: u32,
}

#[derive(Debug, Clone)]
pub struct AllySlotView {
    pub champ: ChampRef,
    pub lane: String,
    pub tentative: bool,
    pub is_me: bool,
}

#[derive(Debug, Clone)]
pub struct EnemySlotView {
    pub champ: ChampRef,
    pub lane: String,
    pub manual: bool,
    pub flex: bool,
    pub rates: HashMap<String, i32>,
}

#[derive(Debug, Clone)]
pub struct ContribView {
    pub champ: ChampRef,
    pub kind: String,
    pub lane: String,
    pub delta: f64,
    pub games: u32,
    pub weighted: f64,
    pub tentative: bool,
}

#[derive(Debug, Clone)]
pub struct RecView {
    pub champ: ChampRef,
    pub score: f64,
    pub base: f64,
    pub contributions: Vec<ContribView>,
}

#[derive(Debug, Clone)]
pub struct CounterView {
    pub champ: ChampRef,
    pub delta: f64,
    pub games: u32,
}

#[derive(Debug, Clone)]
pub struct BlindView {
    pub champ: ChampRef,
    pub score: f64,
    pub value: f64,
    pub mean: f64,
    pub tail: f64,
    pub synergy: f64,
    pub coverage: i32,
    pub counters: Vec<CounterView>,
}

#[derive(Debug, Clone)]
pub struct OddsReasonView {
    pub kind: String,
    pub label: String,
    pub value: f64,
    pub icons: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct OddsView {
    pub win: f64,
    pub known: usize,
    pub pending: usize,
    pub reasons: Vec<OddsReasonView>,
}

#[derive(Debug, Clone)]
pub struct PicksView {
    pub enemy_ad_pct: Option<i32>,
    pub note: String,
    pub locked: Option<ChampRef>,
    pub allies: Vec<AllySlotView>,
    pub enemies: Vec<EnemySlotView>,
    pub bans: Vec<ChampRef>,
    pub blind: Vec<BlindView>,
    pub recs: Vec<RecView>,
    pub odds: Option<OddsView>,
}

#[derive(Debug, Clone)]
pub struct ThreatAgainstView {
    pub champ: ChampRef,
    pub delta: f64,
    pub games: u32,
    pub weighted: f64,
}

#[derive(Debug, Clone)]
pub struct ThreatView {
    pub champ: ChampRef,
    pub lane: String,
    pub score: f64,
    pub pick_rate: f64,
    pub strength: f64,
    pub against: Vec<ThreatAgainstView>,
}

#[derive(Debug, Clone)]
pub struct BansView {
    pub allies: Vec<AllySlotView>,
    pub bans: Vec<ChampRef>,
    pub picks: Vec<ChampRef>,
    pub threats: Vec<ThreatView>,
}

#[derive(Debug, Clone)]
pub struct PartView {
    pub id: i32,
    pub name: String,
    pub icon: String,
    pub plaintext: String,
    pub gold: i32,
    pub owned: bool,
}

#[derive(Debug, Clone)]
pub struct VsOptionView {
    pub champ: ChampRef,
    pub lane: String,
    pub delta: f64,
    pub weighted: f64,
    pub games: u32,
}

#[derive(Debug, Clone)]
pub struct OptionView {
    pub parts: Vec<PartView>,
    pub wr: f64,
    pub pr: f64,
    pub games: u32,
    pub best: bool,
    pub matchup: f64,
    pub has_vs: bool,
    pub yours: bool,
    pub vs: Vec<VsOptionView>,
}

#[derive(Debug, Clone)]
pub struct RuneNodeView {
    pub id: i32,
    pub name: String,
    pub icon: String,
    pub wr: Option<f64>,
    pub pr: Option<f64>,
    pub games: Option<u32>,
    pub matchup: f64,
    pub has_vs: bool,
    pub vs: Vec<VsOptionView>,
    pub chosen: bool,
}

#[derive(Debug, Clone)]
pub struct RunePageView {
    pub label: String,
    pub wr: f64,
    pub games: u32,
    pub small: bool,
    pub styles: Vec<RuneNodeView>,
    pub primary: Vec<RuneNodeView>,
    pub secondary: Vec<RuneNodeView>,
    pub shards: Vec<RuneNodeView>,
    pub win_shards: Vec<RuneNodeView>,
    pub tree_primary: Vec<Vec<RuneNodeView>>,
    pub tree_secondary: Vec<Vec<RuneNodeView>>,
    pub tree_shards: Vec<Vec<RuneNodeView>>,
}

#[derive(Debug, Clone)]
pub struct EnemyDamageView {
    pub name: String,
    pub icon: String,
    pub lane: String,
    pub physical: i32,
    pub magic: i32,
    pub true_dmg: i32,
}

#[derive(Debug, Clone)]
pub struct DamageSplitView {
    pub physical: i32,
    pub magic: i32,
    pub true_dmg: i32,
    pub enemies: Vec<EnemyDamageView>,
}

#[derive(Debug, Clone)]
pub struct BuildSectionView {
    pub key: String,
    pub title: String,
    pub state: String,
    pub options: Vec<OptionView>,
}

#[derive(Debug, Clone)]
pub struct SkillSummaryView {
    pub key: String,
    pub wr: f64,
    pub pr: f64,
    pub games: u32,
}

#[derive(Debug, Clone)]
pub struct BuildView {
    pub champ: ChampRef,
    pub empty: Option<String>,
    pub games: u32,
    pub wr: f64,
    pub lane: String,
    pub opponents: Vec<(ChampRef, String, u32, f64)>,
    pub damage: Option<DamageSplitView>,
    pub pages: Vec<RunePageView>,
    pub keystones: Vec<RuneNodeView>,
    pub spells: Vec<OptionView>,
    pub skills: Vec<SkillSummaryView>,
    pub skill_order: String,
    pub items: Vec<BuildSectionView>,
}

#[derive(Debug, Clone)]
pub struct IngameBuildView {
    pub champ: Option<ChampRef>,
    pub level: i32,
    pub gold: i32,
    pub time_secs: i32,
    pub empty: Option<String>,
    pub opponents: Vec<(ChampRef, String)>,
    pub damage: Option<DamageSplitView>,
    pub core: Vec<PartView>,
    pub next: Option<PartView>,
    pub sections: Vec<BuildSectionView>,
    pub skill_order: String,
    pub keystone: Option<RuneNodeView>,
}

#[derive(Debug, Clone)]
pub struct PlayerView {
    pub name: String,
    pub tag: String,
    pub me: bool,
    pub lane: String,
    pub champ: Option<ChampRef>,
    pub rank: String,
    pub tier: String,
    pub queue: String,
    pub rank_full: String,
    pub wr: Option<i32>,
    pub season_games: u32,
    pub wins: u32,
    pub champ_games: u32,
    pub champ_wr: Option<i32>,
    pub kda: Option<f64>,
    pub kills: Option<f64>,
    pub deaths: Option<f64>,
    pub assists: Option<f64>,
    pub champ_season_games: Option<u32>,
    pub champ_season_wr: Option<i32>,
    pub games: u32,
    pub recent: String,
    pub mastery: Option<u64>,
    pub tags: Vec<String>,
    pub error: bool,
}

#[derive(Debug, Clone)]
pub struct TeamView {
    pub side: String,
    pub label: String,
    pub players: Vec<PlayerView>,
}

#[derive(Debug, Clone)]
pub struct ClientScreen {
    pub header: HeaderView,
    pub status: String,
    pub message: String,
    pub detail: String,
    pub stage: Stage,
    pub bans: Option<BansView>,
    pub picks: Option<PicksView>,
    pub build: Option<BuildView>,
    pub explore: bool,
    pub updated_hms: String,
    pub live_url: Option<String>,
    pub auto_copy_live_url: bool,
}

#[derive(Debug, Clone)]
pub struct GameScreen {
    pub header: HeaderView,
    pub status: String,
    pub build: Option<IngameBuildView>,
    pub build_note: String,
    pub teams: Option<Vec<TeamView>>,
    pub players_note: String,
}

#[derive(Debug, Clone)]
pub struct ExploreOppView {
    pub champ: ChampRef,
    pub pr: f64,
    pub d: f64,
    pub raw: Option<f64>,
    pub games: u32,
    pub tail: bool,
}

#[derive(Debug, Clone)]
pub struct ExploreChampView {
    pub champ: ChampRef,
    pub score: f64,
    pub base: f64,
    pub mean: f64,
    pub tail: f64,
    pub coverage: i32,
    pub opps: Vec<ExploreOppView>,
}

#[derive(Debug, Clone)]
pub struct ExploreReportView {
    pub role: String,
    pub tier: String,
    pub tier_label: String,
    pub days: u32,
    pub champs: Vec<ExploreChampView>,
}

fn r1(v: f64) -> f64 {
    (v * 10.0).round() / 10.0
}

fn r2(v: f64) -> f64 {
    (v * 100.0).round() / 100.0
}

pub fn header_view(role: Option<&str>, tier: &str, days: u32) -> HeaderView {
    let label = TIERS
        .iter()
        .find(|&&(id, _)| id == tier)
        .map(|&(_, l)| l)
        .unwrap_or(tier);
    HeaderView {
        role: role.unwrap_or("").to_string(),
        tier: label.to_string(),
        tier_id: tier.to_string(),
        days,
    }
}

pub fn picks_view(
    ev: &Evaluation,
    champs: &Champions,
    enemy_lanes: &HashMap<i32, String>,
    odds: Option<&DraftOdds>,
) -> PicksView {
    let found = ev
        .recs
        .iter()
        .flat_map(|r| r.contributions.iter())
        .filter(|c| c.label == "vs")
        .count();
    let possible = ev.recs.len() * ev.draft.enemies.len();
    let note = if possible > 0 && (found as f64 / possible as f64) < LOW_DATA {
        "Few matchups at this tier: scores lean on overall win rate.".to_string()
    } else {
        String::new()
    };

    let enemies = ev
        .draft
        .enemies
        .iter()
        .map(|&cid| {
            let rates = ev.enemy_rates.get(&cid).cloned().unwrap_or_default();
            let mut ranked: Vec<f64> = rates.values().copied().collect();
            ranked.sort_by(|a, b| b.total_cmp(a));
            let flex = ranked.len() > 1 && ranked[1] >= FLEX;
            let rounded_rates = LANES
                .iter()
                .map(|&l| (l.to_string(), rates.get(l).copied().unwrap_or(0.0).round() as i32))
                .collect();
            EnemySlotView {
                champ: ChampRef::from_id(cid, champs),
                lane: ev.lanes.get(&cid).cloned().unwrap_or_default(),
                manual: enemy_lanes.contains_key(&cid),
                flex,
                rates: rounded_rates,
            }
        })
        .collect();

    let allies = ev
        .draft
        .allies
        .iter()
        .filter(|s| s.champion_id != 0)
        .map(|s| AllySlotView {
            champ: ChampRef::from_id(s.champion_id, champs),
            lane: if s.position.is_empty() {
                ev.lanes.get(&s.champion_id).cloned().unwrap_or_default()
            } else {
                s.position.clone()
            },
            tentative: s.tentative,
            is_me: s.is_me,
        })
        .collect();

    let mut ban_ids: Vec<i32> = ev.draft.bans.iter().copied().collect();
    ban_ids.sort();
    let bans = ban_ids
        .into_iter()
        .map(|c| ChampRef::from_id(c, champs))
        .collect();

    let blind = ev
        .blind
        .iter()
        .map(|b| BlindView {
            champ: ChampRef::from_id(b.champion_id, champs),
            score: r2(b.score),
            value: r2(b.value),
            mean: r2(b.mean),
            tail: r2(b.tail),
            synergy: r2(b.synergy),
            coverage: (b.coverage * 100.0).round() as i32,
            counters: b
                .counters
                .iter()
                .map(|&(cid, dl, g)| CounterView {
                    champ: ChampRef::from_id(cid, champs),
                    delta: r2(dl),
                    games: g,
                })
                .collect(),
        })
        .collect();

    let recs = ev
        .recs
        .iter()
        .map(|r| {
            let mut cs = r.contributions.clone();
            cs.sort_by(|a, b| b.weighted.abs().total_cmp(&a.weighted.abs()));
            RecView {
                champ: ChampRef::from_id(r.champion_id, champs),
                score: r2(r.score),
                base: r2(r.base),
                contributions: cs
                    .into_iter()
                    .map(|c| ContribView {
                        champ: ChampRef::from_id(c.champion_id, champs),
                        kind: c.label,
                        lane: c.lane,
                        delta: r2(c.delta),
                        games: c.games,
                        weighted: r2(c.weighted),
                        tentative: c.tentative,
                    })
                    .collect(),
            }
        })
        .collect();

    let enemy_ad_pct = champs
        .team_damage(&ev.draft.enemies)
        .map(|(ad, _)| (ad * 100.0).round() as i32);

    PicksView {
        enemy_ad_pct,
        note,
        locked: if ev.draft.my_locked != 0 {
            Some(ChampRef::from_id(ev.draft.my_locked, champs))
        } else {
            None
        },
        allies,
        enemies,
        bans,
        blind,
        recs,
        odds: odds.map(|o| odds_view(o, champs, 4)),
    }
}

pub fn odds_view(o: &DraftOdds, champs: &Champions, top: usize) -> OddsView {
    let reasons = o
        .terms
        .iter()
        .take(top)
        .filter(|t| t.value.abs() >= 0.1)
        .map(|t| {
            let label = if t.kind == "strength" {
                format!(
                    "{} ({})",
                    champs.name(t.champions[0]),
                    t.lanes.first().map(String::as_str).unwrap_or("")
                )
            } else {
                let joiner = if t.kind == "matchup" { " vs " } else { " + " };
                t.champions
                    .iter()
                    .map(|&c| champs.name(c))
                    .collect::<Vec<_>>()
                    .join(joiner)
            };
            OddsReasonView {
                kind: t.kind.clone(),
                label,
                value: r2(t.value),
                icons: t.champions.iter().map(|&c| champs.icon_url(c)).collect(),
            }
        })
        .collect();

    OddsView {
        win: r1(o.win),
        known: o.known,
        pending: o.pending,
        reasons,
    }
}

pub fn bans_view(bev: &BanEvaluation, champs: &Champions) -> BansView {
    let allies = bev
        .draft
        .allies
        .iter()
        .filter(|s| s.champion_id != 0)
        .map(|s| AllySlotView {
            champ: ChampRef::from_id(s.champion_id, champs),
            lane: s.position.clone(),
            tentative: s.tentative,
            is_me: s.is_me,
        })
        .collect();
    let mut ban_ids: Vec<i32> = bev.draft.bans.iter().copied().collect();
    ban_ids.sort();
    let bans = ban_ids
        .into_iter()
        .map(|c| ChampRef::from_id(c, champs))
        .collect();
    let picks = bev
        .picks
        .iter()
        .map(|&c| ChampRef::from_id(c, champs))
        .collect();
    let threats = bev
        .threats
        .iter()
        .map(|t| {
            let mut ag = t.against.clone();
            ag.sort_by(|a, b| a.weighted.total_cmp(&b.weighted));
            ThreatView {
                champ: ChampRef::from_id(t.champion_id, champs),
                lane: t.lane.clone(),
                score: (t.score * 1000.0).round() / 1000.0,
                pick_rate: r1(t.pick_rate),
                strength: r2(t.strength),
                against: ag
                    .into_iter()
                    .map(|c| ThreatAgainstView {
                        champ: ChampRef::from_id(c.champion_id, champs),
                        delta: r2(c.delta),
                        games: c.games,
                        weighted: r2(c.weighted),
                    })
                    .collect(),
            }
        })
        .collect();

    BansView {
        allies,
        bans,
        picks,
        threats,
    }
}

pub fn damage_split(bev: Option<&BuildEvaluation>, champs: &Champions) -> Option<DamageSplitView> {
    let bev = bev?;
    if bev.damage.is_empty() {
        return None;
    }
    let pct = |d: &HashMap<String, f64>| -> (i32, i32, i32) {
        let sum: f64 = d.values().sum::<f64>().max(1.0);
        let p = (100.0 * d.get("physical").copied().unwrap_or(0.0) / sum).round() as i32;
        let m = (100.0 * d.get("magic").copied().unwrap_or(0.0) / sum).round() as i32;
        let t = (100.0 * d.get("true").copied().unwrap_or(0.0) / sum).round() as i32;
        (p, m, t)
    };
    let mut total = HashMap::new();
    for key in ["physical", "magic", "true"] {
        let s: f64 = bev.damage.values().map(|d| d.get(key).copied().unwrap_or(0.0)).sum();
        total.insert(key.to_string(), s);
    }
    let (tp, tm, tt) = pct(&total);
    let lanes: HashMap<i32, &str> = bev.opponents.iter().map(|(l, &c)| (c, l.as_str())).collect();
    let mut entries: Vec<(i32, &HashMap<String, f64>)> = bev.damage.iter().map(|(&c, d)| (c, d)).collect();
    entries.sort_by(|a, b| {
        let sa: f64 = a.1.values().sum();
        let sb: f64 = b.1.values().sum();
        sb.total_cmp(&sa)
    });
    let enemies = entries
        .into_iter()
        .map(|(cid, d)| {
            let (ep, em, et) = pct(d);
            EnemyDamageView {
                name: champs.name(cid),
                icon: champs.icon_url(cid),
                lane: lanes.get(&cid).copied().unwrap_or("").to_string(),
                physical: ep,
                magic: em,
                true_dmg: et,
            }
        })
        .collect();

    Some(DamageSplitView {
        physical: tp,
        magic: tm,
        true_dmg: tt,
        enemies,
    })
}

fn rune_node(
    key: &str,
    build: Option<&builds::Build>,
    assets: &Assets,
    champs: &Champions,
    chosen: bool,
) -> RuneNodeView {
    let rid = key.trim_end_matches('f').parse::<i32>().unwrap_or(0);
    let (id, name, icon) = assets.rune_info(rid);
    if let Some(o) = build.and_then(|b| b.runes.get(key)) {
        RuneNodeView {
            id,
            name,
            icon,
            wr: Some(r1(o.wr)),
            pr: Some(r1(o.pr)),
            games: Some(o.games),
            matchup: r2(o.matchup()),
            has_vs: !o.vs.is_empty(),
            vs: o
                .vs
                .iter()
                .map(|v| VsOptionView {
                    champ: ChampRef::from_id(v.enemy, champs),
                    lane: v.lane.clone(),
                    delta: r2(v.delta),
                    weighted: r2(v.weighted),
                    games: v.games,
                })
                .collect(),
            chosen,
        }
    } else {
        RuneNodeView {
            id,
            name,
            icon,
            wr: None,
            pr: None,
            games: None,
            matchup: 0.0,
            has_vs: false,
            vs: Vec::new(),
            chosen,
        }
    }
}

pub fn build_view(bev: &BuildEvaluation, champs: &Champions, assets: &Assets) -> BuildView {
    let champ = ChampRef::from_id(bev.champion_id, champs);
    let Some(b) = &bev.build else {
        return BuildView {
            champ,
            empty: Some("No build data for this champion and role at this tier.".into()),
            games: 0,
            wr: 0.0,
            lane: bev.role.clone(),
            opponents: Vec::new(),
            damage: damage_split(Some(bev), champs),
            pages: Vec::new(),
            keystones: Vec::new(),
            spells: Vec::new(),
            skills: Vec::new(),
            skill_order: String::new(),
            items: Vec::new(),
        };
    };

    let make_option = |o: &BuildOption, is_summoner: bool, best: Option<&BuildOption>| -> OptionView {
        let parts = o
            .ids()
            .into_iter()
            .map(|id| {
                if is_summoner {
                    let (pid, name, icon) = assets.summoner_info(id);
                    PartView {
                        id: pid,
                        name,
                        icon,
                        plaintext: String::new(),
                        gold: 0,
                        owned: false,
                    }
                } else {
                    let (pid, name, icon, plaintext) = assets.item_info(id);
                    let gold = assets.items.get(&id).map_or(0, |it| it.gold);
                    PartView {
                        id: pid,
                        name,
                        icon,
                        plaintext,
                        gold,
                        owned: false,
                    }
                }
            })
            .collect();
        OptionView {
            parts,
            wr: r1(o.wr),
            pr: r1(o.pr),
            games: o.games,
            best: best.is_some_and(|bk| bk.key == o.key),
            matchup: r2(o.matchup()),
            has_vs: !o.vs.is_empty(),
            yours: false,
            vs: o
                .vs
                .iter()
                .map(|v| VsOptionView {
                    champ: ChampRef::from_id(v.enemy, champs),
                    lane: v.lane.clone(),
                    delta: r2(v.delta),
                    weighted: r2(v.weighted),
                    games: v.games,
                })
                .collect(),
        }
    };

    let tree_rows = |pg: &RunePage, secondary: bool| -> Vec<Vec<RuneNodeView>> {
        let chosen = if secondary { &pg.secondary } else { &pg.primary };
        let style = chosen
            .first()
            .and_then(|r| assets.runes.get(r))
            .map(|r| r.style);
        let Some(all_rows) = style.and_then(|st| assets.trees.get(&st)) else {
            return Vec::new();
        };
        let rows = if secondary && all_rows.len() > 1 {
            &all_rows[1..]
        } else {
            &all_rows[..]
        };
        if rows.is_empty()
            || chosen
                .iter()
                .any(|c| !rows.iter().any(|row| row.contains(c)))
        {
            return Vec::new();
        }
        rows.iter()
            .map(|row| {
                row.iter()
                    .map(|&r| rune_node(&r.to_string(), Some(b), assets, champs, chosen.contains(&r)))
                    .collect()
            })
            .collect()
    };

    let shard_rows = |shards: &[i32]| -> Vec<Vec<RuneNodeView>> {
        let keys = builds::shard_keys(shards);
        shards
            .iter()
            .zip(keys)
            .enumerate()
            .map(|(i, (&sid, key))| {
                let row: Vec<i32> = if i < SHARD_ROWS.len() && SHARD_ROWS[i].contains(&sid) {
                    SHARD_ROWS[i].to_vec()
                } else {
                    vec![sid]
                };
                let sfx = &key[sid.to_string().len()..];
                row.into_iter()
                    .map(|r| rune_node(&format!("{r}{sfx}"), Some(b), assets, champs, r == sid))
                    .collect()
            })
            .collect()
    };

    let mut pages: Vec<RunePageView> = Vec::new();
    for (kind, label) in [("pick", "Most picked"), ("win", "Highest win rate")] {
        let Some(pg) = b.pages.get(kind) else {
            continue;
        };
        if kind == "win"
            && let Some(first) = pages.first_mut()
            && let Some(pick_pg) = b.pages.get("pick")
            && pg.primary == pick_pg.primary
            && pg.secondary == pick_pg.secondary
        {
            first.label = "Most picked · highest win rate".to_string();
            if pg.shards != pick_pg.shards {
                first.win_shards = builds::shard_keys(&pg.shards)
                    .into_iter()
                    .map(|k| rune_node(&k, Some(b), assets, champs, true))
                    .collect();
            }
            continue;
        }

        let mut styles = Vec::new();
        for rid in pg.primary.iter().take(1).chain(pg.secondary.iter().take(1)) {
            if let Some(r) = assets.runes.get(rid) {
                styles.push(rune_node(&r.style.to_string(), None, assets, champs, false));
            }
        }
        pages.push(RunePageView {
            label: label.to_string(),
            wr: r1(pg.wr),
            games: pg.games,
            small: pg.games < 300,
            styles,
            primary: pg
                .primary
                .iter()
                .map(|r| rune_node(&r.to_string(), Some(b), assets, champs, true))
                .collect(),
            secondary: pg
                .secondary
                .iter()
                .map(|r| rune_node(&r.to_string(), Some(b), assets, champs, true))
                .collect(),
            shards: builds::shard_keys(&pg.shards)
                .into_iter()
                .map(|k| rune_node(&k, Some(b), assets, champs, true))
                .collect(),
            win_shards: Vec::new(),
            tree_primary: tree_rows(pg, false),
            tree_secondary: tree_rows(pg, true),
            tree_shards: shard_rows(&pg.shards),
        });
    }

    let mut ks_opts: Vec<&BuildOption> = b
        .runes
        .iter()
        .filter(|(k, o)| {
            k.chars().all(|c| c.is_ascii_digit())
                && k.parse::<i32>()
                    .ok()
                    .and_then(|id| assets.runes.get(&id))
                    .is_some_and(|r| r.slot == 0)
                && o.pr >= 1.0
        })
        .map(|(_, o)| o)
        .collect();
    ks_opts.sort_by(|a, b| b.pr.total_cmp(&a.pr));
    let keystones = ks_opts
        .into_iter()
        .take(3)
        .map(|o| rune_node(&o.key, Some(b), assets, champs, false))
        .collect();

    let item_sections = [
        ("start", "Start"),
        ("core", "Core"),
        ("boots", "Boots"),
        ("item1", "Support item"),
        ("item2", "2nd item"),
        ("item3", "3rd item"),
        ("item4", "4th item"),
        ("item5", "5th item"),
    ];
    let mut items = Vec::new();
    for (key, mut title) in item_sections {
        let Some(opts) = b.sections.get(key) else {
            continue;
        };
        if opts.is_empty() {
            continue;
        }
        if key == "item1"
            && !opts
                .first()
                .is_some_and(|o| o.ids().iter().any(|i| SUPPORT_ITEMS.contains(i)))
        {
            title = "1st item";
        }
        let best = builds::best_vs(opts, 0.3);
        let mut shown: Vec<&BuildOption> = opts.iter().take(4).collect();
        if let Some(bo) = best
            && !shown.iter().any(|x| x.key == bo.key)
        {
            shown.push(bo);
        }
        items.push(BuildSectionView {
            key: key.to_string(),
            title: title.to_string(),
            state: String::new(),
            options: shown
                .into_iter()
                .map(|o| make_option(o, false, best))
                .collect(),
        });
    }

    let spells_list = b.sections.get("spells").map(Vec::as_slice).unwrap_or(&[]);
    let best_spell = builds::best_vs(spells_list, 0.3);
    let spells = spells_list
        .iter()
        .take(3)
        .map(|o| make_option(o, true, best_spell))
        .collect();

    let skills = b
        .sections
        .get("skills")
        .map(|s| {
            s.iter()
                .take(2)
                .map(|o| SkillSummaryView {
                    key: o.key.clone(),
                    wr: r1(o.wr),
                    pr: r1(o.pr),
                    games: o.games,
                })
                .collect()
        })
        .unwrap_or_default();

    let skill_order = b
        .sections
        .get("skill_order")
        .and_then(|s| s.first())
        .map(|o| {
            o.key
                .chars()
                .map(|ch| match ch {
                    '1' => 'Q',
                    '2' => 'W',
                    '3' => 'E',
                    '4' => 'R',
                    _ => '?',
                })
                .collect()
        })
        .unwrap_or_default();

    BuildView {
        champ,
        empty: None,
        games: b.games,
        wr: r1(b.wr),
        lane: bev.role.clone(),
        opponents: b
            .vs
            .iter()
            .map(|(e, l, g, w)| (ChampRef::from_id(*e, champs), l.clone(), *g, r1(*w)))
            .collect(),
        damage: damage_split(Some(bev), champs),
        pages,
        keystones,
        spells,
        skills,
        skill_order,
        items,
    }
}

fn owned_item(iid: i32, items: &HashSet<i32>, assets: &Assets) -> bool {
    if items.contains(&iid) {
        return true;
    }
    if SUPPORT_ITEMS.contains(&iid) {
        return items.iter().any(|i| SUPPORT_ITEMS.contains(i));
    }
    if let Some(it) = assets.items.get(&iid)
        && it.tags.iter().any(|t| t == "Boots")
    {
        return items.iter().any(|&i| {
            i != 1001
                && assets
                    .items
                    .get(&i)
                    .is_some_and(|o| o.tags.iter().any(|t| t == "Boots"))
        });
    }
    false
}

fn extra_items(items: &HashSet<i32>, core: &[i32], assets: &Assets) -> Vec<i32> {
    let mut sorted: Vec<i32> = items.iter().copied().collect();
    sorted.sort();
    sorted
        .into_iter()
        .filter(|&iid| {
            if core.contains(&iid) || SUPPORT_ITEMS.contains(&iid) {
                return false;
            }
            let Some(it) = assets.items.get(&iid) else {
                return false;
            };
            it.into.is_empty() && it.gold >= 1000 && !it.tags.iter().any(|t| t == "Boots")
        })
        .collect()
}

pub fn ingame_build_view(
    live: &LiveGame,
    bev: Option<&BuildEvaluation>,
    champs: &Champions,
    assets: &Assets,
) -> IngameBuildView {
    let champ = bev.map(|b| ChampRef::from_id(b.champion_id, champs));
    let Some(b) = bev.and_then(|x| x.build.as_ref()) else {
        return IngameBuildView {
            champ,
            level: live.level,
            gold: live.gold as i32,
            time_secs: live.game_time as i32,
            empty: Some("No build data for this champion and role.".into()),
            opponents: Vec::new(),
            damage: damage_split(bev, champs),
            core: Vec::new(),
            next: None,
            sections: Vec::new(),
            skill_order: String::new(),
            keystone: None,
        };
    };

    let owned: HashSet<i32> = live.items.iter().copied().collect();
    let make_item = |iid: i32| -> PartView {
        let (id, name, icon, plaintext) = assets.item_info(iid);
        let gold = assets.items.get(&iid).map_or(0, |it| it.gold);
        PartView {
            id,
            name,
            icon,
            plaintext,
            gold,
            owned: owned_item(iid, &owned, assets),
        }
    };
    let make_opt = |o: &BuildOption| -> OptionView {
        let mut vs_sorted = o.vs.clone();
        vs_sorted.sort_by(|a, b| b.weighted.abs().total_cmp(&a.weighted.abs()));
        OptionView {
            parts: o.ids().into_iter().map(make_item).collect(),
            wr: r1(o.wr),
            pr: r1(o.pr),
            games: o.games,
            best: false,
            matchup: r2(o.matchup()),
            has_vs: !o.vs.is_empty(),
            yours: false,
            vs: vs_sorted
                .into_iter()
                .filter(|v| v.weighted.abs() >= 0.05)
                .map(|v| VsOptionView {
                    champ: ChampRef::from_id(v.enemy, champs),
                    lane: v.lane,
                    delta: r2(v.delta),
                    weighted: r2(v.weighted),
                    games: v.games,
                })
                .collect(),
        }
    };

    let cores = b.sections.get("core").map(Vec::as_slice).unwrap_or(&[]);
    let core = cores.iter().max_by_key(|o| {
        let ids = o.ids();
        let own_cnt = ids.iter().filter(|&&i| owned_item(i, &owned, assets)).count();
        let exact_cnt = ids.iter().filter(|i| owned.contains(i)).count();
        (own_cnt, exact_cnt)
    });
    let path: Vec<PartView> = core
        .map(|c| c.ids().into_iter().map(make_item).collect())
        .unwrap_or_default();
    let nxt = path.iter().find(|p| !p.owned).cloned();

    let mut sections = Vec::new();
    let boots = b.sections.get("boots").map(Vec::as_slice).unwrap_or(&[]);
    if !boots.is_empty() && !boots.iter().any(|o| o.ids().iter().all(|i| owned.contains(i))) {
        let mut shown: Vec<&BuildOption> = boots.iter().take(3).collect();
        if let Some(extra_boot) = boots
            .iter()
            .skip(3)
            .find(|o| o.ids().iter().any(|i| owned.contains(i)))
        {
            shown.push(extra_boot);
        }
        let rows = shown
            .into_iter()
            .map(|o| {
                let mut row = make_opt(o);
                for p in &mut row.parts {
                    p.owned = owned.contains(&p.id);
                }
                row.yours = row.parts.iter().all(|p| p.owned);
                row
            })
            .collect();
        sections.push(BuildSectionView {
            key: "boots".into(),
            title: "Boots".into(),
            state: String::new(),
            options: rows,
        });
    }

    let core_ids = core.map(|c| c.ids()).unwrap_or_default();
    let extras = extra_items(&owned, &core_ids, assets);
    let mut unplaced = extras.clone();
    let mut next_marked = nxt.is_some();

    for (n, (key, title)) in [("item3", "3rd item"), ("item4", "4th item"), ("item5", "5th item")]
        .into_iter()
        .enumerate()
    {
        let opts = b.sections.get(key).map(Vec::as_slice).unwrap_or(&[]);
        if n < extras.len() {
            if let Some(mine) = opts
                .iter()
                .find(|o| o.ids().iter().any(|i| unplaced.contains(i)))
            {
                let mids = mine.ids();
                unplaced.retain(|i| !mids.contains(i));
            }
            continue;
        }
        let filtered: Vec<&BuildOption> = opts
            .iter()
            .filter(|o| !o.ids().iter().all(|i| owned_item(*i, &owned, assets)))
            .take(3)
            .collect();
        if !filtered.is_empty() {
            let state = if next_marked { "" } else { "next" };
            next_marked = true;
            sections.push(BuildSectionView {
                key: key.into(),
                title: title.into(),
                state: state.into(),
                options: filtered.into_iter().map(make_opt).collect(),
            });
        }
    }

    let skill_order = b
        .sections
        .get("skill_order")
        .and_then(|s| s.first())
        .map(|o| {
            o.key
                .chars()
                .map(|ch| match ch {
                    '1' => 'Q',
                    '2' => 'W',
                    '3' => 'E',
                    '4' => 'R',
                    _ => '?',
                })
                .collect()
        })
        .unwrap_or_default();

    let keystone = b
        .pages
        .get("pick")
        .and_then(|p| p.primary.first())
        .map(|&rid| rune_node(&rid.to_string(), Some(b), assets, champs, true));

    IngameBuildView {
        champ,
        level: live.level,
        gold: live.gold as i32,
        time_secs: live.game_time as i32,
        empty: None,
        opponents: b
            .vs
            .iter()
            .map(|(e, l, _, _)| (ChampRef::from_id(*e, champs), l.clone()))
            .collect(),
        damage: damage_split(bev, champs),
        core: path,
        next: nxt,
        sections,
        skill_order,
        keystone,
    }
}

fn tier_short(t: &str) -> &str {
    match t {
        "IRON" => "I",
        "BRONZE" => "B",
        "SILVER" => "S",
        "GOLD" => "G",
        "PLATINUM" => "P",
        "EMERALD" => "E",
        "DIAMOND" => "D",
        "MASTER" => "M",
        "GRANDMASTER" => "GM",
        "CHALLENGER" => "C",
        _ => "",
    }
}

fn roman_num(d: &str) -> &str {
    match d {
        "I" => "1",
        "II" => "2",
        "III" => "3",
        "IV" => "4",
        _ => "",
    }
}

fn title_case(s: &str) -> String {
    let mut chars = s.chars();
    match chars.next() {
        None => String::new(),
        Some(f) => f.to_uppercase().collect::<String>() + &chars.as_str().to_lowercase(),
    }
}

pub fn players_view(stats: &[PlayerStats], champs: &Champions) -> Vec<TeamView> {
    let mine = stats
        .iter()
        .find(|s| s.r.as_ref().is_some_and(|r| r.is_me))
        .and_then(|s| s.r.as_ref().map(|r| r.team))
        .unwrap_or(100);
    let lane_idx = |pos: &str| -> usize {
        LANES.iter().position(|&l| l == pos).unwrap_or(LANES.len())
    };

    let mut teams = Vec::new();
    for (side, label) in [("ally", "Your team"), ("enemy", "Enemy team")] {
        let mut group: Vec<&PlayerStats> = stats
            .iter()
            .filter(|s| {
                let is_mine = s.r.as_ref().is_some_and(|r| r.team == mine);
                is_mine == (side == "ally")
            })
            .collect();
        group.sort_by(|a, b| {
            let ra = a.r.as_ref();
            let rb = b.r.as_ref();
            let la = lane_idx(ra.map_or("", |r| r.position.as_str()));
            let lb = lane_idx(rb.map_or("", |r| r.position.as_str()));
            la.cmp(&lb).then_with(|| {
                ra.map_or("", |r| r.name.as_str())
                    .to_lowercase()
                    .cmp(&rb.map_or("", |r| r.name.as_str()).to_lowercase())
            })
        });
        let players = group
            .into_iter()
            .map(|s| {
                let r = s.r.as_ref().expect("player ref");
                let rank = if s.tier.is_empty() {
                    String::new()
                } else {
                    format!("{}{} {} LP", tier_short(&s.tier), roman_num(&s.division), s.lp)
                };
                let season = s.losses.map_or(0, |l| s.wins + l);
                let rank_full = if s.tier.is_empty() {
                    "Unranked".to_string()
                } else {
                    format!("{} {} {} LP", title_case(&s.tier), s.division, s.lp)
                        .replace("  ", " ")
                };
                let recent: String = s
                    .recent
                    .iter()
                    .take(10)
                    .map(|&w| if w { 'W' } else { 'L' })
                    .collect();
                PlayerView {
                    name: r.name.clone(),
                    tag: r.tag.clone(),
                    me: r.is_me,
                    lane: r.position.clone(),
                    champ: (r.champion_id != 0).then(|| ChampRef::from_id(r.champion_id, champs)),
                    rank,
                    tier: s.tier.to_lowercase(),
                    queue: s.queue.clone(),
                    rank_full,
                    wr: (season > 0).then(|| (100.0 * s.wins as f64 / season as f64).round() as i32),
                    season_games: season,
                    wins: s.wins,
                    champ_games: s.champ_games,
                    champ_wr: (s.champ_games > 0)
                        .then(|| (100.0 * s.champ_wins as f64 / s.champ_games as f64).round() as i32),
                    kda: s.kda().map(r1),
                    kills: (s.champ_games > 0).then(|| r1(s.kills as f64 / s.champ_games as f64)),
                    deaths: (s.champ_games > 0).then(|| r1(s.deaths as f64 / s.champ_games as f64)),
                    assists: (s.champ_games > 0).then(|| r1(s.assists as f64 / s.champ_games as f64)),
                    champ_season_games: s.season_games,
                    champ_season_wr: s
                        .season_games
                        .filter(|&g| g > 0)
                        .map(|g| (100.0 * s.season_wins.unwrap_or(0) as f64 / g as f64).round() as i32),
                    games: s.games,
                    recent,
                    mastery: s.mastery,
                    tags: s.tags.clone(),
                    error: s.error,
                }
            })
            .collect();
        teams.push(TeamView {
            side: side.to_string(),
            label: label.to_string(),
            players,
        });
    }
    teams
}

pub fn explore_report_view(
    report: &[BlindReportEntry],
    champs: &Champions,
    role: &str,
    tier: &str,
    days: u32,
) -> ExploreReportView {
    let tier_label = TIERS
        .iter()
        .find(|&&(id, _)| id == tier)
        .map(|&(_, l)| l)
        .unwrap_or(tier)
        .to_string();
    ExploreReportView {
        role: role.to_string(),
        tier: tier.to_string(),
        tier_label,
        days,
        champs: report
            .iter()
            .map(|c| ExploreChampView {
                champ: ChampRef::from_id(c.id, champs),
                score: r2(c.score),
                base: r2(c.base),
                mean: r2(c.mean),
                tail: r2(c.tail),
                coverage: (c.coverage * 100.0).round() as i32,
                opps: c
                    .opps
                    .iter()
                    .map(|o| ExploreOppView {
                        champ: ChampRef::from_id(o.id, champs),
                        pr: r2(o.pr),
                        d: r2(o.d),
                        raw: o.raw.map(r2),
                        games: o.games,
                        tail: o.tail,
                    })
                    .collect(),
            })
            .collect(),
    }
}
