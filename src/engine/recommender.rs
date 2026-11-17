use std::collections::{HashMap, HashSet};
use std::sync::{Arc, RwLock};
use tracing::warn;

use crate::domain::blind_report::{BlindReportEntry, role_report};
use crate::domain::builds::{self, Build};
use crate::domain::champions::Champions;
use crate::domain::config::Config;
use crate::domain::ddragon::Assets;
use crate::domain::lcu::DraftState;
use crate::domain::scoring::{
    Blind, ChampStats, Comfort, LANES, Recommendation, Threat, assign_enemy_lanes, ban_threats,
    infer_enemy_lanes, rank_blind, recommend,
};
use crate::data::ddragon_client::{load_assets, load_champions};
use crate::data::lolalytics::Lolalytics;

pub const DAMAGE_TYPES: &[&str] = &["physical", "magic", "true"];

#[derive(Debug, Clone)]
pub struct Evaluation {
    pub draft: DraftState,
    pub role: String,
    pub recs: Vec<Recommendation>,
    pub lanes: HashMap<i32, String>,
    pub enemy_rates: HashMap<i32, HashMap<String, f64>>,
    pub blind: Vec<Blind>,
}

#[derive(Debug, Clone)]
pub struct BuildEvaluation {
    pub champion_id: i32,
    pub role: String,
    pub build: Option<Build>,
    pub opponents: HashMap<String, i32>,
    pub damage: HashMap<i32, HashMap<String, f64>>,
}

#[derive(Debug, Clone)]
pub struct BanEvaluation {
    pub draft: DraftState,
    pub role: String,
    pub picks: Vec<i32>,
    pub threats: Vec<Threat>,
}

pub struct Recommender {
    pub cfg: RwLock<Config>,
    pub champs: Champions,
    pub assets: Assets,
    pub lola: RwLock<Lolalytics>,
    pub pools: RwLock<HashMap<(String, String), Arc<HashMap<i32, ChampStats>>>>,
    pub stats_cache: RwLock<HashMap<(String, i32, String), Option<Arc<ChampStats>>>>,
    pub pool_locks: tokio::sync::Mutex<HashMap<(String, String), Arc<tokio::sync::Mutex<()>>>>,
}

impl Recommender {
    pub async fn load(cfg: Config) -> Result<Self, String> {
        let champs = load_champions(None).await?;
        let assets = load_assets(&champs.version, None).await;
        let lola = Lolalytics::new(&cfg.tier, cfg.days);
        Ok(Self {
            cfg: RwLock::new(cfg),
            champs,
            assets,
            lola: RwLock::new(lola),
            pools: RwLock::new(HashMap::new()),
            stats_cache: RwLock::new(HashMap::new()),
            pool_locks: tokio::sync::Mutex::new(HashMap::new()),
        })
    }

    pub fn from_parts(cfg: Config, champs: Champions, assets: Assets, lola: Lolalytics) -> Self {
        Self {
            cfg: RwLock::new(cfg),
            champs,
            assets,
            lola: RwLock::new(lola),
            pools: RwLock::new(HashMap::new()),
            stats_cache: RwLock::new(HashMap::new()),
            pool_locks: tokio::sync::Mutex::new(HashMap::new()),
        }
    }

    pub fn role_for(draft: &DraftState) -> Option<String> {
        if LANES.contains(&draft.my_position.as_str()) {
            Some(draft.my_position.clone())
        } else {
            None
        }
    }

    pub fn tier(&self) -> String {
        self.lola.read().unwrap().tier.clone()
    }

    pub fn set_tier(&self, tier: &str) {
        self.lola.write().unwrap().tier = tier.to_string();
    }

    pub fn cached_pool(&self, role: &str, tier: Option<&str>) -> Option<Arc<HashMap<i32, ChampStats>>> {
        let t = tier.map(String::from).unwrap_or_else(|| self.tier());
        self.pools.read().unwrap().get(&(t, role.to_string())).cloned()
    }

    pub fn has_pool(&self, role: &str, tier: Option<&str>) -> bool {
        self.cached_pool(role, tier).is_some()
    }

    pub async fn pool<F>(&self, role: &str, tier: Option<&str>, mut progress: F) -> Arc<HashMap<i32, ChampStats>>
    where
        F: FnMut(String),
    {
        let t = tier.map(String::from).unwrap_or_else(|| self.tier());
        let key = (t.clone(), role.to_string());
        if let Some(p) = self.pools.read().unwrap().get(&key).cloned() {
            return p;
        }
        let mutex = {
            let mut locks = self.pool_locks.lock().await;
            locks
                .entry(key.clone())
                .or_insert_with(|| Arc::new(tokio::sync::Mutex::new(())))
                .clone()
        };
        let _guard = mutex.lock().await;
        if let Some(p) = self.pools.read().unwrap().get(&key).cloned() {
            return p;
        }
        let loaded = Arc::new(self.load_pool(role, &t, &mut progress).await);
        self.pools.write().unwrap().insert(key, Arc::clone(&loaded));
        loaded
    }

    async fn load_pool<F>(&self, role: &str, tier: &str, progress: &mut F) -> HashMap<i32, ChampStats>
    where
        F: FnMut(String),
    {
        let (lola, days) = {
            let l = self.lola.read().unwrap();
            let days = l.days;
            let client = if tier == l.tier { l.clone() } else { l.at_tier(tier) };
            (client, days)
        };
        let ids = self.candidates(role, &lola).await;
        let total = ids.len();
        let label = format!("{role} stats ({tier}, {days} days)");
        progress(format!("Loading {label}: 0/{total}..."));

        let sem = Arc::new(tokio::sync::Semaphore::new(6));
        let mut set = tokio::task::JoinSet::new();
        for cid in ids {
            let slug = self.champs.slug(cid);
            let lola_c = lola.clone();
            let role_s = role.to_string();
            let sem_c = Arc::clone(&sem);
            set.spawn(async move {
                let _p = sem_c.acquire().await;
                (cid, slug.clone(), lola_c.champ_stats(&slug, &role_s).await)
            });
        }

        let mut pool = HashMap::new();
        let mut done = 0usize;
        while let Some(res) = set.join_next().await {
            done += 1;
            progress(format!("Loading {label}: {done}/{total}..."));
            if let Ok((cid, slug, maybe_s)) = res {
                match maybe_s {
                    Some(s) => {
                        pool.insert(s.champion_id, s);
                    }
                    None => {
                        warn!("no lolalytics data for {slug} ({cid}) in {role} at {tier}");
                    }
                }
            }
        }
        pool
    }

    pub async fn candidates(&self, role: &str, lola: &Lolalytics) -> Vec<i32> {
        let min_pr = self.cfg.read().unwrap().min_pick_rate;
        let rows = lola.tier_list(role, None).await;
        let mut ids: Vec<i32> = rows
            .iter()
            .filter(|&(&c, r)| r.pr >= min_pr && self.champs.contains(c))
            .map(|(&c, _)| c)
            .collect();
        ids.sort_by(|a, b| {
            let pa = rows.get(a).map_or(0.0, |r| r.pr);
            let pb = rows.get(b).map_or(0.0, |r| r.pr);
            pb.total_cmp(&pa)
        });
        ids
    }

    pub async fn lane_rates(&self, ids: &[i32]) -> HashMap<i32, HashMap<String, f64>> {
        let lola = self.lola.read().unwrap().clone();
        let mut set = tokio::task::JoinSet::new();
        for &cid in ids {
            let lola_c = lola.clone();
            let slug = self.champs.slug(cid);
            set.spawn(async move { (cid, lola_c.lane_rates(&slug).await) });
        }
        let mut out = HashMap::with_capacity(ids.len());
        while let Some(res) = set.join_next().await {
            if let Ok((cid, rates)) = res {
                out.insert(cid, rates);
            }
        }
        out
    }

    pub async fn evaluate(
        &self,
        draft: &DraftState,
        pickable: Option<&HashSet<i32>>,
        enemy_lanes: Option<&HashMap<i32, String>>,
        comfort: &HashMap<i32, Comfort>,
    ) -> Result<Evaluation, String> {
        let role = Self::role_for(draft).ok_or_else(|| "no assigned role".to_string())?;
        let pool = self.pool(&role, None, |_| {}).await;
        let cfg = self.cfg.read().unwrap().clone();

        let hovers: HashSet<i32> = draft
            .allies
            .iter()
            .filter(|s| s.champion_id != 0 && !s.is_me)
            .map(|s| s.champion_id)
            .collect();
        let mut taken: HashSet<i32> = draft.bans.union(&draft.picked).copied().collect();
        taken.extend(&hovers);

        let candidates: Vec<&ChampStats> = pool
            .iter()
            .filter(|&(&cid, _)| {
                !taken.contains(&cid)
                    && pickable.is_none_or(|p| p.is_empty() || p.contains(&cid))
                    && (!cfg.comfort_only || comfort.contains_key(&cid))
            })
            .map(|(_, s)| s)
            .collect();

        let open_lanes: Vec<&'static str> = LANES.iter().copied().filter(|&l| l != role).collect();
        let others: Vec<_> = draft
            .allies
            .iter()
            .filter(|s| s.champion_id != 0 && !s.is_me)
            .collect();
        let mut allies: HashMap<String, (i32, bool)> = HashMap::new();
        let mut unplaced = Vec::new();
        for s in others {
            if open_lanes.contains(&s.position.as_str()) {
                allies.insert(s.position.clone(), (s.champion_id, s.tentative));
            } else {
                unplaced.push(s);
            }
        }
        if !unplaced.is_empty() {
            let uids: Vec<i32> = unplaced.iter().map(|s| s.champion_id).collect();
            let urates = self.lane_rates(&uids).await;
            let free: Vec<&'static str> = open_lanes
                .iter()
                .copied()
                .filter(|&l| !allies.contains_key(l))
                .collect();
            let guess = infer_enemy_lanes(&uids, &urates, &free);
            for (lane, cid) in guess {
                let tent = unplaced
                    .iter()
                    .find(|s| s.champion_id == cid)
                    .is_some_and(|s| s.tentative);
                allies.insert(lane, (cid, tent));
            }
        }

        let rates = self.lane_rates(&draft.enemies).await;
        let enemies = assign_enemy_lanes(&draft.enemies, &rates, enemy_lanes);
        let weights = cfg.weights_for(&role);
        let recs = recommend(
            &candidates,
            &allies,
            &enemies,
            &weights,
            cfg.k,
            50.0,
            cfg.tentative_factor,
            Some(comfort),
        );

        let mut lanes: HashMap<i32, String> = allies
            .iter()
            .map(|(l, &(c, _))| (c, l.clone()))
            .collect();
        for (l, &c) in &enemies {
            lanes.insert(c, l.clone());
        }

        let mut blind = Vec::new();
        if !enemies.contains_key(&role) {
            let mut blind_taken = taken.clone();
            blind_taken.extend(&draft.enemies);
            blind = self.blind(&role, &candidates, &recs, &blind_taken).await;
        }

        Ok(Evaluation {
            draft: draft.clone(),
            role,
            recs: recs.into_iter().take(cfg.top_n).collect(),
            lanes,
            enemy_rates: rates,
            blind,
        })
    }

    pub async fn blind(
        &self,
        role: &str,
        candidates: &[&ChampStats],
        recs: &[Recommendation],
        taken: &HashSet<i32>,
    ) -> Vec<Blind> {
        let (cfg, lola) = (self.cfg.read().unwrap().clone(), self.lola.read().unwrap().clone());
        let rows = lola.tier_list(role, None).await;
        let opponents: HashMap<i32, f64> = rows
            .into_iter()
            .filter(|&(c, ref r)| r.pr >= cfg.min_pick_rate && !taken.contains(&c))
            .map(|(c, r)| (c, r.pr))
            .collect();
        let by_id: HashMap<i32, &Recommendation> = recs.iter().map(|r| (r.champion_id, r)).collect();
        let items: Vec<(&ChampStats, f64)> = candidates
            .iter()
            .map(|&s| (s, by_id.get(&s.champion_id).map_or(0.0, |r| r.base)))
            .collect();
        let mut out = rank_blind(&items, role, &opponents, cfg.k, &cfg.blind);
        for b in &mut out {
            if let Some(r) = by_id.get(&b.champion_id) {
                b.synergy = r
                    .contributions
                    .iter()
                    .filter(|c| c.label == "with")
                    .map(|c| c.weighted)
                    .sum();
                b.score += b.synergy;
            }
        }
        out.sort_by(|a, b| b.score.total_cmp(&a.score));
        out.truncate(cfg.top_n);
        out
    }

    pub async fn blind_report(&self, role: &str, tier: Option<&str>) -> (Vec<BlindReportEntry>, String, u32) {
        let cfg = self.cfg.read().unwrap().clone();
        let t = tier.map(String::from).unwrap_or_else(|| self.tier());
        let lola = {
            let l = self.lola.read().unwrap();
            if t == l.tier { l.clone() } else { l.at_tier(&t) }
        };
        let pool = self.pool(role, Some(&t), |_| {}).await;
        let pr: HashMap<i32, f64> = lola
            .tier_list(role, None)
            .await
            .into_iter()
            .filter(|(_, r)| r.pr >= cfg.min_pick_rate)
            .map(|(c, r)| (c, r.pr))
            .collect();
        let report = role_report(&pool, &pr, role, &cfg.weights_for(role), cfg.k, &cfg.blind);
        (report, t, cfg.days)
    }

    pub async fn lane_stats(&self, cid: i32, lane: &str) -> Option<Arc<ChampStats>> {
        let tier = self.tier();
        let key = (tier.clone(), cid, lane.to_string());
        if let Some(hit) = self.stats_cache.read().unwrap().get(&key) {
            return hit.clone();
        }
        if let Some(pooled) = self
            .pools
            .read()
            .unwrap()
            .get(&(tier.clone(), lane.to_string()))
            .and_then(|p| p.get(&cid))
            .cloned()
        {
            let arc = Arc::new(pooled);
            self.stats_cache.write().unwrap().insert(key, Some(Arc::clone(&arc)));
            return Some(arc);
        }
        let lola = self.lola.read().unwrap().clone();
        let before = lola.fetch.failures();
        let s = lola.champ_stats(&self.champs.slug(cid), lane).await.map(Arc::new);
        if lola.fetch.failures() == before {
            self.stats_cache.write().unwrap().insert(key, s.clone());
        }
        s
    }

    pub fn cached_lane_stats(&self, cid: i32, lane: &str) -> Option<Option<Arc<ChampStats>>> {
        let tier = self.tier();
        let key = (tier.clone(), cid, lane.to_string());
        if let Some(hit) = self.stats_cache.read().unwrap().get(&key) {
            return Some(hit.clone());
        }
        if let Some(pooled) = self
            .pools
            .read()
            .unwrap()
            .get(&(tier, lane.to_string()))
            .and_then(|p| p.get(&cid))
            .cloned()
        {
            return Some(Some(Arc::new(pooled)));
        }
        None
    }

    pub fn odds_inputs(&self, ev: &Evaluation) -> (HashMap<String, (i32, bool)>, HashMap<String, (i32, bool)>) {
        let mut ours = HashMap::new();
        for (&cid, lane) in &ev.lanes {
            if let Some(s) = ev
                .draft
                .allies
                .iter()
                .find(|s| s.champion_id == cid && !s.is_me)
            {
                ours.insert(lane.clone(), (cid, s.tentative));
            }
        }
        if let Some(me) = ev.draft.allies.iter().find(|s| s.is_me && s.champion_id != 0) {
            ours.insert(ev.role.clone(), (me.champion_id, me.tentative));
        }
        let theirs: HashMap<String, (i32, bool)> = ev
            .lanes
            .iter()
            .filter(|&(&cid, _)| ev.draft.enemies.contains(&cid))
            .map(|(&cid, lane)| (lane.clone(), (cid, false)))
            .collect();
        (ours, theirs)
    }

    pub async fn bans(
        &self,
        draft: &DraftState,
        pickable: Option<&HashSet<i32>>,
        comfort: &HashMap<i32, Comfort>,
    ) -> Result<BanEvaluation, String> {
        let role = Self::role_for(draft).ok_or_else(|| "no assigned role".to_string())?;
        let ev = self.evaluate(draft, pickable, None, comfort).await?;
        let pool = self.pool(&role, None, |_| {}).await;
        let cfg = self.cfg.read().unwrap().clone();
        let picks: Vec<&ChampStats> = ev
            .recs
            .iter()
            .take(cfg.ban_vs_top)
            .filter_map(|r| pool.get(&r.champion_id))
            .collect();

        let mut exclude: HashSet<i32> = draft.bans.union(&draft.picked).copied().collect();
        for s in &draft.allies {
            if s.champion_id != 0 {
                exclude.insert(s.champion_id);
            }
        }
        for r in ev.recs.iter().take(3) {
            exclude.insert(r.champion_id);
        }

        let lola = self.lola.read().unwrap().clone();
        let (r0, r1, r2, r3, r4) = tokio::join!(
            lola.tier_list(LANES[0], None),
            lola.tier_list(LANES[1], None),
            lola.tier_list(LANES[2], None),
            lola.tier_list(LANES[3], None),
            lola.tier_list(LANES[4], None),
        );
        let rows = HashMap::from([
            (LANES[0].to_string(), r0),
            (LANES[1].to_string(), r1),
            (LANES[2].to_string(), r2),
            (LANES[3].to_string(), r3),
            (LANES[4].to_string(), r4),
        ]);

        let hovering: HashSet<i32> = draft
            .allies
            .iter()
            .filter(|s| s.champion_id != 0 && !s.is_me)
            .map(|s| s.champion_id)
            .collect();
        let mut ally_stats = Vec::new();
        for (&cid, lane) in &ev.lanes {
            if hovering.contains(&cid)
                && LANES.contains(&lane.as_str())
                && let Some(st) = self.lane_stats(cid, lane).await
            {
                ally_stats.push((st, cfg.weights_for(lane)));
            }
        }
        let ally_refs: Vec<(&ChampStats, _)> = ally_stats
            .iter()
            .map(|(st, w)| (st.as_ref(), w.clone()))
            .collect();

        let mut threats: Vec<Threat> = ban_threats(
            &picks,
            &rows,
            &cfg.weights_for(&role),
            cfg.k,
            &exclude,
            0.5,
            &ally_refs,
        )
        .into_iter()
        .filter(|t| self.champs.contains(t.champion_id))
        .collect();
        threats.truncate(cfg.ban_count);

        Ok(BanEvaluation {
            draft: draft.clone(),
            role,
            picks: picks.iter().map(|p| p.champion_id).collect(),
            threats,
        })
    }

    pub async fn build(&self, champion_id: i32, role: &str, enemies: &HashMap<String, i32>) -> BuildEvaluation {
        let (cfg, lola) = (self.cfg.read().unwrap().clone(), self.lola.read().unwrap().clone());
        let slug = self.champs.slug(champion_id);
        let raw_base = lola.build_full(&slug, role).await;
        let mut base = builds::parse_build(&raw_base);

        let weights = cfg.build_weights_for(role);
        let opponents: HashMap<String, i32> = enemies
            .iter()
            .filter(|(l, _)| weights.contains_key(l.as_str()))
            .map(|(l, &c)| (l.clone(), c))
            .collect();

        if let Some(ref mut b) = base {
            let mut set = tokio::task::JoinSet::new();
            for (lane, &cid) in &opponents {
                let lola_c = lola.clone();
                let slug_c = slug.clone();
                let role_c = role.to_string();
                let vs_slug = self.champs.slug(cid);
                let lane_c = lane.clone();
                set.spawn(async move {
                    let val = lola_c.matchup_build(&slug_c, &role_c, &vs_slug, &lane_c).await;
                    (lane_c, cid, val)
                });
            }
            while let Some(res) = set.join_next().await {
                if let Ok((lane, cid, val)) = res
                    && let Some(vs) = builds::parse_build(&val)
                {
                    builds::add_matchup(
                        b,
                        &vs,
                        cid,
                        &lane,
                        weights[&lane],
                        cfg.build_k,
                        cfg.build_min_games,
                    );
                }
            }
        }

        let damage = self.enemy_damage(enemies).await;
        BuildEvaluation {
            champion_id,
            role: role.to_string(),
            build: base,
            opponents,
            damage,
        }
    }

    pub async fn enemy_damage(&self, enemies: &HashMap<String, i32>) -> HashMap<i32, HashMap<String, f64>> {
        let lola = self.lola.read().unwrap().clone();
        let mut set = tokio::task::JoinSet::new();
        for (lane, &cid) in enemies {
            if !LANES.contains(&lane.as_str()) {
                continue;
            }
            let lola_c = lola.clone();
            let slug = self.champs.slug(cid);
            let lane_c = lane.clone();
            set.spawn(async move { (cid, lola_c.build_full(&slug, &lane_c).await) });
        }
        let mut out = HashMap::new();
        while let Some(res) = set.join_next().await {
            if let Ok((cid, val)) = res
                && let Some(dmg) = val
                    .get("header")
                    .and_then(|h| h.get("damage"))
                    .and_then(|d| d.as_object())
            {
                let mut split = HashMap::new();
                let mut sum = 0.0;
                for &t in DAMAGE_TYPES {
                    let v = dmg.get(t).and_then(|x| x.as_f64()).unwrap_or(0.0);
                    sum += v;
                    split.insert(t.to_string(), v);
                }
                if sum > 0.0 {
                    out.insert(cid, split);
                }
            }
        }
        out
    }
}
