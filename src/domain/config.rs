use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use toml_edit::{DocumentMut, Item, Table, Value};
use tracing::warn;

use super::scoring::{BlindConfig, LANES, OddsConfig};

pub const DEFAULT_LOCKFILE: &str = r"C:\Riot Games\League of Legends\lockfile";
const DEFAULT_CONFIG_TOML: &str = include_str!("../../config.toml");

pub fn data_dir() -> PathBuf {
    let base = std::env::var_os("LOCALAPPDATA")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            std::env::var_os("USERPROFILE")
                .map(PathBuf::from)
                .unwrap_or_else(|| PathBuf::from("."))
        });
    base.join("league-analysis")
}

pub fn cache_dir() -> PathBuf {
    data_dir().join("cache")
}

pub fn default_config_path() -> PathBuf {
    let exe = std::env::current_exe().ok();
    select_config_path(
        Path::new("config.toml"),
        exe.as_deref().and_then(Path::parent),
        &data_dir(),
    )
}

fn select_config_path(local: &Path, exe_dir: Option<&Path>, user_dir: &Path) -> PathBuf {
    // Use the same per-user config as the previous Windows release.
    if !exe_dir.is_some_and(is_velopack_install) {
        if local.exists() {
            return local.to_path_buf();
        }
        if let Some(dir) = exe_dir {
            let beside = dir.join("config.toml");
            if beside.exists() {
                return beside;
            }
        }
    }

    let user = user_dir.join("config.toml");
    if !user.exists()
        && fs::create_dir_all(user_dir).is_ok()
        && let Ok(mut file) = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&user)
    {
        let _ = file.write_all(DEFAULT_CONFIG_TOML.as_bytes());
    }
    user
}

fn is_velopack_install(exe_dir: &Path) -> bool {
    exe_dir
        .file_name()
        .is_some_and(|name| name.eq_ignore_ascii_case("current"))
        && exe_dir.join("sq.version").is_file()
        && exe_dir
            .parent()
            .is_some_and(|root| root.join("Update.exe").is_file())
}

fn duo_partner(role: &str) -> Option<&'static str> {
    match role {
        "bottom" => Some("support"),
        "support" => Some("bottom"),
        _ => None,
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct ComfortConfig {
    pub weight: f64,
    pub k: f64,
    pub history: f64,
    pub mastery: f64,
    pub mastery_cap: f64,
    pub half_life: f64,
    pub min_games: u32,
}

impl Default for ComfortConfig {
    fn default() -> Self {
        Self {
            weight: 1.0,
            k: 40.0,
            history: 0.5,
            mastery: 0.15,
            mastery_cap: 1.0,
            half_life: 90.0,
            min_games: 3,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
#[serde(default)]
pub struct ComfortOverride {
    pub weight: Option<f64>,
    pub k: Option<f64>,
    pub history: Option<f64>,
    pub mastery: Option<f64>,
    pub mastery_cap: Option<f64>,
    pub half_life: Option<f64>,
    pub min_games: Option<u32>,
}

impl ComfortConfig {
    pub fn apply(&mut self, ov: &ComfortOverride) {
        if let Some(v) = ov.weight {
            self.weight = v;
        }
        if let Some(v) = ov.k {
            self.k = v;
        }
        if let Some(v) = ov.history {
            self.history = v;
        }
        if let Some(v) = ov.mastery {
            self.mastery = v;
        }
        if let Some(v) = ov.mastery_cap {
            self.mastery_cap = v;
        }
        if let Some(v) = ov.half_life {
            self.half_life = v;
        }
        if let Some(v) = ov.min_games {
            self.min_games = v;
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct RoleWeights {
    pub base: f64,
    pub enemy: HashMap<String, f64>,
    pub ally: HashMap<String, f64>,
    pub comfort: ComfortConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
#[serde(default)]
pub struct RoleWeightsOverride {
    pub base: Option<f64>,
    pub enemy: HashMap<String, f64>,
    pub ally: HashMap<String, f64>,
    pub comfort: Option<ComfortOverride>,
}

pub fn default_weights(role: &str) -> RoleWeights {
    let duo = duo_partner(role);
    let mut enemy = HashMap::new();
    let mut ally = HashMap::new();
    for &lane in LANES {
        let ew = if lane == role || Some(lane) == duo {
            1.0
        } else if lane == "jungle" || role == "jungle" {
            0.5
        } else {
            0.3
        };
        enemy.insert(lane.to_string(), ew);
        if lane != role {
            let aw = if Some(lane) == duo {
                1.0
            } else if lane == "jungle" || role == "jungle" {
                0.5
            } else {
                0.3
            };
            ally.insert(lane.to_string(), aw);
        }
    }
    RoleWeights {
        base: 1.0,
        enemy,
        ally,
        comfort: ComfortConfig::default(),
    }
}

pub fn default_build_weights(role: &str) -> HashMap<String, f64> {
    let pairs: &[(&str, f64)] = match role {
        "bottom" => &[("bottom", 1.0), ("support", 1.0), ("jungle", 0.5)],
        "support" => &[("support", 1.0), ("bottom", 1.0), ("jungle", 0.5)],
        "top" => &[("top", 1.0), ("jungle", 0.5)],
        "middle" => &[("middle", 1.0), ("jungle", 0.6), ("support", 0.3)],
        "jungle" => &[
            ("jungle", 1.0),
            ("top", 0.3),
            ("middle", 0.3),
            ("bottom", 0.3),
            ("support", 0.3),
        ],
        _ => return HashMap::from([(role.to_string(), 1.0)]),
    };
    pairs
        .iter()
        .map(|&(k, v)| (k.to_string(), v))
        .collect()
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct Config {
    pub min_pick_rate: f64,
    pub tier: String,
    pub days: u32,
    pub queue: String,
    pub region: String,
    pub k: f64,
    pub tentative_factor: f64,
    pub top_n: usize,
    pub ban_count: usize,
    pub ban_vs_top: usize,
    pub build_k: f64,
    pub build_min_games: u32,
    pub build_weights: HashMap<String, HashMap<String, f64>>,
    pub ingame_scale: f64,
    pub lockfile: String,
    pub weights: HashMap<String, RoleWeightsOverride>,
    pub comfort: ComfortOverride,
    pub comfort_only: bool,
    pub odds: OddsConfig,
    pub blind: BlindConfig,
    pub record_games: bool,
    pub record_every: f64,
    pub opgg: bool,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            min_pick_rate: 1.0,
            tier: "master_plus".to_string(),
            days: 14,
            queue: "ranked".to_string(),
            region: "all".to_string(),
            k: 500.0,
            tentative_factor: 0.5,
            top_n: 8,
            ban_count: 5,
            ban_vs_top: 5,
            build_k: 300.0,
            build_min_games: 20,
            build_weights: HashMap::new(),
            ingame_scale: 1.0,
            lockfile: DEFAULT_LOCKFILE.to_string(),
            weights: HashMap::new(),
            comfort: ComfortOverride::default(),
            comfort_only: false,
            odds: OddsConfig::default(),
            blind: BlindConfig::default(),
            record_games: false,
            record_every: 15.0,
            opgg: true,
        }
    }
}

impl Config {
    pub fn weights_for(&self, role: &str) -> RoleWeights {
        let mut w = default_weights(role);
        w.comfort.apply(&self.comfort);
        if let Some(ov) = self.weights.get(role) {
            if let Some(b) = ov.base {
                w.base = b;
            }
            for (k, v) in &ov.enemy {
                w.enemy.insert(k.clone(), *v);
            }
            for (k, v) in &ov.ally {
                w.ally.insert(k.clone(), *v);
            }
            if let Some(cov) = &ov.comfort {
                w.comfort.apply(cov);
            }
        }
        w
    }

    pub fn build_weights_for(&self, role: &str) -> HashMap<String, f64> {
        let mut w = default_build_weights(role);
        if let Some(ov) = self.build_weights.get(role) {
            for (k, v) in ov {
                w.insert(k.clone(), *v);
            }
        }
        w.into_iter()
            .filter(|(k, v)| LANES.contains(&k.as_str()) && *v > 0.0)
            .collect()
    }
}

pub fn load_config(path: Option<&Path>) -> Config {
    load_config_strict(path).unwrap_or_else(|e| {
        warn!("config.toml unreadable ({e}), using defaults");
        Config::default()
    })
}

pub fn load_config_strict(path: Option<&Path>) -> Result<Config, String> {
    let p = path
        .map(PathBuf::from)
        .unwrap_or_else(default_config_path);
    let txt = fs::read_to_string(&p).map_err(|e| format!("read {}: {e}", p.display()))?;
    toml::from_str(&txt).map_err(|e| format!("parse {}: {e}", p.display()))
}

#[derive(Debug, Clone, PartialEq)]
pub struct SettingsValues {
    pub min_pick_rate: f64,
    pub comfort_only: bool,
    pub comfort_weight: f64,
    pub lockfile: String,
}

impl SettingsValues {
    pub fn defaults() -> Self {
        let c = Config::default();
        let comfort = ComfortConfig::default();
        Self {
            min_pick_rate: c.min_pick_rate,
            comfort_only: c.comfort_only,
            comfort_weight: comfort.weight,
            lockfile: c.lockfile,
        }
    }

    pub fn from_config(cfg: &Config) -> Self {
        let mut comfort = ComfortConfig::default();
        comfort.apply(&cfg.comfort);
        Self {
            min_pick_rate: cfg.min_pick_rate,
            comfort_only: cfg.comfort_only,
            comfort_weight: comfort.weight,
            lockfile: cfg.lockfile.clone(),
        }
    }

    pub fn validate(&self) -> HashMap<&'static str, String> {
        let mut errs = HashMap::new();
        if !(0.0..=5.0).contains(&self.min_pick_rate) || !self.min_pick_rate.is_finite() {
            errs.insert("min_pick_rate", "Must be between 0 and 5.".to_string());
        }
        if !(0.0..=2.0).contains(&self.comfort_weight) || !self.comfort_weight.is_finite() {
            errs.insert("comfort_weight", "Must be between 0 and 2.".to_string());
        }
        if self.lockfile.trim().is_empty() {
            errs.insert("lockfile", "Enter a path.".to_string());
        }
        errs
    }
}

pub fn save_settings_form(values: &SettingsValues, path: &Path) -> Result<HashMap<&'static str, String>, String> {
    let errs = values.validate();
    if !errs.is_empty() {
        return Ok(errs);
    }
    let raw = if path.exists() {
        fs::read_to_string(path).map_err(|e| format!("read {}: {e}", path.display()))?
    } else {
        String::new()
    };
    let mut doc: DocumentMut = raw
        .parse()
        .map_err(|e| format!("parse {}: {e}", path.display()))?;
    let defaults = SettingsValues::defaults();
    doc["min_pick_rate"] = toml_edit::value(values.min_pick_rate);
    doc["comfort_only"] = toml_edit::value(values.comfort_only);
    let trimmed_lock = values.lockfile.trim();
    let lock_val: Value = if !trimmed_lock.contains('\'') && !trimmed_lock.contains('\n') {
        Value::from(trimmed_lock)
    } else {
        Value::from(trimmed_lock)
    };
    doc["lockfile"] = Item::Value(lock_val);

    if let Some(tbl) = doc.get_mut("comfort").and_then(|i| i.as_table_mut()) {
        tbl["weight"] = toml_edit::value(values.comfort_weight);
    } else if (values.comfort_weight - defaults.comfort_weight).abs() > f64::EPSILON {
        let mut tbl = Table::new();
        tbl["weight"] = toml_edit::value(values.comfort_weight);
        doc["comfort"] = Item::Table(tbl);
    }

    let tmp = path.with_extension("toml.tmp");
    let rendered = doc.to_string();
    fs::write(&tmp, &rendered).map_err(|e| format!("write {}: {e}", tmp.display()))?;
    if let Err(e) = load_config_strict(Some(&tmp)) {
        let _ = fs::remove_file(&tmp);
        return Err(e);
    }
    fs::rename(&tmp, path).map_err(|e| format!("rename {}: {e}", path.display()))?;
    Ok(HashMap::new())
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct UiSettings {
    pub tier: Option<String>,
    pub window_rect: Option<[i32; 4]>,
}

pub fn load_ui_settings() -> UiSettings {
    let p = data_dir().join("settings.json");
    fs::read_to_string(p)
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

pub fn save_ui_settings(patch: impl FnOnce(&mut UiSettings)) {
    let p = data_dir().join("settings.json");
    let mut s = load_ui_settings();
    patch(&mut s);
    if let Some(parent) = p.parent() {
        let _ = fs::create_dir_all(parent);
    }
    if let Ok(json) = serde_json::to_string(&s) {
        let _ = fs::write(p, json);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    struct ConfigPaths {
        root: PathBuf,
        local: PathBuf,
        exe_dir: PathBuf,
        user_dir: PathBuf,
    }

    impl ConfigPaths {
        fn new() -> Self {
            static NEXT_ID: AtomicUsize = AtomicUsize::new(0);
            let root = std::env::temp_dir().join(format!(
                "lol_companion_config_paths_{}_{}",
                std::process::id(),
                NEXT_ID.fetch_add(1, Ordering::Relaxed)
            ));
            let cwd = root.join("cwd");
            let exe_dir = root.join("app").join("current");
            fs::create_dir_all(&cwd).unwrap();
            fs::create_dir_all(&exe_dir).unwrap();
            Self {
                local: cwd.join("config.toml"),
                exe_dir,
                user_dir: root.join("league-analysis"),
                root,
            }
        }

        fn make_velopack_install(&self) {
            fs::write(self.exe_dir.join("sq.version"), "").unwrap();
            fs::write(self.root.join("app").join("Update.exe"), "").unwrap();
        }

        fn select(&self) -> PathBuf {
            select_config_path(&self.local, Some(&self.exe_dir), &self.user_dir)
        }
    }

    impl Drop for ConfigPaths {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.root);
        }
    }

    #[test]
    fn packaged_config_preserves_legacy_user_settings() {
        let paths = ConfigPaths::new();
        paths.make_velopack_install();
        fs::write(&paths.local, "min_pick_rate = 4.0").unwrap();
        fs::write(paths.exe_dir.join("config.toml"), DEFAULT_CONFIG_TOML).unwrap();
        fs::create_dir_all(&paths.user_dir).unwrap();
        let user_config = paths.user_dir.join("config.toml");
        let legacy = "# My settings\nmin_pick_rate = 2.5\ncomfort_only = true\n";
        fs::write(&user_config, legacy).unwrap();

        let selected = paths.select();

        assert_eq!(selected, user_config);
        assert_eq!(fs::read_to_string(&selected).unwrap(), legacy);
        assert!(load_config_strict(Some(&selected)).unwrap().comfort_only);
    }

    #[test]
    fn packaged_config_initializes_new_user_settings_from_defaults() {
        let paths = ConfigPaths::new();
        paths.make_velopack_install();
        fs::write(&paths.local, "min_pick_rate = 4.0").unwrap();
        fs::write(paths.exe_dir.join("config.toml"), "min_pick_rate = 3.0").unwrap();

        let selected = paths.select();

        assert_eq!(selected, paths.user_dir.join("config.toml"));
        assert_eq!(fs::read_to_string(&selected).unwrap(), DEFAULT_CONFIG_TOML);
        assert!(load_config_strict(Some(&selected)).is_ok());
    }

    #[test]
    fn loose_config_prefers_working_directory_then_executable_directory() {
        let paths = ConfigPaths::new();
        fs::write(paths.exe_dir.join("sq.version"), "").unwrap();
        let beside = paths.exe_dir.join("config.toml");
        fs::write(&beside, "min_pick_rate = 3.0").unwrap();
        fs::write(&paths.local, "min_pick_rate = 4.0").unwrap();

        assert_eq!(paths.select(), paths.local);
        fs::remove_file(&paths.local).unwrap();
        assert_eq!(paths.select(), beside);
        assert!(!paths.user_dir.exists());
    }


    #[test]
    fn test_default_weights_by_role() {
        let sup = default_weights("support");
        assert_eq!(sup.enemy["support"], 1.0);
        assert_eq!(sup.enemy["bottom"], 1.0);
        assert_eq!(sup.ally["bottom"], 1.0);
        assert!(!sup.ally.contains_key("support"));

        let mid = default_weights("middle");
        assert_eq!(mid.enemy["middle"], 1.0);
        assert_eq!(mid.enemy["jungle"], 0.5);
        assert_eq!(mid.enemy["top"], 0.3);
        let max_ally = mid.ally.values().copied().fold(0.0_f64, f64::max);
        assert_eq!(max_ally, 0.5);

        let jg = default_weights("jungle");
        assert_eq!(jg.enemy["jungle"], 1.0);
        assert!(jg.ally.values().all(|&v| (v - 0.5).abs() < 1e-9));
    }

    #[test]
    fn test_weights_override_merges() {
        let mut cfg = Config::default();
        cfg.weights.insert(
            "middle".into(),
            RoleWeightsOverride {
                enemy: HashMap::from([("jungle".into(), 0.9)]),
                ..Default::default()
            },
        );
        let w = cfg.weights_for("middle");
        assert_eq!(w.enemy["jungle"], 0.9);
        assert_eq!(w.enemy["middle"], 1.0);
        assert_eq!(w.base, 1.0);
    }

    #[test]
    fn test_build_weights_per_role() {
        let mut cfg = Config::default();
        assert_eq!(
            cfg.build_weights_for("support"),
            HashMap::from([
                ("support".into(), 1.0),
                ("bottom".into(), 1.0),
                ("jungle".into(), 0.5)
            ])
        );
        assert_eq!(
            cfg.build_weights_for("top"),
            HashMap::from([("top".into(), 1.0), ("jungle".into(), 0.5)])
        );
        assert_eq!(
            cfg.build_weights_for("middle"),
            HashMap::from([
                ("middle".into(), 1.0),
                ("jungle".into(), 0.6),
                ("support".into(), 0.3)
            ])
        );
        assert_eq!(cfg.build_weights_for("jungle").len(), 5);

        cfg.build_weights.insert(
            "middle".into(),
            HashMap::from([("support".into(), 0.0)]),
        );
        assert_eq!(
            cfg.build_weights_for("middle"),
            HashMap::from([("middle".into(), 1.0), ("jungle".into(), 0.6)])
        );
    }

    #[test]
    fn test_settings_save_preserves_comments() {
        let dir = std::env::temp_dir().join("lol_companion_test_cfg");
        let _ = fs::create_dir_all(&dir);
        let path = dir.join("config.toml");
        fs::write(&path, "# keep this comment\nmin_pick_rate = 1.0\nlockfile = 'C:\\old'\n").unwrap();
        let vals = SettingsValues {
            min_pick_rate: 2.5,
            comfort_only: true,
            comfort_weight: 0.5,
            lockfile: r"D:\Games\lockfile".into(),
        };
        let errs = save_settings_form(&vals, &path).unwrap();
        assert!(errs.is_empty());
        let saved = fs::read_to_string(&path).unwrap();
        assert!(saved.contains("# keep this comment"));
        let loaded = load_config_strict(Some(&path)).unwrap();
        assert_eq!(loaded.min_pick_rate, 2.5);
        assert!(loaded.comfort_only);
        assert_eq!(loaded.comfort.weight, Some(0.5));
        assert_eq!(loaded.lockfile, r"D:\Games\lockfile");
        let _ = fs::remove_dir_all(&dir);
    }
}
