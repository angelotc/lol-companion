use serde_json::Value;
use sha1::{Digest, Sha1};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, RwLock};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

#[derive(Clone)]
struct MemEntry {
    data: Arc<Value>,
    stored_at: SystemTime,
}

pub struct JsonCache {
    pub dir: PathBuf,
    pub ttl: Duration,
    mem: RwLock<HashMap<String, MemEntry>>,
}

impl JsonCache {
    pub fn new(dir: impl Into<PathBuf>, ttl: Duration) -> Self {
        let dir = dir.into();
        let _ = fs::create_dir_all(&dir);
        Self {
            dir,
            ttl,
            mem: RwLock::new(HashMap::new()),
        }
    }

    pub fn path(&self, key: &str, label: &str) -> PathBuf {
        let mut hasher = Sha1::new();
        hasher.update(key.as_bytes());
        let hash = hasher.finalize();
        let hex: String = hash.iter().take(8).map(|b| format!("{b:02x}")).collect();
        if label.is_empty() {
            self.dir.join(format!("{hex}.json"))
        } else {
            self.dir.join(format!("{hex}_{label}.json"))
        }
    }

    pub fn fresh(&self, key: &str, label: &str) -> Option<Arc<Value>> {
        let now = SystemTime::now();
        if let Ok(guard) = self.mem.read()
            && let Some(entry) = guard.get(key)
            && now.duration_since(entry.stored_at).unwrap_or(Duration::MAX) < self.ttl
        {
            return Some(Arc::clone(&entry.data));
        }
        let p = self.path(key, label);
        let meta = fs::metadata(&p).ok()?;
        let modified = meta.modified().ok()?;
        if now.duration_since(modified).unwrap_or(Duration::MAX) < self.ttl {
            let val = Self::read_disk(&p)?;
            let arc = Arc::new(val);
            if let Ok(mut guard) = self.mem.write() {
                guard.insert(
                    key.to_string(),
                    MemEntry {
                        data: Arc::clone(&arc),
                        stored_at: modified,
                    },
                );
            }
            return Some(arc);
        }
        None
    }

    pub async fn get<F, Fut>(&self, key: &str, label: &str, fetch: F) -> Arc<Value>
    where
        F: FnOnce() -> Fut,
        Fut: std::future::Future<Output = Option<Value>>,
    {
        if let Some(hit) = self.fresh(key, label) {
            return hit;
        }
        let p = self.path(key, label);
        if let Some(data) = fetch().await {
            let arc = Arc::new(data);
            Self::write_disk(&p, &arc);
            if let Ok(mut guard) = self.mem.write() {
                guard.insert(
                    key.to_string(),
                    MemEntry {
                        data: Arc::clone(&arc),
                        stored_at: SystemTime::now(),
                    },
                );
            }
            return arc;
        }
        if p.exists()
            && let Some(stale) = Self::read_disk(&p)
        {
            let arc = Arc::new(stale);
            if let Ok(mut guard) = self.mem.write() {
                guard.insert(
                    key.to_string(),
                    MemEntry {
                        data: Arc::clone(&arc),
                        stored_at: UNIX_EPOCH,
                    },
                );
            }
            return arc;
        }
        Arc::new(Value::Object(Default::default()))
    }

    fn read_disk(p: &Path) -> Option<Value> {
        let txt = fs::read_to_string(p).ok()?;
        serde_json::from_str(&txt).ok()
    }

    fn write_disk(p: &Path, data: &Value) {
        let Some(parent) = p.parent() else {
            return;
        };
        let _ = fs::create_dir_all(parent);
        let tmp = parent.join(format!(
            ".tmp_{}_{}",
            std::process::id(),
            p.file_name().and_then(|n| n.to_str()).unwrap_or("cache")
        ));
        if let Ok(bytes) = serde_json::to_vec(data)
            && fs::write(&tmp, bytes).is_ok()
            && fs::rename(&tmp, p).is_err()
        {
            let _ = fs::remove_file(&tmp);
        }
    }
}
