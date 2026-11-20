use serde_json::json;
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use tokio::sync::watch;
use tracing::{info, warn};

use crate::domain::config::{Config, StreamConfig, data_dir, load_ui_settings, save_ui_settings};
use crate::domain::lcu::LcuCreds;
use crate::domain::live::{LiveGame, parse_live};
use crate::domain::players::PlayerStats;
use crate::domain::scoring::{ChampStats, Comfort, DraftOdds, LANES, draft_odds};
use crate::domain::state::{Event, Session, Stage};
use crate::data::lcu::{LcuClient, read_lockfile};
use crate::data::lcu_data::{Mastery, fetch_mastery, sync_history};
use crate::data::live::{GameRecorder, LiveClient};
use crate::data::opgg::OpGG;
use crate::data::players::{PlayerClient, game_refs, load_all, region};
use crate::data::store::Store;
use crate::data::stream::{LiveStreamer, StreamDraftPayload};

use super::recommender::{BuildEvaluation, Evaluation, Recommender};
use super::views::{
    ClientScreen, ExploreReportView, GameScreen, TIERS, bans_view, build_view,
    explore_report_view, header_view, ingame_build_view, picks_view, players_view, stage_message,
};

pub const SYNC_EVERY: Duration = Duration::from_secs(20);
pub const LIVE_POLL: Duration = Duration::from_secs(2);
pub const PLAYERS_RETRY: Duration = Duration::from_secs(30);
pub const STATS_RETRY: Duration = Duration::from_secs(30);

#[derive(Debug, Clone)]
pub enum ScreenUpdate {
    Client(ClientScreen),
    Game(GameScreen),
}

impl ScreenUpdate {
    pub fn is_game(&self) -> bool {
        matches!(self, ScreenUpdate::Game(_))
    }
}

fn current_hms() -> String {
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let s = secs % 60;
    let m = (secs / 60) % 60;
    let h = (secs / 3600) % 24;
    format!("{h:02}:{m:02}:{s:02}")
}

struct InnerState {
    cfg: Config,
    session: Session,
    user_hidden: bool,
    idle_message: String,
    notice: Option<(String, String)>,
    draft_id: Option<i64>,
    last_eval: Option<Evaluation>,
    last_odds: Option<DraftOdds>,
    stats_pending: HashSet<(i32, String)>,
    stats_retry_at: HashMap<(i32, String), Instant>,
    build_cache: Option<((i32, String, String, Vec<(String, i32)>), BuildEvaluation)>,
    build_pending: HashSet<(i32, String, String, Vec<(String, i32)>)>,
    pool_loading: HashSet<(String, String)>,
    mastery: HashMap<i32, Mastery>,
    synced_at: Option<Instant>,
    preload_msg: String,
    preloaded: HashSet<(String, String)>,
    ingame_shown: bool,
    players: Option<(i64, Vec<PlayerStats>)>,
    players_loading: bool,
    players_retry_at: Option<Instant>,
    live: Option<LiveGame>,
    last_role: String,
    recorder: GameRecorder,
    live_url: Option<String>,
}

pub struct Controller {
    pub store: Option<Arc<Store>>,
    pub rec: Mutex<Option<Arc<Recommender>>>,
    pub streamer: Arc<LiveStreamer>,
    state: Mutex<InnerState>,
    live_client: tokio::sync::Mutex<Option<LiveClient>>,
    screen_tx: watch::Sender<ScreenUpdate>,
    render_seq: AtomicU64,
    pub on_repaint: Mutex<Option<Box<dyn Fn() + Send + Sync>>>,
    pub lockfile_watch: Arc<tokio::sync::RwLock<String>>,
}

impl Controller {
    pub fn new(cfg: Config, store: Option<Arc<Store>>) -> (Arc<Self>, watch::Receiver<ScreenUpdate>) {
        let lockfile = Arc::new(tokio::sync::RwLock::new(cfg.lockfile.clone()));
        let init_screen = ScreenUpdate::Client(ClientScreen {
            header: header_view(None, &cfg.tier, cfg.days),
            status: "waiting".into(),
            message: "Waiting for the League client...".into(),
            detail: String::new(),
            stage: Stage::Idle,
            bans: None,
            picks: None,
            build: None,
            explore: false,
            updated_hms: current_hms(),
            live_url: None,
            auto_copy_live_url: cfg.stream.auto_copy_link,
        });
        let (screen_tx, screen_rx) = watch::channel(init_screen);
        let rec_every = cfg.record_every;
        let ctl = Arc::new(Self {
            store,
            rec: Mutex::new(None),
            streamer: Arc::new(LiveStreamer::new()),
            state: Mutex::new(InnerState {
                cfg,
                session: Session::default(),
                user_hidden: false,
                idle_message: "Waiting for the League client...".into(),
                notice: None,
                draft_id: None,
                last_eval: None,
                last_odds: None,
                stats_pending: HashSet::new(),
                stats_retry_at: HashMap::new(),
                build_cache: None,
                build_pending: HashSet::new(),
                pool_loading: HashSet::new(),
                mastery: HashMap::new(),
                synced_at: None,
                preload_msg: String::new(),
                preloaded: HashSet::new(),
                ingame_shown: false,
                players: None,
                players_loading: false,
                players_retry_at: None,
                live: None,
                last_role: String::new(),
                recorder: GameRecorder::new(data_dir().join("recordings"), rec_every),
                live_url: None,
            }),
            live_client: tokio::sync::Mutex::new(None),
            screen_tx,
            render_seq: AtomicU64::new(0),
            on_repaint: Mutex::new(None),
            lockfile_watch: lockfile,
        });
        (ctl, screen_rx)
    }

    pub fn set_repaint_hook(&self, hook: impl Fn() + Send + Sync + 'static) {
        *self.on_repaint.lock().unwrap() = Some(Box::new(hook));
    }

    fn emit(&self, update: ScreenUpdate) {
        let _ = self.screen_tx.send(update);
        if let Some(cb) = self.on_repaint.lock().unwrap().as_ref() {
            cb();
        }
    }

    pub fn user_hidden(&self) -> bool {
        self.state.lock().unwrap().user_hidden
    }

    pub fn set_user_hidden(&self, hidden: bool) {
        self.state.lock().unwrap().user_hidden = hidden;
    }

    pub fn ingame_screen_active(&self) -> bool {
        let st = self.state.lock().unwrap();
        st.session.stage() == Stage::Ingame && st.ingame_shown
    }

    pub fn set_idle_message(&self, msg: impl Into<String>) {
        self.state.lock().unwrap().idle_message = msg.into();
    }

    pub fn set_simulate_data(
        &self,
        players: Option<(i64, Vec<PlayerStats>)>,
        live: Option<LiveGame>,
        ingame: bool,
    ) {
        let mut st = self.state.lock().unwrap();
        if let Some(p) = players {
            st.players = Some(p);
        }
        if let Some(l) = live {
            st.live = Some(l);
        }
        if ingame {
            st.session.phase = Some("InProgress".into());
            st.ingame_shown = true;
        }
    }

    pub async fn boot(self: &Arc<Self>) -> Result<(), String> {
        self.show_status("Loading champion data...", "waiting");
        let cfg = self.state.lock().unwrap().cfg.clone();
        let rec = Recommender::load(cfg).await?;
        let saved_tier = load_ui_settings().tier;
        if let Some(t) = saved_tier
            && TIERS.iter().any(|&(id, _)| id == t)
        {
            rec.set_tier(&t);
        } else {
            let cur = rec.tier();
            if !TIERS.iter().any(|&(id, _)| id == cur) {
                rec.set_tier(TIERS[0].0);
            }
        }
        *self.rec.lock().unwrap() = Some(Arc::new(rec));
        self.render(self.clone()).await;
        Ok(())
    }

    pub fn get_rec(&self) -> Option<Arc<Recommender>> {
        self.rec.lock().unwrap().clone()
    }

    pub fn tier(&self) -> String {
        self.get_rec()
            .map(|r| r.tier())
            .unwrap_or_else(|| self.state.lock().unwrap().cfg.tier.clone())
    }

    pub fn main_role(&self) -> Option<String> {
        if let Some(store) = &self.store {
            let top = store.top_roles(1);
            if let Some(r) = top.into_iter().next() {
                return Some(r);
            }
        }
        let st = self.state.lock().unwrap();
        st.last_eval
            .as_ref()
            .map(|e| e.role.clone())
            .or_else(|| (!st.last_role.is_empty()).then(|| st.last_role.clone()))
    }

    pub async fn explore_report(&self, role: &str, tier: &str) -> Result<ExploreReportView, String> {
        let rec = self.get_rec().ok_or_else(|| "stats not ready".to_string())?;
        let (report, t, days) = rec.blind_report(role, Some(tier)).await;
        Ok(explore_report_view(&report, &rec.champs, role, &t, days))
    }

    fn comfort_for(&self, role: &str) -> HashMap<i32, Comfort> {
        let (half_life, mastery) = {
            let st = self.state.lock().unwrap();
            let hl = st.cfg.weights_for(role).comfort.half_life;
            (hl, st.mastery.clone())
        };
        let mut out: HashMap<i32, Comfort> = mastery
            .into_iter()
            .map(|(cid, m)| {
                (
                    cid,
                    Comfort {
                        mastery: m.points,
                        ..Default::default()
                    },
                )
            })
            .collect();
        if let Some(store) = &self.store {
            for (cid, (games, wins, eff_games, eff_wins)) in store.champ_record(Some(role), half_life) {
                let m = out.get(&cid).map_or(0, |c| c.mastery);
                out.insert(
                    cid,
                    Comfort {
                        games,
                        wins,
                        mastery: m,
                        eff_games: Some(eff_games),
                        eff_wins: Some(eff_wins),
                    },
                );
            }
        }
        out
    }

    pub fn show_status(&self, message: &str, kind: &str) {
        let rec = self.get_rec();
        let st = self.state.lock().unwrap();
        let stage = st.session.stage();
        let tier = rec.as_ref().map(|r| r.tier()).unwrap_or_else(|| st.cfg.tier.clone());
        let header = header_view(None, &tier, st.cfg.days);
        if stage == Stage::Ingame && st.ingame_shown {
            let (teams, players_note) = Self::game_players_snapshot(&st, rec.as_deref());
            self.emit(ScreenUpdate::Game(GameScreen {
                header,
                status: kind.to_string(),
                build: None,
                build_note: message.to_string(),
                teams,
                players_note,
            }));
            return;
        }
        let explore = !stage.is_draft() && rec.is_some();
        self.emit(ScreenUpdate::Client(ClientScreen {
            header,
            status: kind.to_string(),
            message: message.to_string(),
            detail: String::new(),
            stage,
            bans: None,
            picks: None,
            build: None,
            explore,
            updated_hms: current_hms(),
            live_url: st.live_url.clone(),
            auto_copy_live_url: st.cfg.stream.auto_copy_link,
        }));
    }

    fn game_players_snapshot(
        st: &InnerState,
        rec: Option<&Recommender>,
    ) -> (Option<Vec<super::views::TeamView>>, String) {
        let Some((_, list)) = &st.players else {
            let msg = if st.players_loading {
                "Loading players..."
            } else {
                "Player stats unavailable (League client not reachable)."
            };
            return (None, msg.to_string());
        };
        let Some(rec) = rec else {
            return (None, "No player data for this game.".to_string());
        };
        if list.is_empty() {
            return (None, "No player data for this game.".to_string());
        }
        (Some(players_view(list, &rec.champs)), String::new())
    }

    /// Non-blocking render: computes instantly from whatever pools/stats/builds are already in
    /// memory and spawns background tasks for anything missing (never holding a lock over HTTP!).
    pub async fn render(&self, self_arc: Arc<Self>) {
        let seq = self.render_seq.fetch_add(1, Ordering::SeqCst) + 1;
        let rec = self.get_rec();

        // Snapshot everything under a tiny synchronous lock, then drop it immediately.
        let snap = {
            let st = self.state.lock().unwrap();
            if let Some((msg, kind)) = &st.notice {
                let msg = msg.clone();
                let kind = kind.clone();
                drop(st);
                self.show_status(&msg, &kind);
                return;
            }
            let stage = st.session.stage();
            let tier = rec.as_ref().map(|r| r.tier()).unwrap_or_else(|| st.cfg.tier.clone());
            let days = st.cfg.days;

            if stage == Stage::Ingame && st.ingame_shown {
                let (teams, mut players_note) = Self::game_players_snapshot(&st, rec.as_deref());
                let need_players = st.players.is_none()
                    && !st.players_loading
                    && st.players_retry_at.is_none_or(|t| Instant::now() >= t);
                let live = st.live.clone();
                let last_role = st.last_role.clone();
                let build_cache = st.build_cache.clone();
                let _cfg = st.cfg.clone();
                drop(st);

                if need_players {
                    players_note = "Loading players...".to_string();
                    self_arc.clone().spawn_players();
                }

                let mut build = None;
                let mut build_note = String::new();
                let mut kind = "active".to_string();
                let mut role_out = None;

                if let (Some(g), Some(rec)) = (&live, &rec) {
                    let cid = rec.champs.id_from_slug(&g.champion);
                    let role = if !g.role.is_empty() {
                        g.role.clone()
                    } else {
                        last_role
                    };
                    if let (Some(cid), true) = (cid, !role.is_empty()) {
                        role_out = Some(role.clone());
                        let enemies: HashMap<String, i32> = g
                            .enemies
                            .iter()
                            .filter_map(|(lane, name)| rec.champs.id_from_slug(name).map(|id| (lane.clone(), id)))
                            .collect();
                        let mut sorted_e: Vec<(String, i32)> = enemies.iter().map(|(l, &c)| (l.clone(), c)).collect();
                        sorted_e.sort();
                        let key = (cid, role.clone(), tier.clone(), sorted_e);
                        if let Some((ck, bev)) = &build_cache
                            && *ck == key
                        {
                            build = Some(ingame_build_view(g, Some(bev), &rec.champs, &rec.assets));
                        } else {
                            build_note = "Loading build...".to_string();
                            kind = "waiting".to_string();
                            self_arc.clone().spawn_build(key, enemies);
                        }
                    } else {
                        build_note = "No build: champion or role unknown in this game.".to_string();
                    }
                } else {
                    build_note = "Waiting for game data...".to_string();
                }

                self.emit(ScreenUpdate::Game(GameScreen {
                    header: header_view(role_out.as_deref(), &tier, days),
                    status: kind,
                    build,
                    build_note,
                    teams,
                    players_note,
                }));
                return;
            }

            if !stage.is_draft() || st.session.draft.is_none() {
                let msg = if stage == Stage::Idle {
                    st.idle_message.clone()
                } else {
                    stage_message(stage).to_string()
                };
                let detail = if stage.is_pre_draft() {
                    st.preload_msg.clone()
                } else {
                    String::new()
                };
                let explore = rec.is_some();
                let auto_copy_live_url = st.cfg.stream.auto_copy_link;
                drop(st);
                self.emit(ScreenUpdate::Client(ClientScreen {
                    header: header_view(None, &tier, days),
                    status: "waiting".into(),
                    message: msg,
                    detail,
                    stage,
                    bans: None,
                    picks: None,
                    build: None,
                    explore,
                    updated_hms: current_hms(),
                    live_url: None,
                    auto_copy_live_url,
                }));
                return;
            }

            let draft = st.session.draft.clone().unwrap();
            let frozen = st.session.frozen.clone();
            let pickable = st.session.pickable.clone();
            let enemy_lanes = st.session.enemy_lanes.clone();
            let cfg = st.cfg.clone();
            let build_cache = st.build_cache.clone();
            let live_url = st.live_url.clone();
            (stage, tier, days, draft, frozen, pickable, enemy_lanes, cfg, build_cache, live_url)
        };

        let (stage, tier, days, draft, frozen, pickable, enemy_lanes, cfg, build_cache, live_url) = snap;
        let auto_copy_live_url = cfg.stream.auto_copy_link;
        let Some(rec) = rec else {
            return;
        };
        let Some(role) = Recommender::role_for(&draft) else {
            self.emit(ScreenUpdate::Client(ClientScreen {
                header: header_view(None, &tier, days),
                status: "waiting".into(),
                message: "No assigned role in this queue - recommendations need a position (draft or ranked).".into(),
                detail: String::new(),
                stage,
                bans: None,
                picks: None,
                build: None,
                explore: false,
                updated_hms: current_hms(),
                live_url,
                auto_copy_live_url,
            }));
            return;
        };

        // Check if the role's pool is already cached in memory; if not, kick off async load and show progress!
        if !rec.has_pool(&role, Some(&tier)) {
            self_arc.clone().spawn_pool(role.clone(), tier.clone());
            self.emit(ScreenUpdate::Client(ClientScreen {
                header: header_view(Some(&role), &tier, days),
                status: "waiting".into(),
                message: format!("Loading {role} stats ({tier})..."),
                detail: String::new(),
                stage,
                bans: None,
                picks: None,
                build: None,
                explore: false,
                updated_hms: current_hms(),
                live_url,
                auto_copy_live_url,
            }));
            return;
        }

        let comfort = self.comfort_for(&role);
        let Ok(mut ev) = rec
            .evaluate(&draft, pickable.as_ref(), Some(&enemy_lanes), &comfort)
            .await
        else {
            self.show_status("Error scoring draft", "error");
            return;
        };
        if let Some(fr) = &frozen
            && let Ok(fr_ev) = rec
                .evaluate(fr, pickable.as_ref(), Some(&enemy_lanes), &comfort)
                .await
        {
            ev.recs = fr_ev.recs;
        }

        let odds = self.compute_odds(&self_arc, &rec, &ev, &cfg);
        let picks = picks_view(&ev, &rec.champs, &enemy_lanes, odds.as_ref());

        let bans = if stage == Stage::Ban {
            rec.bans(&draft, pickable.as_ref(), &comfort)
                .await
                .ok()
                .map(|b| bans_view(&b, &rec.champs))
        } else {
            None
        };

        let build = if stage == Stage::Locked {
            let enemies: HashMap<String, i32> = ev
                .lanes
                .iter()
                .filter(|&(&cid, _)| ev.draft.enemies.contains(&cid))
                .map(|(&cid, lane)| (lane.clone(), cid))
                .collect();
            let mut sorted_e: Vec<(String, i32)> = enemies.iter().map(|(l, &c)| (l.clone(), c)).collect();
            sorted_e.sort();
            let key = (ev.draft.my_locked, ev.role.clone(), tier.clone(), sorted_e);
            if let Some((ck, bev)) = &build_cache
                && *ck == key
            {
                Some(build_view(bev, &rec.champs, &rec.assets))
            } else {
                self_arc.clone().spawn_build(key, enemies);
                None
            }
        } else {
            None
        };

        if self.render_seq.load(Ordering::SeqCst) != seq {
            return;
        }
        {
            let mut st = self.state.lock().unwrap();
            st.last_eval = Some(ev);
            st.last_odds = odds;
        }

        self.emit(ScreenUpdate::Client(ClientScreen {
            header: header_view(Some(&role), &tier, days),
            status: "active".into(),
            message: String::new(),
            detail: String::new(),
            stage,
            bans,
            picks: Some(picks),
            build,
            explore: false,
            updated_hms: current_hms(),
            live_url,
            auto_copy_live_url,
        }));
    }

    fn compute_odds(
        &self,
        self_arc: &Arc<Self>,
        rec: &Arc<Recommender>,
        ev: &Evaluation,
        cfg: &Config,
    ) -> Option<DraftOdds> {
        let (ours_ids, theirs_ids) = rec.odds_inputs(ev);
        if ours_ids.is_empty() && theirs_ids.is_empty() {
            return None;
        }
        let mut missing = Vec::new();
        let mut ours_owned: HashMap<String, (Arc<ChampStats>, bool)> = HashMap::new();
        let mut theirs_owned: HashMap<String, (Arc<ChampStats>, bool)> = HashMap::new();

        for (lane, (cid, tent)) in ours_ids {
            match rec.cached_lane_stats(cid, &lane) {
                None => missing.push((cid, lane)),
                Some(Some(st)) => {
                    ours_owned.insert(lane, (st, tent));
                }
                Some(None) => {}
            }
        }
        for (lane, (cid, tent)) in theirs_ids {
            match rec.cached_lane_stats(cid, &lane) {
                None => missing.push((cid, lane)),
                Some(Some(st)) => {
                    theirs_owned.insert(lane, (st, tent));
                }
                Some(None) => {}
            }
        }

        let pending = missing.len();
        if !missing.is_empty() {
            self_arc.clone().spawn_odds_stats(missing);
        }

        let ours_ref: HashMap<String, (&ChampStats, bool)> = ours_owned
            .iter()
            .map(|(l, (s, t))| (l.clone(), (s.as_ref(), *t)))
            .collect();
        let theirs_ref: HashMap<String, (&ChampStats, bool)> = theirs_owned
            .iter()
            .map(|(l, (s, t))| (l.clone(), (s.as_ref(), *t)))
            .collect();

        let mut odds = draft_odds(&ours_ref, &theirs_ref, cfg.k, &cfg.odds, cfg.tentative_factor);
        odds.known = ours_ref.len() + theirs_ref.len();
        odds.pending = pending;
        Some(odds)
    }

    fn spawn_pool(self: Arc<Self>, role: String, tier: String) {
        {
            let mut st = self.state.lock().unwrap();
            if !st.pool_loading.insert((tier.clone(), role.clone())) {
                return;
            }
        }
        tokio::spawn(async move {
            if let Some(rec) = self.get_rec() {
                let ctl_c = Arc::clone(&self);
                let _ = rec
                    .pool(&role, Some(&tier), move |msg| {
                        let st = ctl_c.state.lock().unwrap();
                        if st.session.stage().is_draft() {
                            drop(st);
                            ctl_c.show_status(&msg, "waiting");
                        }
                    })
                    .await;
            }
            self.state.lock().unwrap().pool_loading.remove(&(tier, role));
            self.render(Arc::clone(&self)).await;
        });
    }

    fn spawn_odds_stats(self: Arc<Self>, missing: Vec<(i32, String)>) {
        let need: Vec<(i32, String)> = {
            let mut st = self.state.lock().unwrap();
            let now = Instant::now();
            let filtered: Vec<_> = missing
                .into_iter()
                .filter(|n| {
                    !st.stats_pending.contains(n)
                        && st.stats_retry_at.get(n).is_none_or(|&t| t <= now)
                })
                .collect();
            for n in &filtered {
                st.stats_pending.insert(n.clone());
            }
            filtered
        };
        if need.is_empty() {
            return;
        }
        tokio::spawn(async move {
            if let Some(rec) = self.get_rec() {
                let mut set = tokio::task::JoinSet::new();
                for (cid, lane) in &need {
                    let r = Arc::clone(&rec);
                    let l = lane.clone();
                    let c = *cid;
                    set.spawn(async move { r.lane_stats(c, &l).await });
                }
                while set.join_next().await.is_some() {}

                let retry_at = Instant::now() + STATS_RETRY;
                let in_draft = {
                    let mut st = self.state.lock().unwrap();
                    for n in need {
                        if rec.cached_lane_stats(n.0, &n.1).is_none() {
                            st.stats_retry_at.insert(n.clone(), retry_at);
                        } else {
                            st.stats_retry_at.remove(&n);
                        }
                        st.stats_pending.remove(&n);
                    }
                    st.session.stage().is_draft()
                };
                if in_draft {
                    self.render(Arc::clone(&self)).await;
                }
            }
        });
    }

    fn spawn_build(
        self: Arc<Self>,
        key: (i32, String, String, Vec<(String, i32)>),
        enemies: HashMap<String, i32>,
    ) {
        {
            let mut st = self.state.lock().unwrap();
            if !st.build_pending.insert(key.clone()) {
                return;
            }
        }
        tokio::spawn(async move {
            if let Some(rec) = self.get_rec() {
                let bev = rec.build(key.0, &key.1, &enemies).await;
                let mut st = self.state.lock().unwrap();
                st.build_cache = Some((key.clone(), bev));
                st.build_pending.remove(&key);
            } else {
                self.state.lock().unwrap().build_pending.remove(&key);
            }
            self.render(Arc::clone(&self)).await;
        });
    }

    fn spawn_players(self: Arc<Self>) {
        let lockfile = {
            let mut st = self.state.lock().unwrap();
            if st.players_loading || st.players.is_some() {
                return;
            }
            if st.players_retry_at.is_some_and(|t| Instant::now() < t) {
                return;
            }
            st.players_loading = true;
            st.cfg.lockfile.clone()
        };
        tokio::spawn(async move {
            let result = self.load_players_async(&lockfile).await;
            let shown = {
                let mut st = self.state.lock().unwrap();
                st.players_loading = false;
                if st.session.stage() != Stage::Ingame {
                    return;
                }
                if result.is_none() {
                    st.players_retry_at = Some(Instant::now() + PLAYERS_RETRY);
                }
                st.players = result;
                st.ingame_shown
            };
            if shown {
                self.render(Arc::clone(&self)).await;
            }
        });
    }

    async fn load_players_async(&self, lockfile: &str) -> Option<(i64, Vec<PlayerStats>)> {
        let creds: LcuCreds = read_lockfile(Some(lockfile)).ok()?;
        let lcu = LcuClient::new(&creds);
        let client = Arc::new(PlayerClient::Live(lcu));

        let live_raw = {
            let mut lc = self.live_client.lock().await;
            if lc.is_none() {
                *lc = Some(LiveClient::new(Duration::from_millis(1500)));
            }
            lc.as_ref().unwrap().raw().await
        };
        let rec = self.get_rec();
        let (gid, refs) = game_refs(&client, live_raw.as_ref(), |name| {
            rec.as_ref().and_then(|r| r.champs.id_from_slug(name))
        })
        .await
        .ok()?;
        if refs.is_empty() {
            return None;
        }
        let use_opgg = self.state.lock().unwrap().cfg.opgg;
        let opgg = use_opgg.then(|| Arc::new(OpGG::new()));
        let reg = region(&client).await;
        let stats = load_all(client, refs, opgg, reg).await;
        info!("player stats: {} players ({} partial)", stats.len(), stats.iter().filter(|s| s.error).count());
        if stats.is_empty() { None } else { Some((gid, stats)) }
    }

    pub async fn on_phase(self: &Arc<Self>, phase: &str) {
        let events = {
            let mut st = self.state.lock().unwrap();
            if st.session.phase.as_deref() != Some(phase) {
                info!("gameflow phase: {phase}");
            }
            st.session.on_phase(phase)
        };
        self.handle(events, None).await;
    }

    pub async fn on_session(
        self: &Arc<Self>,
        session: Option<&serde_json::Value>,
        pickable: Option<HashSet<i32>>,
    ) {
        let (events, draft_id) = {
            let mut st = self.state.lock().unwrap();
            let evs = st.session.on_session(session, pickable);
            (evs, st.draft_id)
        };
        if let (Some(ses), Some(did), Some(store)) = (session, draft_id, &self.store) {
            if let Err(e) = store.snapshot(did, ses) {
                warn!("store snapshot failed: {e}");
            }
        }
        self.handle(events, session).await;
    }

    async fn handle(self: &Arc<Self>, events: Vec<Event>, session: Option<&serde_json::Value>) {
        if events.is_empty() {
            return;
        }
        let (
            stage,
            do_preload,
            do_sync,
            sync_delay,
            draft_started,
            locked_in,
            draft_ended,
            stream_start,
            stream_end,
        ) = {
            let mut st = self.state.lock().unwrap();
            let stage = st.session.stage();
            let has_phase = events.contains(&Event::Phase);
            let do_preload = has_phase
                && (stage.is_pre_draft() || matches!(stage, Stage::Idle | Stage::Postgame));
            let do_sync = has_phase && matches!(stage, Stage::Idle | Stage::Lobby | Stage::Postgame);
            let sync_delay = if stage == Stage::Postgame { 5.0 } else { 0.0 };
            let draft_started = events.contains(&Event::DraftStarted);
            let draft_ended = events.contains(&Event::DraftEnded);
            if draft_started {
                st.user_hidden = false;
                st.preload_msg.clear();
                st.last_odds = None;
                st.live_url = None;
            }
            if draft_ended {
                st.live_url = None;
            }
            let stream_ready = st.cfg.stream.enabled && !st.cfg.stream.endpoint.trim().is_empty();
            let stream_start = if draft_started && stream_ready {
                Some(st.cfg.stream.clone())
            } else {
                None
            };
            let stream_end = if draft_ended && st.cfg.stream.enabled {
                Some(st.cfg.stream.clone())
            } else {
                None
            };
            if has_phase {
                if stage != Stage::Ingame {
                    st.players = None;
                    st.players_retry_at = None;
                }
            }
            (
                stage,
                do_preload,
                do_sync,
                sync_delay,
                draft_started,
                events.contains(&Event::LockedIn),
                draft_ended,
                stream_start,
                stream_end,
            )
        };

        if let Some(stream_cfg) = stream_start {
            let tier = self.tier();
            self.spawn_stream_start(stream_cfg, tier);
        }

        if draft_started && let Some(store) = &self.store {
            let tier = self.tier();
            let role = {
                let st = self.state.lock().unwrap();
                st.session
                    .draft
                    .as_ref()
                    .and_then(|d| Recommender::role_for(d))
            };
            if let Ok(did) = store.start_draft(role.as_deref(), &tier) {
                self.state.lock().unwrap().draft_id = Some(did);
                if let Some(ses) = session {
                    let _ = store.snapshot(did, ses);
                }
            }
        }

        if events.contains(&Event::Phase) && stage == Stage::Ingame {
            self.clone().spawn_players();
        }
        if do_preload {
            self.clone().start_preload();
        }
        if do_sync {
            self.clone().start_sync(sync_delay);
        }

        self.render(Arc::clone(self)).await;

        if events.contains(&Event::DraftChanged) {
            self.spawn_stream_push();
            let (did, odds) = {
                let st = self.state.lock().unwrap();
                (st.draft_id, st.last_odds.clone())
            };
            if let (Some(did), Some(o), Some(store)) = (did, odds, &self.store) {
                let terms: Vec<_> = o
                    .terms
                    .iter()
                    .take(5)
                    .map(|t| {
                        json!({
                            "kind": t.kind,
                            "champions": t.champions,
                            "value": (t.value * 100.0).round() / 100.0,
                        })
                    })
                    .collect();
                let _ = store.save_recs(
                    did,
                    "odds",
                    &json!({
                        "odds": (o.win * 100.0).round() / 100.0,
                        "known": o.known,
                        "pending": o.pending,
                        "terms": terms,
                    }),
                );
            }
        }

        if locked_in {
            let (did, ev, my_locked) = {
                let mut st = self.state.lock().unwrap();
                if let Some(ev) = &st.last_eval {
                    st.last_role = ev.role.clone();
                }
                (
                    st.draft_id,
                    st.last_eval.clone(),
                    st.session.draft.as_ref().map_or(0, |d| d.my_locked),
                )
            };
            if let (Some(did), Some(ev), Some(store)) = (did, ev, &self.store) {
                let _ = store.update_draft(did, None, Some(my_locked), Some(&ev.role));
                let _ = store.save_recs(did, "locked", &json!({"role": ev.role}));
            }
        }

        if draft_ended {
            if let Some(stream_cfg) = stream_end {
                self.spawn_stream_end(stream_cfg);
            }
            let (did, ev) = {
                let mut st = self.state.lock().unwrap();
                let res = (st.draft_id, st.last_eval.clone());
                st.draft_id = None;
                st.last_eval = None;
                res
            };
            if let (Some(did), Some(store)) = (did, &self.store) {
                if let Some(ev) = ev {
                    let _ = store.save_recs(did, "final", &json!({"role": ev.role}));
                }
                let now = SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .map(|d| d.as_secs_f64())
                    .unwrap_or(0.0);
                let _ = store.update_draft(did, Some(now), None, None);
            }
        }
    }

    fn current_stream_payload(&self) -> Option<(StreamConfig, StreamDraftPayload)> {
        let tier = self.tier();
        let st = self.state.lock().unwrap();
        if !st.cfg.stream.enabled || st.cfg.stream.endpoint.trim().is_empty() {
            return None;
        }
        let stage = st.session.stage();
        if !stage.is_draft() {
            return None;
        }
        let draft = st.session.draft.as_ref()?;
        let payload = StreamDraftPayload::from_draft(stage, &tier, draft, &st.session.enemy_lanes);
        Some((st.cfg.stream.clone(), payload))
    }

    fn spawn_stream_start(self: &Arc<Self>, stream_cfg: StreamConfig, tier: String) {
        let ctl = Arc::clone(self);
        tokio::spawn(async move {
            match ctl.streamer.start_room(&stream_cfg, &tier).await {
                Ok(url) => {
                    let still_in_draft = {
                        let mut st = ctl.state.lock().unwrap();
                        if st.session.stage().is_draft() {
                            st.live_url = Some(url);
                            true
                        } else {
                            false
                        }
                    };
                    if still_in_draft {
                        if let Some((cfg, payload)) = ctl.current_stream_payload() {
                            if let Err(e) = ctl.streamer.push_state(&cfg, &payload).await {
                                warn!("stream initial push_state failed: {e}");
                            }
                        }
                        ctl.render(Arc::clone(&ctl)).await;
                    } else if let Err(e) = ctl.streamer.end_room(&stream_cfg).await {
                        warn!("stream end_room after late start failed: {e}");
                    }
                }
                Err(e) => {
                    warn!("stream start_room failed: {e}");
                }
            }
        });
    }

    fn spawn_stream_push(self: &Arc<Self>) {
        if let Some((cfg, payload)) = self.current_stream_payload() {
            let streamer = Arc::clone(&self.streamer);
            tokio::spawn(async move {
                if let Err(e) = streamer.push_state(&cfg, &payload).await {
                    warn!("stream push_state failed: {e}");
                }
            });
        }
    }

    fn spawn_stream_end(self: &Arc<Self>, stream_cfg: StreamConfig) {
        let streamer = Arc::clone(&self.streamer);
        tokio::spawn(async move {
            if let Err(e) = streamer.end_room(&stream_cfg).await {
                warn!("stream end_room failed: {e}");
            }
        });
    }

    fn start_sync(self: Arc<Self>, delay_secs: f64) {
        let Some(store) = self.store.clone() else {
            return;
        };
        let lockfile = {
            let mut st = self.state.lock().unwrap();
            if st.synced_at.is_some_and(|t| t.elapsed() < SYNC_EVERY) {
                return;
            }
            st.synced_at = Some(Instant::now());
            st.cfg.lockfile.clone()
        };
        tokio::spawn(async move {
            if delay_secs > 0.0 {
                tokio::time::sleep(Duration::from_secs_f64(delay_secs)).await;
            }
            if let Ok(creds) = read_lockfile(Some(&lockfile)) {
                let client = LcuClient::new(&creds);
                let m = fetch_mastery(&client).await;
                if !m.is_empty() {
                    self.state.lock().unwrap().mastery = m;
                }
                sync_history(&client, &store).await;
            }
        });
    }

    fn start_preload(self: Arc<Self>) {
        let Some(rec) = self.get_rec() else {
            return;
        };
        tokio::spawn(async move {
            let tier = rec.tier();
            let (stage, lockfile, usual) = {
                let st = self.state.lock().unwrap();
                let mut u = self
                    .store
                    .as_ref()
                    .map(|s| s.top_roles(5))
                    .unwrap_or_default();
                if !st.last_role.is_empty() && !u.contains(&st.last_role) {
                    u.insert(0, st.last_role.clone());
                }
                u.retain(|r| LANES.contains(&r.as_str()));
                (st.session.stage(), st.cfg.lockfile.clone(), u)
            };
            let roles = if stage.is_pre_draft() {
                if let Ok(creds) = read_lockfile(Some(&lockfile)) {
                    LcuClient::new(&creds).get_lobby_positions().await
                } else {
                    Vec::new()
                }
            } else {
                usual
            };
            let to_load: Vec<String> = {
                let mut st = self.state.lock().unwrap();
                let mut list = Vec::new();
                for r in roles {
                    if st.preloaded.insert((tier.clone(), r.clone())) {
                        list.push(r);
                    }
                }
                list
            };
            if to_load.is_empty() {
                return;
            }
            for role in &to_load {
                if rec.has_pool(role, Some(&tier)) {
                    continue;
                }
                let ctl_c = Arc::clone(&self);
                let _ = rec
                    .pool(role, Some(&tier), move |msg| {
                        let txt = format!("Preloading {}", msg.strip_prefix("Loading ").unwrap_or(&msg));
                        let mut st = ctl_c.state.lock().unwrap();
                        st.preload_msg = txt;
                        let render_now = st.session.stage().is_pre_draft() && st.notice.is_none();
                        drop(st);
                        if render_now {
                            let ctl_r = Arc::clone(&ctl_c);
                            tokio::spawn(async move {
                                ctl_r.render(Arc::clone(&ctl_r)).await;
                            });
                        }
                    })
                    .await;
            }
            {
                let mut st = self.state.lock().unwrap();
                st.preload_msg = format!("Stats ready: {}", to_load.join(", "));
            }
            if self.state.lock().unwrap().session.stage().is_pre_draft() {
                self.render(Arc::clone(&self)).await;
            }
        });
    }

    pub async fn toggle_ingame(self: &Arc<Self>) {
        let shown = {
            let mut st = self.state.lock().unwrap();
            st.ingame_shown = !st.ingame_shown;
            st.ingame_shown
        };
        info!("in-game panel {}", if shown { "on" } else { "off" });
        if shown {
            self.poll_live(None).await;
            self.render(Arc::clone(self)).await;
        }
    }

    pub async fn poll_live(self: &Arc<Self>, pre_data: Option<serde_json::Value>) -> bool {
        let data = match pre_data {
            Some(d) => Some(d),
            None => {
                let mut lc = self.live_client.lock().await;
                if lc.is_none() {
                    *lc = Some(LiveClient::new(Duration::from_millis(1500)));
                }
                lc.as_ref().unwrap().raw().await
            }
        };
        let g = data.as_ref().and_then(parse_live);
        let changed = {
            let mut st = self.state.lock().unwrap();
            let same = match (&st.live, &g) {
                (Some(a), Some(b)) => a.same_static_state(b),
                (None, None) => true,
                _ => false,
            };
            st.live = g.clone();
            !same
        };
        if changed {
            self.render(Arc::clone(self)).await;
        }
        g.is_some()
    }

    pub async fn live_loop(self: Arc<Self>) {
        loop {
            let (in_game, record_games, record_every, shown) = {
                let mut st = self.state.lock().unwrap();
                let ig = st.session.stage() == Stage::Ingame;
                if !ig {
                    st.recorder.end();
                }
                (ig, st.cfg.record_games, st.cfg.record_every, st.ingame_shown)
            };
            let mut data = None;
            if in_game && record_games {
                let due = self.state.lock().unwrap().recorder.due();
                if due {
                    let mut lc = self.live_client.lock().await;
                    if lc.is_none() {
                        *lc = Some(LiveClient::new(Duration::from_millis(1500)));
                    }
                    data = lc.as_ref().unwrap().raw().await;
                    if let Some(ref d) = data {
                        let mut st = self.state.lock().unwrap();
                        st.recorder.every = record_every;
                        st.recorder.save(d);
                    }
                }
            }
            if in_game && shown {
                self.poll_live(data).await;
            }
            tokio::time::sleep(LIVE_POLL).await;
        }
    }

    pub async fn set_tier(self: &Arc<Self>, tier: &str) {
        if TIERS.iter().any(|&(id, _)| id == tier)
            && let Some(rec) = self.get_rec()
        {
            rec.set_tier(tier);
            save_ui_settings(|s| s.tier = Some(tier.to_string()));
        }
        self.render(Arc::clone(self)).await;
    }

    pub async fn set_enemy_lane(self: &Arc<Self>, champion_id: i32, lane: &str) {
        self.state
            .lock()
            .unwrap()
            .session
            .set_enemy_lane(champion_id, lane);
        self.spawn_stream_push();
        self.render(Arc::clone(self)).await;
    }

    pub async fn reload_config(self: &Arc<Self>, cfg: Config) {
        *self.lockfile_watch.write().await = cfg.lockfile.clone();
        if let Some(rec) = self.get_rec() {
            *rec.cfg.write().unwrap() = cfg.clone();
        }
        self.state.lock().unwrap().cfg = cfg;
        self.render(Arc::clone(self)).await;
    }

    pub fn fail(&self, message: &str) {
        self.state.lock().unwrap().notice = Some((message.to_string(), "error".into()));
        self.show_status(message, "error");
    }

    pub fn config_snapshot(&self) -> Config {
        self.state.lock().unwrap().cfg.clone()
    }
}

pub fn record_path_for(p: Option<PathBuf>) -> Option<PathBuf> {
    p
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::champions::Champions;
    use crate::domain::ddragon::Assets;
    use crate::data::lolalytics::Lolalytics;

    #[tokio::test]
    async fn test_controller_records_draft() {
        let dir = std::env::temp_dir().join("lol_companion_test_ctl");
        let _ = std::fs::remove_dir_all(&dir);
        let store = Arc::new(Store::open(dir.join("s.sqlite")).unwrap());
        let (ctl, mut rx) = Controller::new(Config::default(), Some(store));

        let champs = Champions::from_json(&json!({"data": {"Thresh": {"key": "412", "id": "Thresh", "name": "Thresh", "image": {"full": "Thresh.png"}, "info": {"attack": 5, "magic": 6}, "tags": ["Support"]}}}), "1.0");
        let assets = Assets::default();
        let lola = Lolalytics::with_cache_dir("master_plus", 14, dir.join("cache"));
        let rec = Arc::new(Recommender::from_parts(Config::default(), champs, assets, lola));
        // Pre-populate support pool so render does not hit the network
        let mut pool = HashMap::new();
        pool.insert(
            412,
            ChampStats {
                champion_id: 412,
                lane: "support".into(),
                wr: 51.0,
                games: 10000,
                avg_wr: Some(50.0),
                matchups: HashMap::new(),
                synergies: HashMap::new(),
            },
        );
        rec.pools
            .write()
            .unwrap()
            .insert(("master_plus".into(), "support".into()), Arc::new(pool));
        *ctl.rec.lock().unwrap() = Some(rec);

        let fixture_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/session.json");
        let mut ses: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(fixture_path).unwrap()).unwrap();

        ctl.on_phase("ChampSelect").await;
        ctl.on_session(Some(&ses), None).await;
        let cur = rx.borrow_and_update().clone();
        match cur {
            ScreenUpdate::Client(c) => {
                assert!(c.picks.is_some());
                assert_eq!(c.header.role, "support");
            }
            _ => panic!("expected client screen"),
        }

        ses["actions"][1][2]["completed"] = json!(true);
        ses["actions"][1][2]["championId"] = json!(412);
        ctl.on_session(Some(&ses), None).await;
        ctl.on_phase("InProgress").await;
        let cur2 = rx.borrow_and_update().clone();
        match cur2 {
            ScreenUpdate::Client(c) => {
                assert!(c.picks.is_none());
                assert!(c.build.is_none());
            }
            _ => panic!("expected client screen"),
        }
        let _ = std::fs::remove_dir_all(&dir);
    }
}
