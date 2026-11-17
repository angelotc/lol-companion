use flate2::Compression;
use flate2::read::ZlibDecoder;
use flate2::write::ZlibEncoder;
use rusqlite::{Connection, OptionalExtension, params};
use serde_json::Value;
use sha1::{Digest, Sha1};
use std::collections::HashMap;
use std::fs;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::domain::config::data_dir;

pub const DRAFT_GAME_WINDOW: f64 = 15.0 * 60.0;

const SCHEMA: &str = r#"
CREATE TABLE IF NOT EXISTS drafts (
    id INTEGER PRIMARY KEY,
    started REAL NOT NULL,
    ended REAL,
    role TEXT,
    tier TEXT,
    my_champ INTEGER,
    game_id INTEGER,
    result TEXT
);
CREATE TABLE IF NOT EXISTS snapshots (
    draft_id INTEGER NOT NULL REFERENCES drafts(id),
    t REAL NOT NULL,
    session BLOB NOT NULL
);
CREATE TABLE IF NOT EXISTS recommendations (
    draft_id INTEGER NOT NULL REFERENCES drafts(id),
    t REAL NOT NULL,
    kind TEXT NOT NULL,
    payload TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS games (
    game_id INTEGER PRIMARY KEY,
    created REAL NOT NULL,
    queue INTEGER,
    champ INTEGER,
    role TEXT,
    win INTEGER,
    duration INTEGER,
    detail TEXT
);
CREATE INDEX IF NOT EXISTS games_champ ON games(champ, role);
"#;

fn now_secs() -> f64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs_f64())
        .unwrap_or(0.0)
}

pub fn pack_json(val: &Value) -> Vec<u8> {
    let bytes = serde_json::to_vec(val).unwrap_or_default();
    let mut enc = ZlibEncoder::new(Vec::new(), Compression::default());
    let _ = enc.write_all(&bytes);
    enc.finish().unwrap_or_default()
}

pub fn unpack_json(blob: &[u8]) -> Option<Value> {
    let mut dec = ZlibDecoder::new(blob);
    let mut out = Vec::new();
    dec.read_to_end(&mut out).ok()?;
    serde_json::from_slice(&out).ok()
}

pub struct Store {
    db: Mutex<Connection>,
    last_hash: Mutex<HashMap<i64, [u8; 20]>>,
}

impl Store {
    pub fn open_default() -> Result<Self, rusqlite::Error> {
        Self::open(data_dir().join("store.sqlite"))
    }

    pub fn open(path: impl AsRef<Path>) -> Result<Self, rusqlite::Error> {
        let p: PathBuf = path.as_ref().to_path_buf();
        if let Some(parent) = p.parent() {
            let _ = fs::create_dir_all(parent);
        }
        let conn = Connection::open(p)?;
        conn.execute_batch("PRAGMA journal_mode=WAL;")?;
        conn.execute_batch(SCHEMA)?;
        Ok(Self {
            db: Mutex::new(conn),
            last_hash: Mutex::new(HashMap::new()),
        })
    }

    pub fn start_draft(&self, role: Option<&str>, tier: &str) -> Result<i64, rusqlite::Error> {
        let db = self.db.lock().unwrap();
        db.execute(
            "INSERT INTO drafts(started, role, tier) VALUES (?,?,?)",
            params![now_secs(), role, tier],
        )?;
        Ok(db.last_insert_rowid())
    }

    pub fn update_draft(
        &self,
        draft_id: i64,
        ended: Option<f64>,
        my_champ: Option<i32>,
        role: Option<&str>,
    ) -> Result<(), rusqlite::Error> {
        let db = self.db.lock().unwrap();
        if let Some(e) = ended {
            db.execute("UPDATE drafts SET ended=? WHERE id=?", params![e, draft_id])?;
        }
        if let Some(c) = my_champ {
            db.execute("UPDATE drafts SET my_champ=? WHERE id=?", params![c, draft_id])?;
        }
        if let Some(r) = role {
            db.execute("UPDATE drafts SET role=? WHERE id=?", params![r, draft_id])?;
        }
        Ok(())
    }

    /// Dedup by hashing serialized JSON *before* zlib compression to avoid wasted CPU on repeats.
    pub fn snapshot(&self, draft_id: i64, session: &Value) -> Result<(), rusqlite::Error> {
        let raw = serde_json::to_vec(session).unwrap_or_default();
        let mut hasher = Sha1::new();
        hasher.update(&raw);
        let digest: [u8; 20] = hasher.finalize().into();
        {
            let mut lh = self.last_hash.lock().unwrap();
            if lh.get(&draft_id) == Some(&digest) {
                return Ok(());
            }
            lh.insert(draft_id, digest);
        }
        let mut enc = ZlibEncoder::new(Vec::new(), Compression::default());
        let _ = enc.write_all(&raw);
        let blob = enc.finish().unwrap_or_default();
        let db = self.db.lock().unwrap();
        db.execute(
            "INSERT INTO snapshots VALUES (?,?,?)",
            params![draft_id, now_secs(), blob],
        )?;
        Ok(())
    }

    pub fn save_recs(&self, draft_id: i64, kind: &str, payload: &Value) -> Result<(), rusqlite::Error> {
        let s = serde_json::to_string(payload).unwrap_or_default();
        let db = self.db.lock().unwrap();
        db.execute(
            "INSERT INTO recommendations VALUES (?,?,?,?)",
            params![draft_id, now_secs(), kind, s],
        )?;
        Ok(())
    }

    pub fn has_game(&self, game_id: i64) -> Result<bool, rusqlite::Error> {
        let db = self.db.lock().unwrap();
        let row: Option<i64> = db
            .query_row("SELECT 1 FROM games WHERE game_id=?", params![game_id], |r| r.get(0))
            .optional()?;
        Ok(row.is_some())
    }

    pub fn add_game(
        &self,
        game_id: i64,
        created: f64,
        queue: i64,
        champ: i32,
        role: &str,
        win: bool,
        duration: i64,
        detail: &Value,
    ) -> Result<(), rusqlite::Error> {
        let s = serde_json::to_string(detail).unwrap_or_default();
        let db = self.db.lock().unwrap();
        db.execute(
            "INSERT OR IGNORE INTO games VALUES (?,?,?,?,?,?,?,?)",
            params![game_id, created, queue, champ, role, i32::from(win), duration, s],
        )?;
        Ok(())
    }

    pub fn draft_role_for(&self, champ: i32, created: f64) -> Option<String> {
        let db = self.db.lock().unwrap();
        let res: Option<Option<String>> = db
            .query_row(
                "SELECT role FROM drafts WHERE my_champ=? AND ended IS NOT NULL \
                 AND ended BETWEEN ? AND ? ORDER BY ABS(? - ended) LIMIT 1",
                params![champ, created - DRAFT_GAME_WINDOW, created + 60.0, created],
                |r| r.get(0),
            )
            .optional()
            .ok()
            .flatten();
        res.flatten()
    }

    pub fn link_draft(&self, game_id: i64, champ: i32, created: f64, win: bool) -> Result<(), rusqlite::Error> {
        let db = self.db.lock().unwrap();
        let id: Option<i64> = db
            .query_row(
                "SELECT id FROM drafts WHERE game_id IS NULL AND my_champ=? AND ended IS NOT NULL \
                 AND ended BETWEEN ? AND ? ORDER BY ABS(? - ended) LIMIT 1",
                params![champ, created - DRAFT_GAME_WINDOW, created + 60.0, created],
                |r| r.get(0),
            )
            .optional()?;
        if let Some(id) = id {
            let result = if win { "win" } else { "loss" };
            db.execute(
                "UPDATE drafts SET game_id=?, result=? WHERE id=?",
                params![game_id, result, id],
            )?;
        }
        Ok(())
    }

    pub fn top_roles(&self, limit: usize) -> Vec<String> {
        let db = self.db.lock().unwrap();
        let Ok(mut stmt) = db.prepare(
            "SELECT role FROM games WHERE role IS NOT NULL AND role != '' \
             GROUP BY role ORDER BY COUNT(*) DESC LIMIT ?",
        ) else {
            return Vec::new();
        };
        let Ok(rows) = stmt.query_map(params![limit as i64], |r| r.get::<_, String>(0)) else {
            return Vec::new();
        };
        rows.filter_map(|r| r.ok()).collect()
    }

    /// Returns champion_id -> (games, wins, eff_games, eff_wins)
    pub fn champ_record(
        &self,
        role: Option<&str>,
        half_life_days: f64,
    ) -> HashMap<i32, (u32, u32, f64, f64)> {
        let db = self.db.lock().unwrap();
        let mut out: HashMap<i32, (u32, u32, f64, f64)> = HashMap::new();
        let now = now_secs();
        let decay = if half_life_days > 0.0 {
            2.0_f64.ln() / (half_life_days * 86400.0)
        } else {
            0.0
        };

        let mut push_row = |cid: i32, win: i32, created: f64| {
            let w = if decay > 0.0 {
                (-(now - created).max(0.0) * decay).exp()
            } else {
                1.0
            };
            let e = out.entry(cid).or_insert((0, 0, 0.0, 0.0));
            e.0 += 1;
            if win != 0 {
                e.1 += 1;
            }
            e.2 += w;
            if win != 0 {
                e.3 += w;
            }
        };

        if let Some(r) = role {
            if let Ok(mut stmt) = db.prepare("SELECT champ, win, created FROM games WHERE role=?")
                && let Ok(rows) = stmt.query_map(params![r], |row| {
                    Ok((row.get::<_, i32>(0)?, row.get::<_, i32>(1)?, row.get::<_, f64>(2)?))
                })
            {
                for row in rows.flatten() {
                    push_row(row.0, row.1, row.2);
                }
            }
        } else if let Ok(mut stmt) = db.prepare("SELECT champ, win, created FROM games")
            && let Ok(rows) = stmt.query_map([], |row| {
                Ok((row.get::<_, i32>(0)?, row.get::<_, i32>(1)?, row.get::<_, f64>(2)?))
            })
        {
            for row in rows.flatten() {
                push_row(row.0, row.1, row.2);
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_store_draft_and_games() {
        let dir = std::env::temp_dir().join("lol_companion_test_store");
        let _ = fs::remove_dir_all(&dir);
        let store = Store::open(dir.join("s.sqlite")).unwrap();
        let did = store.start_draft(Some("support"), "master_plus").unwrap();
        store.snapshot(did, &json!({"a": 1})).unwrap();
        store.snapshot(did, &json!({"a": 1})).unwrap();
        store.update_draft(did, Some(1000.0), Some(412), Some("support")).unwrap();
        store.add_game(99, 1050.0, 420, 412, "support", true, 1800, &json!({})).unwrap();
        store.link_draft(99, 412, 1050.0, true).unwrap();
        assert!(store.has_game(99).unwrap());
        assert_eq!(store.top_roles(2), vec!["support"]);
        let rec = store.champ_record(Some("support"), 90.0);
        assert_eq!(rec[&412].0, 1);
        assert_eq!(rec[&412].1, 1);
        let _ = fs::remove_dir_all(&dir);
    }
}
