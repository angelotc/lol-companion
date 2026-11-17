use reqwest::Client;
use serde_json::Value;
use std::fs;
use std::path::Path;
use std::time::Duration;

use crate::domain::champions::{Champions, DDRAGON};
use crate::domain::config::cache_dir;
use crate::domain::ddragon::Assets;

pub async fn load_champions(cache: Option<&Path>) -> Result<Champions, String> {
    let dir = cache.unwrap_or(&cache_dir()).to_path_buf();
    let _ = fs::create_dir_all(&dir);
    let client = Client::builder()
        .timeout(Duration::from_secs(15))
        .build()
        .map_err(|e| e.to_string())?;

    match fetch_champions(&client, &dir).await {
        Ok(c) => Ok(c),
        Err(err) => {
            // Offline fallback: newest cached ddragon_<ver>.json
            let mut entries: Vec<_> = fs::read_dir(&dir)
                .map_err(|_| err.clone())?
                .filter_map(|e| e.ok())
                .filter(|e| {
                    let name = e.file_name().to_string_lossy().to_string();
                    name.starts_with("ddragon_")
                        && name.ends_with(".json")
                        && name
                            .strip_prefix("ddragon_")
                            .and_then(|s| s.strip_suffix(".json"))
                            .is_some_and(|v| v.chars().next().is_some_and(|c| c.is_ascii_digit()))
                })
                .collect();
            entries.sort_by_key(|e| e.metadata().and_then(|m| m.modified()).ok());
            let Some(latest) = entries.pop() else {
                return Err(err);
            };
            let ver = latest
                .file_name()
                .to_string_lossy()
                .trim_start_matches("ddragon_")
                .trim_end_matches(".json")
                .to_string();
            let txt = fs::read_to_string(latest.path()).map_err(|e| e.to_string())?;
            let val: Value = serde_json::from_str(&txt).map_err(|e| e.to_string())?;
            Ok(Champions::from_json(&val, ver))
        }
    }
}

async fn fetch_champions(client: &Client, dir: &Path) -> Result<Champions, String> {
    let vers: Vec<String> = client
        .get(format!("{DDRAGON}/api/versions.json"))
        .send()
        .await
        .map_err(|e| e.to_string())?
        .error_for_status()
        .map_err(|e| e.to_string())?
        .json()
        .await
        .map_err(|e| e.to_string())?;
    let ver = vers.first().ok_or("empty versions.json")?.clone();
    let path = dir.join(format!("ddragon_{ver}.json"));
    let data: Value = if path.exists() {
        let txt = fs::read_to_string(&path).map_err(|e| e.to_string())?;
        serde_json::from_str(&txt).map_err(|e| e.to_string())?
    } else {
        let val: Value = client
            .get(format!("{DDRAGON}/cdn/{ver}/data/en_US/champion.json"))
            .send()
            .await
            .map_err(|e| e.to_string())?
            .error_for_status()
            .map_err(|e| e.to_string())?
            .json()
            .await
            .map_err(|e| e.to_string())?;
        if let Ok(bytes) = serde_json::to_vec(&val) {
            let _ = fs::write(&path, bytes);
        }
        val
    };
    Ok(Champions::from_json(&data, ver))
}

pub async fn load_assets(version: &str, cache: Option<&Path>) -> Assets {
    let dir = cache.unwrap_or(&cache_dir()).to_path_buf();
    let _ = fs::create_dir_all(&dir);
    let files = [
        ("items", format!("{DDRAGON}/cdn/{version}/data/en_US/item.json"), Value::Object(Default::default())),
        ("runes", format!("{DDRAGON}/cdn/{version}/data/en_US/runesReforged.json"), Value::Array(Vec::new())),
        ("summoners", format!("{DDRAGON}/cdn/{version}/data/en_US/summoner.json"), Value::Object(Default::default())),
    ];
    let client = Client::builder()
        .timeout(Duration::from_secs(15))
        .build()
        .ok();
    let mut loaded: [Value; 3] = [
        Value::Object(Default::default()),
        Value::Array(Vec::new()),
        Value::Object(Default::default()),
    ];
    for (idx, (name, url, fallback)) in files.into_iter().enumerate() {
        let path = dir.join(format!("ddragon_{name}_{version}.json"));
        if path.exists()
            && let Ok(txt) = fs::read_to_string(&path)
            && let Ok(val) = serde_json::from_str(&txt)
        {
            loaded[idx] = val;
            continue;
        }
        if let Some(c) = &client
            && let Ok(resp) = c.get(&url).send().await
            && let Ok(ok) = resp.error_for_status()
            && let Ok(val) = ok.json::<Value>().await
        {
            if let Ok(bytes) = serde_json::to_vec(&val) {
                let _ = fs::write(&path, bytes);
            }
            loaded[idx] = val;
            continue;
        }
        loaded[idx] = fallback;
    }
    Assets::from_json(version, &loaded[0], &loaded[1], &loaded[2])
}
