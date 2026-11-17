#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod domain;
mod data;
mod engine;
mod images;
mod tray;
mod ui;
mod win32;

use clap::Parser;
use egui::{ViewportBuilder, ViewportCommand, ViewportId};
use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::{mpsc, watch};
use tracing::{error, info};

use crate::domain::config::{Config, data_dir, default_config_path, load_config, load_ui_settings, save_ui_settings};
use crate::domain::lcu::Slot;
use crate::domain::live::LiveGame;
use crate::domain::scoring::LANES;
use crate::data::lcu::{LcuClient, LcuUpdate, read_lockfile, watch_session};
use crate::data::players::{FixtureClient, PlayerClient, game_refs, load_all};
use crate::data::store::Store;
use crate::engine::controller::{Controller, ScreenUpdate};
use crate::engine::views::GameScreen;

use images::ImageCache;
use tray::{AppTray, TrayAction};
use ui::client_view::{ClientUiState, render_client};
use ui::game_view::render_game_overlay;
use ui::settings_view::{SettingsUiState, render_settings};
use ui::theme::apply_theme;

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
}

enum AppSignal {
    TogglePanel,
    ShowPanel,
    ToggleIngame,
}

struct CompanionApp {
    ctl: Arc<Controller>,
    screen_rx: watch::Receiver<ScreenUpdate>,
    sig_rx: mpsc::UnboundedReceiver<AppSignal>,
    tray: Option<AppTray>,
    imgs: ImageCache,
    client_ui: ClientUiState,
    settings_ui: SettingsUiState,
    always_show: bool,
    quitting: bool,
    panel_shown: bool,
    game_shown: bool,
    panel_hwnd: isize,
    game_hwnd: isize,
    last_game_place: Option<win32::Placement>,
    last_panel_rect: Option<win32::Rect>,
    last_track_at: Instant,
    theme_applied: bool,
    latest_game: Option<GameScreen>,
}

impl CompanionApp {
    fn new(
        ctl: Arc<Controller>,
        screen_rx: watch::Receiver<ScreenUpdate>,
        sig_rx: mpsc::UnboundedReceiver<AppSignal>,
        tray: Option<AppTray>,
        cfg_path: PathBuf,
        always_show: bool,
    ) -> Self {
        Self {
            ctl,
            screen_rx,
            sig_rx,
            tray,
            imgs: ImageCache::new(),
            client_ui: ClientUiState::default(),
            settings_ui: SettingsUiState::new(cfg_path, format!("Version {}", env!("CARGO_PKG_VERSION"))),
            always_show,
            quitting: false,
            panel_shown: always_show,
            game_shown: false,
            panel_hwnd: 0,
            game_hwnd: 0,
            last_game_place: None,
            last_panel_rect: None,
            last_track_at: Instant::now() - Duration::from_secs(1),
            theme_applied: false,
            latest_game: None,
        }
    }

    fn toggle_panel(&mut self, ctx: &egui::Context) {
        if self.panel_hwnd == 0 {
            self.panel_hwnd = win32::find_window_by_title(PANEL_TITLE);
        }
        let minimized = self.panel_hwnd != 0 && win32::is_minimized(self.panel_hwnd);
        let hide = !self.ctl.user_hidden() && !minimized;
        self.ctl.set_user_hidden(hide);
        if hide {
            self.panel_shown = false;
            ctx.send_viewport_cmd(ViewportCommand::Visible(false));
        } else {
            self.show_panel(ctx);
        }
    }

    fn show_panel(&mut self, ctx: &egui::Context) {
        self.ctl.set_user_hidden(false);
        self.panel_shown = true;
        ctx.send_viewport_cmd(ViewportCommand::Visible(true));
        ctx.send_viewport_cmd(ViewportCommand::Focus);
        if self.panel_hwnd != 0 {
            win32::bring_to_front(self.panel_hwnd);
        }
    }

    fn tick_tracker(&mut self, ctx: &egui::Context, screen: &ScreenUpdate) {
        if let ScreenUpdate::Game(g) = screen {
            self.latest_game = Some(g.clone());
        }

        if self.panel_hwnd == 0 {
            self.panel_hwnd = win32::find_window_by_title(PANEL_TITLE);
            if self.panel_hwnd != 0 {
                win32::lock_aspect(self.panel_hwnd);
                if let Some(saved) = load_ui_settings().window_rect {
                    let r = win32::Rect {
                        left: saved[0],
                        top: saved[1],
                        right: saved[2],
                        bottom: saved[3],
                    };
                    if win32::on_screen(r) {
                        win32::set_rect(self.panel_hwnd, r);
                        self.last_panel_rect = Some(r);
                    }
                }
            }
        }

        let want_game = self.ctl.ingame_screen_active();
        let want_panel = if self.always_show && want_game {
            false
        } else {
            !self.ctl.user_hidden()
        };

        if want_panel != self.panel_shown {
            self.panel_shown = want_panel;
            ctx.send_viewport_cmd(ViewportCommand::Visible(want_panel));
            if want_panel && self.panel_hwnd != 0 {
                win32::bring_to_front(self.panel_hwnd);
            }
        }

        if want_game {
            let scale = self.ctl.config_snapshot().ingame_scale as f32;
            let (placement, focused) = if self.always_show {
                (Some(win32::place_ingame(win32::primary_work_area(), scale)), true)
            } else {
                let game = win32::find_game();
                if game == 0 || win32::is_minimized(game) {
                    (None, false)
                } else {
                    let front = win32::foreground_root();
                    let p = win32::place_ingame(win32::window_rect(game), scale);
                    (Some(p), front == game || (self.game_hwnd != 0 && front == self.game_hwnd))
                }
            };
            let show = placement.is_some() && focused;
            self.game_shown = show;
            if show && let Some(p) = placement {
                if self.game_hwnd == 0 {
                    self.game_hwnd = win32::find_window_by_title(GAME_TITLE);
                    if self.game_hwnd != 0 {
                        win32::set_no_activate(self.game_hwnd, true);
                    }
                }
                if self.game_hwnd != 0 && self.last_game_place != Some(p) {
                    win32::move_topmost(self.game_hwnd, p);
                    self.last_game_place = Some(p);
                }
            }
        } else {
            self.game_shown = false;
        }

        if self.panel_shown && self.panel_hwnd != 0 && win32::is_normal(self.panel_hwnd) {
            let r = win32::window_rect(self.panel_hwnd);
            if self.last_panel_rect != Some(r) {
                self.last_panel_rect = Some(r);
                save_ui_settings(|s| s.window_rect = Some([r.left, r.top, r.right, r.bottom]));
            }
        }
    }
}

impl eframe::App for CompanionApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        if !self.theme_applied {
            apply_theme(ctx);
            self.theme_applied = true;
            let ctx_c = ctx.clone();
            self.ctl.set_repaint_hook(move || ctx_c.request_repaint());
        }

        // Handle close-to-tray on root window
        if !self.quitting && ctx.input(|i| i.viewport().close_requested()) {
            ctx.send_viewport_cmd(ViewportCommand::CancelClose);
            self.ctl.set_user_hidden(true);
            self.panel_shown = false;
            ctx.send_viewport_cmd(ViewportCommand::Visible(false));
        }

        // Poll tray & hotkey signals
        let tray_actions = self.tray.as_ref().map(|t| t.poll_actions()).unwrap_or_default();
        for act in tray_actions {
            match act {
                TrayAction::TogglePanel => self.toggle_panel(ctx),
                TrayAction::OpenSettings => {
                    self.settings_ui.reload_from_disk();
                    self.settings_ui.open = true;
                }
                TrayAction::ToggleAutostart => {}
                TrayAction::Quit => {
                    self.quitting = true;
                    ctx.send_viewport_cmd(ViewportCommand::Close);
                    std::process::exit(0);
                }
            }
        }
        while let Ok(sig) = self.sig_rx.try_recv() {
            match sig {
                AppSignal::TogglePanel => self.toggle_panel(ctx),
                AppSignal::ShowPanel => self.show_panel(ctx),
                AppSignal::ToggleIngame => {
                    let ctl_c = Arc::clone(&self.ctl);
                    tokio::spawn(async move {
                        ctl_c.toggle_ingame().await;
                    });
                }
            }
        }

        self.imgs.poll(ctx);

        let screen = self.screen_rx.borrow_and_update().clone();
        if self.last_track_at.elapsed() >= Duration::from_millis(250) {
            self.last_track_at = Instant::now();
            self.tick_tracker(ctx, &screen);
        }

        // Root client panel
        egui::CentralPanel::default().show(ctx, |ui| {
            if let ScreenUpdate::Client(c) = &screen {
                render_client(
                    ui,
                    c,
                    &mut self.client_ui,
                    &self.ctl,
                    &mut self.imgs,
                    &mut self.settings_ui.open,
                );
            } else {
                ui.centered_and_justified(|ui| {
                    ui.label("In game — press Ctrl+X over League to toggle the overlay.");
                });
            }
        });

        // In-game frameless overlay viewport
        if self.game_shown && let Some(g) = &self.latest_game {
            let g_clone = g.clone();
            let place = self.last_game_place.unwrap_or_else(|| {
                win32::place_ingame(win32::primary_work_area(), self.ctl.config_snapshot().ingame_scale as f32)
            });
            let dpi = win32::dpi_scale(place).max(1.0);
            let builder = ViewportBuilder::default()
                .with_title(GAME_TITLE)
                .with_decorations(false)
                .with_always_on_top()
                .with_resizable(false)
                .with_inner_size([place.width as f32 / dpi, place.height as f32 / dpi])
                .with_position([place.x as f32 / dpi, place.y as f32 / dpi]);
            ctx.show_viewport_immediate(ViewportId::from_hash_of("ingame_overlay"), builder, |ctx, _| {
                egui::CentralPanel::default().show(ctx, |ui| {
                    render_game_overlay(ui, &g_clone, &mut self.imgs);
                });
            });
        }

        // Settings viewport
        if self.settings_ui.open {
            let builder = ViewportBuilder::default()
                .with_title(SETTINGS_TITLE)
                .with_inner_size([480.0, 440.0])
                .with_min_inner_size([420.0, 340.0]);
            ctx.show_viewport_immediate(ViewportId::from_hash_of("settings_win"), builder, |ctx, _| {
                if ctx.input(|i| i.viewport().close_requested()) {
                    self.settings_ui.open = false;
                }
                egui::CentralPanel::default().show(ctx, |ui| {
                    render_settings(ui, &mut self.settings_ui, &self.ctl);
                });
            });
        }

        ctx.request_repaint_after(Duration::from_millis(250));
    }
}

fn spawn_ctrl_x_listener(ctl: Arc<Controller>, sig_tx: mpsc::UnboundedSender<AppSignal>) {
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
                } else {
                    let _ = sig_tx.send(AppSignal::TogglePanel);
                }
            }
            let _ = &ctl;
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

    if live {
        let tx_show = sig_tx.clone();
        win32::wait_show_requests(move || {
            let _ = tx_show.send(AppSignal::ShowPanel);
        });
    }

    let tx_hotkey = sig_tx.clone();
    win32::register_toggle_hotkey(move || {
        let _ = tx_hotkey.send(AppSignal::TogglePanel);
    });
    spawn_ctrl_x_listener(Arc::clone(&ctl), sig_tx);

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

    let tray = AppTray::new().ok();
    let work = win32::primary_work_area();
    let w = (win32::PAGE_SIZE.0 as f32).min(work.width() as f32 * 0.9);
    let h = w / (16.0 / 9.0);
    let native_options = eframe::NativeOptions {
        viewport: ViewportBuilder::default()
            .with_title(PANEL_TITLE)
            .with_inner_size([w, h])
            .with_min_inner_size([640.0, 360.0])
            .with_visible(always_show),
        ..Default::default()
    };

    let app = CompanionApp::new(ctl, screen_rx, sig_rx, tray, cfg_path, always_show);
    if let Err(e) = eframe::run_native(PANEL_TITLE, native_options, Box::new(|_cc| Ok(Box::new(app)))) {
        error!("eframe exited with error: {e}");
    }
}
