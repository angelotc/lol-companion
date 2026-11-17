use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use tracing::{info, warn};
use velopack::{UpdateCheck, UpdateInfo, UpdateManager, VelopackApp, VelopackAsset, sources};

pub const REPO_URL: &str = "https://github.com/angelotc/lol-companion";

pub fn startup() {
    VelopackApp::build().run();
}

/// Stable executable path for Start with Windows (Velopack stub outside `current\` survives updates).
pub fn stable_exe_path() -> PathBuf {
    let Ok(exe) = std::env::current_exe() else {
        return PathBuf::new();
    };
    let in_current = exe
        .parent()
        .and_then(|p| p.file_name())
        .is_some_and(|n| n.eq_ignore_ascii_case("current"));
    if in_current
        && let Some(root) = exe.parent().and_then(|p| p.parent())
        && root.join("Update.exe").exists()
        && let Ok(entries) = std::fs::read_dir(root)
    {
        let stubs: Vec<PathBuf> = entries
            .flatten()
            .map(|e| e.path())
            .filter(|p| {
                p.extension().is_some_and(|e| e.eq_ignore_ascii_case("exe"))
                    && !p
                        .file_name()
                        .is_some_and(|n| n.eq_ignore_ascii_case("Update.exe"))
            })
            .collect();
        if stubs.len() == 1 {
            return stubs.into_iter().next().unwrap();
        }
    }
    exe
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UpdateState {
    Unsupported,
    Idle,
    Checking,
    Downloading(String),
    Ready(String),
    Error,
}

enum PendingUpdate {
    Info(Box<UpdateInfo>),
    Asset(VelopackAsset),
}

impl PendingUpdate {
    fn as_asset(&self) -> &VelopackAsset {
        match self {
            PendingUpdate::Info(i) => i.as_ref().as_ref(),
            PendingUpdate::Asset(a) => a,
        }
    }
}

struct Inner {
    state: UpdateState,
    mgr: Option<UpdateManager>,
    pending: Option<PendingUpdate>,
    on_change: Box<dyn Fn(UpdateState) + Send + Sync>,
}

#[derive(Clone)]
pub struct Updater {
    inner: Arc<Mutex<Inner>>,
    busy: Arc<Mutex<()>>,
}

impl Updater {
    pub fn new(on_change: impl Fn(UpdateState) + Send + Sync + 'static) -> Self {
        let source = sources::GithubSource::new(REPO_URL, None, false);
        let (mgr, state, pending) = match UpdateManager::new(source, None, None) {
            Ok(um) => {
                if let Some(asset) = um.get_update_pending_restart() {
                    let ver = asset.Version.clone();
                    (Some(um), UpdateState::Ready(ver), Some(PendingUpdate::Asset(asset)))
                } else {
                    (Some(um), UpdateState::Idle, None)
                }
            }
            Err(e) => {
                info!("updates unsupported (not Velopack-installed): {e}");
                (None, UpdateState::Unsupported, None)
            }
        };
        Self {
            inner: Arc::new(Mutex::new(Inner {
                state,
                mgr,
                pending,
                on_change: Box::new(on_change),
            })),
            busy: Arc::new(Mutex::new(())),
        }
    }

    pub fn state(&self) -> UpdateState {
        self.inner.lock().unwrap().state.clone()
    }

    pub fn current_version(&self) -> String {
        let g = self.inner.lock().unwrap();
        match &g.mgr {
            Some(um) => um.get_current_version_as_string(),
            None => "dev".to_string(),
        }
    }

    pub fn is_ready(&self) -> bool {
        matches!(self.inner.lock().unwrap().state, UpdateState::Ready(_))
    }

    fn set_state(&self, st: UpdateState) {
        let mut g = self.inner.lock().unwrap();
        g.state = st.clone();
        (g.on_change)(st);
    }

    pub fn check_async(&self) {
        let mgr = {
            let g = self.inner.lock().unwrap();
            if matches!(
                g.state,
                UpdateState::Unsupported | UpdateState::Checking | UpdateState::Downloading(_) | UpdateState::Ready(_)
            ) {
                return;
            }
            g.mgr.clone()
        };
        let Some(um) = mgr else {
            return;
        };
        let this = self.clone();
        std::thread::spawn(move || {
            let Ok(_guard) = this.busy.try_lock() else {
                return;
            };
            this.set_state(UpdateState::Checking);
            match um.check_for_updates() {
                Ok(UpdateCheck::UpdateAvailable(info)) => {
                    let ver = info.TargetFullRelease.Version.clone();
                    info!("update available: {ver}");
                    this.set_state(UpdateState::Downloading(ver.clone()));
                    match um.download_updates(&info, None) {
                        Ok(()) => {
                            let mut g = this.inner.lock().unwrap();
                            g.pending = Some(PendingUpdate::Info(info));
                            let ready = UpdateState::Ready(ver);
                            g.state = ready.clone();
                            (g.on_change)(ready);
                        }
                        Err(e) => {
                            warn!("update download failed: {e}");
                            this.set_state(UpdateState::Error);
                        }
                    }
                }
                Ok(UpdateCheck::NoUpdateAvailable | UpdateCheck::RemoteIsEmpty) => {
                    info!("up to date");
                    this.set_state(UpdateState::Idle);
                }
                Err(e) => {
                    warn!("update check failed: {e}");
                    this.set_state(UpdateState::Error);
                }
            }
        });
    }

    pub fn apply_and_restart(&self) {
        let (mgr, asset) = {
            let g = self.inner.lock().unwrap();
            let Some(um) = g.mgr.clone() else { return };
            let Some(p) = &g.pending else { return };
            (um, p.as_asset().clone())
        };
        if let Err(e) = mgr.apply_updates_and_restart(&asset) {
            warn!("apply_updates_and_restart failed: {e}");
            self.set_state(UpdateState::Error);
        }
    }

    pub fn apply_on_exit(&self) {
        let (mgr, asset) = {
            let g = self.inner.lock().unwrap();
            if !matches!(g.state, UpdateState::Ready(_)) {
                return;
            }
            let Some(um) = g.mgr.clone() else { return };
            let Some(p) = &g.pending else { return };
            (um, p.as_asset().clone())
        };
        if let Err(e) = mgr.wait_exit_then_apply_updates(&asset, true, false, Vec::<String>::new()) {
            warn!("wait_exit_then_apply_updates failed: {e}");
        }
    }
}
