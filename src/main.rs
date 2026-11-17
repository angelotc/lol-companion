#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod data;
mod domain;
mod engine;
mod images;
mod tray;
mod ui;
mod updater;
mod win32;

use clap::Parser;
use egui::{ViewportBuilder, ViewportCommand, ViewportId};
use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicIsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::sync::{mpsc, watch};
use tracing::{error, info};

use crate::data::lcu::{LcuClient, LcuUpdate, read_lockfile, watch_session};
use crate::data::players::{FixtureClient, PlayerClient, game_refs, load_all};
use crate::data::store::Store;
use crate::domain::config::{Config, data_dir, default_config_path, load_config, load_ui_settings, save_ui_settings};
use crate::domain::lcu::Slot;
use crate::domain::live::LiveGame;
use crate::domain::scoring::LANES;
use crate::engine::controller::{Controller, ScreenUpdate};
use crate::engine::views::GameScreen;

use images::ImageCache;
use tray::{TrayAction, TrayHandle, TrayIds};
use ui::client_view::{ClientUiState, render_client};
use ui::game_view::render_game_overlay;
use ui::settings_view::{SettingsUiState, render_settings};
use ui::theme::apply_theme;
use updater::Updater;

const PANEL_TITLE: &str = "LoL Companion";
const GAME_TITLE: &str = "LoL Companion — In game";
const SETTINGS_TITLE: &str = "LoL Companion — Settings";

#[derive(Parser, Debug, Clone)]
#[command(name = "lol-companion", about = "League of Legends draft & build companion")]
struct Cli {
    #[arg(long)]
    config: Option<PathBuf>,
    #[arg(long)]
    record: Option<PathBuf>,
    #[arg(long)]
    replay: Option<PathBuf>,
    #[arg(long, default_value_t = 1.0)]
    delay: f64,
    #[arg(long)]
    simulate: bool,
    #[arg(long)]
    role: Option<String>,
    #[arg(long, default_value = "")]
    allies: String,
    #[arg(long, default_value = "")]
    enemies: String,
    #[arg(long, default_value = "")]
    bans: String,
    #[arg(long, value_delimiter = ',')]
    items: Vec<i32>,
    #[arg(long)]
    players: bool,
    #[arg(long, default_value = "Leona")]
    champion: String,
    #[arg(long, default_value_t = 9)]
    level: i32,
    #[arg(long, default_value_t = 650.0)]
    gold: f64,
    #[arg(long)]
    screenshot: Option<PathBuf>,
}

enum AppSignal {
    TogglePanel,
    ShowPanel,
    OpenSettings,
    ToggleIngame,
    Quit,
}

struct SharedUi {
    ctl: Arc<Controller>,
    updater: Updater,
    tray: Arc<Mutex<Option<TrayHandle>>>,
    imgs: Mutex<ImageCache>,
    latest_game: Mutex<Option<GameScreen>>,
    settings_ui: Mutex<SettingsUiState>,
    egui_ctx: Mutex<Option<egui::Context>>,
    panel_hwnd: AtomicIsize,
    game_hwnd: AtomicIsize,
    panel_shown: AtomicBool,
    game_shown: AtomicBool,
    last_game_place: Mutex<Option<win32::Placement>>,
    always_show: bool,
    screenshot: Mutex<Option<PathBuf>>,
    screenshot_sent: AtomicBool,
    started_at: std::time::Instant,
}

fn check_screenshot(ctx: &egui::Context, shared: &SharedUi, ready: bool) {
    let shot = ctx.input(|i| {
        i.events.iter().find_map(|e| match e {
            egui::Event::Screenshot { image, .. } => Some(Arc::clone(image)),
            _ => None,
        })
    });
    if let Some(img) = shot
        && let Some(path) = shared.screenshot.lock().unwrap().take()
    {
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let _ = image::save_buffer(
            &path,
            img.as_raw(),
            img.size[0] as u32,
            img.size[1] as u32,
            image::ColorType::Rgba8,
        );
        std::process::exit(0);
    }
    if shared.screenshot.lock().unwrap().is_some() {
        if ready && shared.started_at.elapsed() >= Duration::from_secs(6) {
            if !shared.screenshot_sent.swap(true, Ordering::Relaxed) {
                ctx.send_viewport_cmd(ViewportCommand::Screenshot(Default::default()));
            }
        } else {
            ctx.request_repaint_after(Duration::from_millis(250));
        }
    }
}

impl SharedUi {
    fn wake_root(&self) {
        if let Some(ctx) = self.egui_ctx.lock().unwrap().as_ref() {
            ctx.request_repaint_of(ViewportId::ROOT);
        }
    }

    fn wake_ingame(&self) {
        if let Some(ctx) = self.egui_ctx.lock().unwrap().as_ref() {
            ctx.request_repaint_of(ViewportId::from_hash_of("ingame_overlay"));
        }
    }

    fn ensure_panel_visible(&self) {
        let hwnd = self.panel_hwnd.load(Ordering::Relaxed);
        if hwnd != 0 {
            win32::show_window(hwnd, true);
            win32::bring_to_front(hwnd);
        }
        self.panel_shown.store(true, Ordering::Relaxed);
        self.wake_root();
    }
}

struct CompanionApp {
    shared: Arc<SharedUi>,
    screen_rx: watch::Receiver<ScreenUpdate>,
    sig_rx: mpsc::UnboundedReceiver<AppSignal>,
    client_ui: ClientUiState,
    quitting: bool,
    initialized: bool,
    cached_screen: ScreenUpdate,
}

impl CompanionApp {
    fn new(
        shared: Arc<SharedUi>,
        screen_rx: watch::Receiver<ScreenUpdate>,
        sig_rx: mpsc::UnboundedReceiver<AppSignal>,
    ) -> Self {
        let initial = screen_rx.borrow().clone();
        Self {
            shared,
            screen_rx,
            sig_rx,
            client_ui: ClientUiState::default(),
            quitting: false,
            initialized: false,
            cached_screen: initial,
        }
    }

    fn toggle_panel(&mut self, ctx: &egui::Context) {
        let mut hwnd = self.shared.panel_hwnd.load(Ordering::Relaxed);
        if hwnd == 0 {
            hwnd = win32::find_window_by_title(PANEL_TITLE);
            if hwnd != 0 {
                self.shared.panel_hwnd.store(hwnd, Ordering::Relaxed);
            }
        }
        let minimized = hwnd != 0 && win32::is_minimized(hwnd);
        let hide = !self.shared.ctl.user_hidden() && !minimized;
        self.shared.ctl.set_user_hidden(hide);
        if hide {
            self.shared.panel_shown.store(false, Ordering::Relaxed);
            ctx.send_viewport_cmd(ViewportCommand::Visible(false));
        } else {
            self.show_panel(ctx);
        }
    }

    fn show_panel(&mut self, ctx: &egui::Context) {
        self.shared.ctl.set_user_hidden(false);
        self.shared.panel_shown.store(true, Ordering::Relaxed);
        let hwnd = self.shared.panel_hwnd.load(Ordering::Relaxed);
        if hwnd != 0 {
            win32::show_window(hwnd, true);
            win32::bring_to_front(hwnd);
        }
        ctx.send_viewport_cmd(ViewportCommand::Visible(true));
        ctx.send_viewport_cmd(ViewportCommand::Focus);
    }
}

impl eframe::App for CompanionApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        if !self.initialized {
            self.initialized = true;
            *self.shared.egui_ctx.lock().unwrap() = Some(ctx.clone());
            self.shared.imgs.lock().unwrap().bind_ctx(ctx);
            let shared_c = Arc::clone(&self.shared);
            self.shared.ctl.set_repaint_hook(move || {
                if shared_c.panel_shown.load(Ordering::Relaxed) {
                    shared_c.wake_root();
                }
                if shared_c.game_shown.load(Ordering::Relaxed) {
                    shared_c.wake_ingame();
                }
            });
        }

        if !self.quitting && ctx.input(|i| i.viewport().close_requested()) {
            ctx.send_viewport_cmd(ViewportCommand::CancelClose);
            self.shared.ctl.set_user_hidden(true);
            self.shared.panel_shown.store(false, Ordering::Relaxed);
            ctx.send_viewport_cmd(ViewportCommand::Visible(false));
        }

        while let Ok(sig) = self.sig_rx.try_recv() {
            match sig {
                AppSignal::TogglePanel => self.toggle_panel(ctx),
                AppSignal::ShowPanel => self.show_panel(ctx),
                AppSignal::OpenSettings => {
                    let mut st = self.shared.settings_ui.lock().unwrap();
                    st.reload_from_disk();
                    st.open = true;
                }
                AppSignal::ToggleIngame => {
                    let ctl_c = Arc::clone(&self.shared.ctl);
                    tokio::spawn(async move {
                        ctl_c.toggle_ingame().await;
                    });
                }
                AppSignal::Quit => {
                    self.quitting = true;
                    self.shared.updater.apply_on_exit();
                    ctx.send_viewport_cmd(ViewportCommand::Close);
                    std::process::exit(0);
                }
            }
        }

        self.shared.imgs.lock().unwrap().poll(ctx);

        if self.screen_rx.has_changed().unwrap_or(false) {
            self.cached_screen = self.screen_rx.borrow_and_update().clone();
            if let ScreenUpdate::Game(g) = &self.cached_screen {
                *self.shared.latest_game.lock().unwrap() = Some(g.clone());
            }
        }

        let want_game = self.shared.ctl.ingame_screen_active();
        let want_panel = if self.shared.always_show && want_game {
            false
        } else {
            !self.shared.ctl.user_hidden()
        };
        if want_panel != self.shared.panel_shown.load(Ordering::Relaxed) {
            self.shared.panel_shown.store(want_panel, Ordering::Relaxed);
            ctx.send_viewport_cmd(ViewportCommand::Visible(want_panel));
            if want_panel {
                let hwnd = self.shared.panel_hwnd.load(Ordering::Relaxed);
                if hwnd != 0 {
                    win32::bring_to_front(hwnd);
                }
            }
        }

        let mut open_settings = false;
        if self.shared.panel_shown.load(Ordering::Relaxed) {
            egui::CentralPanel::default().show(ctx, |ui| {
                if let ScreenUpdate::Client(c) = &self.cached_screen {
                    let mut imgs = self.shared.imgs.lock().unwrap();
                    render_client(
                        ui,
                        c,
                        &mut self.client_ui,
                        &self.shared.ctl,
                        &mut imgs,
                        &mut open_settings,
                    );
                } else {
                    ui.centered_and_justified(|ui| {
                        ui.label("In game — press Ctrl+X over League to toggle the overlay.");
                    });
                }
            });
            let panel_ready = matches!(
                &self.cached_screen,
                ScreenUpdate::Client(c)
                    if c.bans.is_some()
                        || c.picks.is_some()
                        || c.build.is_some()
                        || (c.explore && self.client_ui.explore.report.is_some())
            );
            check_screenshot(ctx, &self.shared, panel_ready);
        }
        if open_settings {
            let mut st = self.shared.settings_ui.lock().unwrap();
            st.reload_from_disk();
            st.open = true;
        }

        // In-game frameless overlay (always registered as a deferred viewport so it can be shown/hidden without touching ROOT)
        let game_shown = self.shared.game_shown.load(Ordering::Relaxed);
        let has_game_data = self.shared.latest_game.lock().unwrap().is_some();
        let place = self
            .shared
            .last_game_place
            .lock()
            .unwrap()
            .unwrap_or_else(|| {
                win32::place_ingame(
                    win32::primary_work_area(),
                    self.shared.ctl.config_snapshot().ingame_scale as f32,
                )
            });
        let dpi = win32::dpi_scale(place).max(1.0);
        let builder = ViewportBuilder::default()
            .with_title(GAME_TITLE)
            .with_decorations(false)
            .with_always_on_top()
            .with_resizable(false)
            .with_taskbar(false)
            .with_visible(game_shown && has_game_data)
            .with_inner_size([place.width as f32 / dpi, place.height as f32 / dpi])
            .with_position([place.x as f32 / dpi, place.y as f32 / dpi]);
        let shared_ig = Arc::clone(&self.shared);
        ctx.show_viewport_deferred(ViewportId::from_hash_of("ingame_overlay"), builder, move |ctx, _| {
            if shared_ig.game_hwnd.load(Ordering::Relaxed) == 0 {
                let hwnd = win32::find_window_by_title(GAME_TITLE);
                if hwnd != 0 {
                    win32::set_no_activate(hwnd, true);
                    shared_ig.game_hwnd.store(hwnd, Ordering::Relaxed);
                }
            }
            if !shared_ig.game_shown.load(Ordering::Relaxed) {
                return;
            }
            let maybe_g = shared_ig.latest_game.lock().unwrap().clone();
            if let Some(g) = maybe_g {
                let mut imgs = shared_ig.imgs.lock().unwrap();
                imgs.poll(ctx);
                egui::CentralPanel::default().show(ctx, |ui| {
                    render_game_overlay(ui, &g, &mut imgs);
                });
                check_screenshot(ctx, &shared_ig, true);
            }
        });

        // Settings deferred viewport
        let settings_open = self.shared.settings_ui.lock().unwrap().open;
        if settings_open {
            let builder = ViewportBuilder::default()
                .with_title(SETTINGS_TITLE)
                .with_inner_size([480.0, 440.0])
                .with_min_inner_size([420.0, 340.0]);
            let shared_st = Arc::clone(&self.shared);
            ctx.show_viewport_deferred(ViewportId::from_hash_of("settings_win"), builder, move |ctx, _| {
                let mut st = shared_st.settings_ui.lock().unwrap();
                if ctx.input(|i| i.viewport().close_requested()) {
                    st.open = false;
                }
                egui::CentralPanel::default().show(ctx, |ui| {
                    render_settings(ui, &mut st, &shared_st.ctl);
                });
            });
        }
    }
}

fn spawn_tracker_and_tray(
    shared: Arc<SharedUi>,
    tray_ids: Option<TrayIds>,
    sig_tx: mpsc::UnboundedSender<AppSignal>,
) {
    if let Some(ids) = tray_ids {
        let sh_m = Arc::clone(&shared);
        let tx_m = sig_tx.clone();
        std::thread::spawn(move || {
            while let Some(id) = tray::recv_menu_event() {
                if let Some(act) = ids.classify(&id) {
                    match act {
                        TrayAction::TogglePanel => {
                            sh_m.ensure_panel_visible();
                            let _ = tx_m.send(AppSignal::TogglePanel);
                        }
                        TrayAction::UpdateClick => {
                            if sh_m.updater.is_ready() {
                                sh_m.updater.apply_and_restart();
                            } else {
                                sh_m.updater.check_async();
                            }
                        }
                        TrayAction::OpenSettings => {
                            sh_m.ensure_panel_visible();
                            let _ = tx_m.send(AppSignal::OpenSettings);
                        }
                        TrayAction::ToggleAutostart(on) => {
                            if let Some(ref t) = *sh_m.tray.lock().unwrap() {
                                t.set_autostart_checked(on);
                            }
                        }
                        TrayAction::Quit => {
                            sh_m.updater.apply_on_exit();
                            let _ = tx_m.send(AppSignal::Quit);
                            std::process::exit(0);
                        }
                    }
                }
            }
        });

        let sh_t = Arc::clone(&shared);
        let tx_t = sig_tx.clone();
        std::thread::spawn(move || {
            loop {
                if tray::recv_tray_double_click() {
                    sh_t.ensure_panel_visible();
                    let _ = tx_t.send(AppSignal::TogglePanel);
                } else {
                    std::thread::sleep(Duration::from_millis(200));
                }
            }
        });
    }

    std::thread::spawn(move || {
        let mut last_panel_rect: Option<win32::Rect> = None;
        let mut last_overlay_vis = false;
        loop {
            std::thread::sleep(Duration::from_millis(250));

            if shared.panel_hwnd.load(Ordering::Relaxed) == 0 {
                let hwnd = win32::find_window_by_title(PANEL_TITLE);
                if hwnd != 0 {
                    shared.panel_hwnd.store(hwnd, Ordering::Relaxed);
                    win32::lock_aspect(hwnd);
                    if let Some(saved) = load_ui_settings().window_rect {
                        let r = win32::Rect {
                            left: saved[0],
                            top: saved[1],
                            right: saved[2],
                            bottom: saved[3],
                        };
                        if win32::on_screen(r) {
                            win32::set_rect(hwnd, r);
                            last_panel_rect = Some(r);
                        }
                    }
                }
            }
            if shared.game_hwnd.load(Ordering::Relaxed) == 0 {
                let hwnd = win32::find_window_by_title(GAME_TITLE);
                if hwnd != 0 {
                    win32::set_no_activate(hwnd, true);
                    shared.game_hwnd.store(hwnd, Ordering::Relaxed);
                }
            }

            let want_game = shared.ctl.ingame_screen_active();
            let want_panel = if shared.always_show && want_game {
                false
            } else {
                !shared.ctl.user_hidden()
            };
            if want_panel != shared.panel_shown.load(Ordering::Relaxed) {
                if want_panel {
                    shared.ensure_panel_visible();
                } else {
                    shared.wake_root();
                }
            }

            let gh = shared.game_hwnd.load(Ordering::Relaxed);
            let has_data = shared.latest_game.lock().unwrap().is_some();
            let (placement, focused) = if want_game {
                let scale = shared.ctl.config_snapshot().ingame_scale as f32;
                if shared.always_show {
                    (Some(win32::place_ingame(win32::primary_work_area(), scale)), true)
                } else {
                    let game = win32::find_game();
                    if game == 0 || win32::is_minimized(game) {
                        (None, false)
                    } else {
                        let front = win32::foreground_root();
                        let p = win32::place_ingame(win32::window_rect(game), scale);
                        (Some(p), front == game || (gh != 0 && front == gh))
                    }
                }
            } else {
                (None, false)
            };

            let vis = want_game && has_data && placement.is_some() && focused;
            shared.game_shown.store(vis, Ordering::Relaxed);
            if vis && let Some(p) = placement {
                let mut lp = shared.last_game_place.lock().unwrap();
                if *lp != Some(p) {
                    *lp = Some(p);
                    if gh != 0 {
                        win32::move_topmost(gh, p);
                    }
                }
            }
            if vis != last_overlay_vis && gh != 0 {
                if vis {
                    win32::set_no_activate(gh, true);
                    if let Some(p) = placement {
                        win32::move_topmost(gh, p);
                    }
                    win32::show_window_na(gh);
                    shared.wake_ingame();
                } else {
                    win32::show_window(gh, false);
                }
                last_overlay_vis = vis;
            }

            let ph = shared.panel_hwnd.load(Ordering::Relaxed);
            if shared.panel_shown.load(Ordering::Relaxed) && ph != 0 && win32::is_normal(ph) {
                let r = win32::window_rect(ph);
                if last_panel_rect != Some(r) {
                    last_panel_rect = Some(r);
                    save_ui_settings(|s| s.window_rect = Some([r.left, r.top, r.right, r.bottom]));
                }
            }
        }
    });
}

fn spawn_ctrl_x_listener(shared: Arc<SharedUi>, sig_tx: mpsc::UnboundedSender<AppSignal>) {
    std::thread::spawn(move || {
        let mut was = false;
        loop {
            std::thread::sleep(Duration::from_millis(33));
            // Check Ctrl+X keys FIRST; 0 Win32 window calls when idle!
            let now = win32::ctrl_x_down();
            if !now {
                was = false;
                continue;
            }
            if was {
                continue;
            }
            was = true;
            let game = win32::find_game();
            let front = win32::foreground_root();
            let client = win32::find_client();
            let panel = win32::find_window_by_title(PANEL_TITLE);
            let game_panel = win32::find_window_by_title(GAME_TITLE);
            if front != 0 && (front == game || front == client || front == panel || front == game_panel) {
                if game != 0 {
                    let _ = sig_tx.send(AppSignal::ToggleIngame);
                    shared.wake_root();
                } else {
                    shared.ensure_panel_visible();
                    let _ = sig_tx.send(AppSignal::TogglePanel);
                }
            }
        }
    });
}

async fn run_live(ctl: Arc<Controller>, record: Option<PathBuf>) {
    if let Some(ref p) = record
        && let Some(parent) = p.parent()
    {
        let _ = std::fs::create_dir_all(parent);
    }
    let (lcu_tx, mut lcu_rx) = mpsc::unbounded_channel();
    let lockfile_watch = Arc::clone(&ctl.lockfile_watch);
    tokio::spawn(async move {
        watch_session(lockfile_watch, lcu_tx).await;
    });

    let mut pickable: HashSet<i32> = HashSet::new();
    while let Some(upd) = lcu_rx.recv().await {
        match upd {
            LcuUpdate::Phase(phase) => {
                ctl.set_idle_message("Waiting for champ select...");
                ctl.on_phase(&phase).await;
            }
            LcuUpdate::Session(ses) => {
                if let (Some(p), Some(s)) = (&record, &ses)
                    && let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(p)
                {
                    use std::io::Write;
                    let t = std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .map(|d| d.as_secs_f64())
                        .unwrap_or(0.0);
                    let _ = writeln!(f, "{}", serde_json::json!({"t": t, "session": s}));
                }
                if ses.is_none() {
                    pickable.clear();
                } else if pickable.is_empty() {
                    let lf = ctl.config_snapshot().lockfile;
                    if let Ok(creds) = read_lockfile(Some(&lf))
                        && let Ok(ids) = LcuClient::new(&creds).get_pickable_ids().await
                    {
                        pickable = ids;
                    }
                }
                ctl.on_session(ses.as_ref(), Some(pickable.clone())).await;
            }
        }
    }
}

async fn run_replay(ctl: Arc<Controller>, path: PathBuf, delay_secs: f64) {
    let Ok(txt) = std::fs::read_to_string(&path) else {
        ctl.fail("Could not read replay file");
        return;
    };
    for line in txt.lines() {
        let trimmed = line.trim().trim_start_matches('\u{feff}');
        if trimmed.is_empty() {
            continue;
        }
        if let Ok(entry) = serde_json::from_str::<serde_json::Value>(trimmed)
            && let Some(ses) = entry.get("session")
        {
            ctl.on_session(Some(ses), None).await;
            tokio::time::sleep(Duration::from_secs_f64(delay_secs)).await;
        }
    }
}

async fn run_simulate(ctl: Arc<Controller>, cli: Cli) {
    let Some(rec) = ctl.get_rec() else {
        return;
    };
    let parse_ids = |csv: &str| -> Vec<i32> {
        csv.split(',')
            .filter(|s| !s.trim().is_empty())
            .filter_map(|s| rec.champs.id_from_slug(s))
            .collect()
    };
    let role = cli.role.clone().unwrap_or_else(|| "support".into());
    let allies: Vec<Slot> = parse_ids(&cli.allies)
        .into_iter()
        .map(|c| Slot {
            champion_id: c,
            position: String::new(),
            tentative: false,
            is_me: false,
        })
        .collect();
    let enemies = parse_ids(&cli.enemies);
    let bans: HashSet<i32> = parse_ids(&cli.bans).into_iter().collect();

    if cli.players {
        let fix = PathBuf::from("tests/fixtures/players");
        if let Ok(fc) = FixtureClient::load(&fix) {
            let client = Arc::new(PlayerClient::Fixture(fc));
            if let Ok((_gid, refs)) = game_refs(&client, None, |_| None).await {
                let stats = load_all(client, refs, None, String::new()).await;
                ctl.set_simulate_data(Some((1, stats)), None, true);
            }
        }
    }
    if !cli.items.is_empty() {
        let mut enemy_map = std::collections::HashMap::new();
        for (lane, cid) in LANES.iter().rev().zip(enemies.iter()) {
            enemy_map.insert((*lane).to_string(), rec.champs.name(*cid));
        }
        let lg = LiveGame {
            champion: cli.champion.clone(),
            role: role.clone(),
            level: cli.level,
            gold: cli.gold,
            items: cli.items.clone(),
            game_time: cli.level as f64 * 120.0,
            allies: Default::default(),
            enemies: enemy_map,
        };
        ctl.set_simulate_data(None, Some(lg), true);
    }

    if !cli.players && cli.items.is_empty() {
        let mut picked: HashSet<i32> = allies.iter().map(|s| s.champion_id).collect();
        picked.extend(&enemies);
        let ses = serde_json::json!({
            "localPlayerCellId": 0,
            "myTeam": std::iter::once(serde_json::json!({"cellId": 0, "championId": 0, "assignedPosition": role}))
                .chain(allies.iter().enumerate().map(|(i, a)| serde_json::json!({"cellId": i + 1, "championId": a.champion_id, "assignedPosition": a.position})))
                .collect::<Vec<_>>(),
            "theirTeam": enemies.iter().map(|c| serde_json::json!({"championId": c})).collect::<Vec<_>>(),
            "bans": {"myTeamBans": bans.iter().copied().collect::<Vec<_>>(), "theirTeamBans": []},
            "actions": [],
        });
        ctl.on_session(Some(&ses), None).await;
    } else {
        ctl.render(Arc::clone(&ctl)).await;
    }
}

fn main() {
    updater::startup();
    let _ = rustls::crypto::aws_lc_rs::default_provider().install_default();

    let cli = Cli::parse();
    if cli.simulate && cli.role.is_none() {
        eprintln!("--simulate needs --role");
        std::process::exit(2);
    }

    let _ = std::fs::create_dir_all(data_dir());
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    win32::make_dpi_aware();
    let always_show = cli.replay.is_some() || cli.simulate;
    let live = !always_show;
    if live && !win32::claim_single_instance() {
        info!("already running: showed the other copy's panel");
        return;
    }

    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .expect("tokio runtime");
    let _rt_guard = rt.enter();

    let cfg_path = cli.config.clone().unwrap_or_else(default_config_path);
    let cfg: Config = load_config(Some(&cfg_path));
    let store = if live {
        Store::open_default().ok().map(Arc::new)
    } else {
        None
    };

    let (ctl, screen_rx) = Controller::new(cfg, store);
    let (sig_tx, sig_rx) = mpsc::unbounded_channel();

    let tray_slot: Arc<Mutex<Option<TrayHandle>>> = Arc::new(Mutex::new(None));
    let tray_cb = Arc::clone(&tray_slot);
    let updater = Updater::new(move |st| {
        if let Some(ref t) = *tray_cb.lock().unwrap() {
            t.set_update_state(st);
        }
    });
    let ver = updater.current_version();
    let ver_lbl = if ver == "dev" {
        format!("Version {} (dev)", env!("CARGO_PKG_VERSION"))
    } else {
        format!("Version {ver}")
    };

    let tray_ids = if let Some((handle, ids)) = tray::spawn_tray(updater.state()) {
        *tray_slot.lock().unwrap() = Some(handle);
        Some(ids)
    } else {
        None
    };

    let shared = Arc::new(SharedUi {
        ctl: Arc::clone(&ctl),
        updater: updater.clone(),
        tray: tray_slot,
        imgs: Mutex::new(ImageCache::new()),
        latest_game: Mutex::new(None),
        settings_ui: Mutex::new(SettingsUiState::new(cfg_path, ver_lbl)),
        egui_ctx: Mutex::new(None),
        panel_hwnd: AtomicIsize::new(0),
        game_hwnd: AtomicIsize::new(0),
        panel_shown: AtomicBool::new(always_show),
        game_shown: AtomicBool::new(false),
        last_game_place: Mutex::new(None),
        always_show,
        screenshot: Mutex::new(cli.screenshot.clone()),
        screenshot_sent: AtomicBool::new(false),
        started_at: std::time::Instant::now(),
    });

    if live {
        updater.check_async();
        let tx_show = sig_tx.clone();
        let sh_show = Arc::clone(&shared);
        win32::wait_show_requests(move || {
            sh_show.ensure_panel_visible();
            let _ = tx_show.send(AppSignal::ShowPanel);
        });
    }

    let tx_hotkey = sig_tx.clone();
    let sh_hk = Arc::clone(&shared);
    win32::register_toggle_hotkey(move || {
        sh_hk.ensure_panel_visible();
        let _ = tx_hotkey.send(AppSignal::TogglePanel);
    });
    spawn_ctrl_x_listener(Arc::clone(&shared), sig_tx.clone());

    spawn_tracker_and_tray(Arc::clone(&shared), tray_ids, sig_tx);

    {
        let mut rx = screen_rx.clone();
        let sh = Arc::clone(&shared);
        tokio::spawn(async move {
            while rx.changed().await.is_ok() {
                if let ScreenUpdate::Game(g) = rx.borrow_and_update().clone() {
                    *sh.latest_game.lock().unwrap() = Some(g);
                }
                if sh.panel_shown.load(Ordering::Relaxed) {
                    sh.wake_root();
                }
                if sh.game_shown.load(Ordering::Relaxed) {
                    sh.wake_ingame();
                }
            }
        });
    }

    if cli.items.is_empty() && !cli.players {
        let ctl_live = Arc::clone(&ctl);
        tokio::spawn(async move {
            ctl_live.live_loop().await;
        });
    }

    let ctl_boot = Arc::clone(&ctl);
    let cli_boot = cli.clone();
    tokio::spawn(async move {
        if let Err(e) = ctl_boot.boot().await {
            error!("boot failed: {e}");
            ctl_boot.fail("Something went wrong loading champion data");
            return;
        }
        if cli_boot.simulate {
            run_simulate(ctl_boot, cli_boot).await;
        } else if let Some(rep) = cli_boot.replay {
            run_replay(ctl_boot, rep, cli_boot.delay).await;
        } else {
            run_live(ctl_boot, cli_boot.record).await;
        }
    });

    let work = win32::primary_work_area();
    let w = (win32::PAGE_SIZE.0 as f32).min(work.width() as f32 * 0.9);
    let h = w / (16.0 / 9.0);
    let mut viewport = ViewportBuilder::default()
        .with_title(PANEL_TITLE)
        .with_inner_size([w, h])
        .with_min_inner_size([640.0, 360.0])
        .with_visible(always_show);
    if let Ok(dyn_img) = image::load_from_memory(include_bytes!("../assets/icon.png")) {
        let rgba = dyn_img.to_rgba8();
        let (width, height) = rgba.dimensions();
        viewport = viewport.with_icon(Arc::new(egui::IconData {
            rgba: rgba.into_raw(),
            width,
            height,
        }));
    }
    let native_options = eframe::NativeOptions {
        viewport,
        ..Default::default()
    };

    let app = CompanionApp::new(shared, screen_rx, sig_rx);
    if let Err(e) = eframe::run_native(PANEL_TITLE, native_options, Box::new(|cc| {
        apply_theme(&cc.egui_ctx);
        Ok(Box::new(app))
    })) {
        error!("eframe exited with error: {e}");
    }
}
