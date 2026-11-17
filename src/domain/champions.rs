use serde_json::Value;
use std::collections::HashMap;

pub const DDRAGON: &str = "https://ddragon.leagueoflegends.com";

pub fn slug_override(s: &str) -> &str {
    match s {
        "monkeyking" => "wukong",
        other => other,
    }
}

pub fn magic_share_override(norm_id: &str) -> Option<f64> {
    Some(match norm_id {
        "amumu" => 0.85,
        "alistar" => 0.75,
        "malphite" => 0.8,
        "sejuani" => 0.75,
        "zac" => 0.8,
        "maokai" => 0.8,
        "rammus" => 0.7,
        "nautilus" => 0.7,
        "leona" => 0.6,
        "braum" => 0.6,
        "chogath" => 0.8,
        "sion" => 0.4,
        "ornn" => 0.6,
        "kaisa" => 0.6,
        "ezreal" => 0.35,
        "corki" => 0.6,
        "kayle" => 0.65,
        "shyvana" => 0.55,
        "belveth" => 0.1,
        "varus" => 0.35,
        "akali" => 0.85,
        "ekko" => 0.85,
        "diana" => 0.9,
        "gragas" => 0.85,
        "volibear" => 0.5,
        "teemo" => 0.85,
        "kennen" => 0.85,
        _ => return None,
    })
}

pub fn norm(s: &str) -> String {
    s.chars()
        .filter(|c| c.is_ascii_alphanumeric())
        .map(|c| c.to_ascii_lowercase())
        .collect()
}

#[derive(Debug, Clone)]
pub struct ChampionInfo {
    pub id: i32,
    pub ddragon_id: String,
    pub name: String,
    pub image_full: String,
    pub attack: f64,
    pub magic: f64,
    pub tags: Vec<String>,
}

#[derive(Debug, Clone, Default)]
pub struct Champions {
    pub version: String,
    by_id: HashMap<i32, ChampionInfo>,
    slug_to_id: HashMap<String, i32>,
    name_to_id: HashMap<String, i32>,
}

impl Champions {
    pub fn from_json(data: &Value, version: impl Into<String>) -> Self {
        let version = version.into();
        let mut by_id = HashMap::new();
        let mut slug_to_id = HashMap::new();
        let mut name_to_id = HashMap::new();
        if let Some(map) = data.get("data").and_then(|v| v.as_object()) {
            for c in map.values() {
                let Some(id) = c
                    .get("key")
                    .and_then(|v| v.as_str())
                    .and_then(|s| s.parse::<i32>().ok())
                else {
                    continue;
                };
                let dd_id = c
                    .get("id")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                let name = c
                    .get("name")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                let image_full = c
                    .get("image")
                    .and_then(|i| i.get("full"))
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                let info = c.get("info");
                let attack = info
                    .and_then(|i| i.get("attack"))
                    .and_then(|v| v.as_f64())
                    .unwrap_or(0.0);
                let magic = info
                    .and_then(|i| i.get("magic"))
                    .and_then(|v| v.as_f64())
                    .unwrap_or(0.0);
                let tags = c
                    .get("tags")
                    .and_then(|v| v.as_array())
                    .map(|a| {
                        a.iter()
                            .filter_map(|t| t.as_str().map(String::from))
                            .collect()
                    })
                    .unwrap_or_default();

                let s = slug_override(&norm(&dd_id)).to_string();
                slug_to_id.insert(s, id);
                name_to_id.insert(norm(&name), id);
                by_id.insert(
                    id,
                    ChampionInfo {
                        id,
                        ddragon_id: dd_id,
                        name,
                        image_full,
                        attack,
                        magic,
                        tags,
                    },
                );
            }
        }
        Self {
            version,
            by_id,
            slug_to_id,
            name_to_id,
        }
    }

    pub fn icon_url(&self, cid: i32) -> String {
        match self.by_id.get(&cid) {
            Some(c) if !c.image_full.is_empty() => {
                format!("{DDRAGON}/cdn/{}/img/champion/{}", self.version, c.image_full)
            }
            _ => String::new(),
        }
    }

    pub fn name(&self, cid: i32) -> String {
        self.by_id
            .get(&cid)
            .map(|c| c.name.clone())
            .unwrap_or_else(|| format!("#{cid}"))
    }

    pub fn slug(&self, cid: i32) -> String {
        let Some(c) = self.by_id.get(&cid) else {
            return String::new();
        };
        slug_override(&norm(&c.ddragon_id)).to_string()
    }

    pub fn id_from_slug(&self, slug: &str) -> Option<i32> {
        let n = norm(slug);
        self.slug_to_id
            .get(&n)
            .or_else(|| self.name_to_id.get(&n))
            .copied()
    }

    pub fn damage(&self, cid: i32) -> (f64, f64) {
        let Some(c) = self.by_id.get(&cid) else {
            return (0.5, 0.5);
        };
        let magic = magic_share_override(&norm(&c.ddragon_id)).unwrap_or_else(|| {
            let sum = c.attack + c.magic;
            if sum > 0.0 {
                c.magic / sum
            } else {
                0.5
            }
        });
        (1.0 - magic, magic)
    }

    pub fn team_damage(&self, cids: &[i32]) -> Option<(f64, f64)> {
        let shares: Vec<(f64, f64)> = cids
            .iter()
            .filter(|cid| self.by_id.contains_key(cid))
            .map(|&cid| self.damage(cid))
            .collect();
        if shares.is_empty() {
            return None;
        }
        let n = shares.len() as f64;
        let ad: f64 = shares.iter().map(|s| s.0).sum();
        let ap: f64 = shares.iter().map(|s| s.1).sum();
        Some((ad / n, ap / n))
    }

    pub fn all_ids(&self) -> Vec<i32> {
        let mut ids: Vec<i32> = self.by_id.keys().copied().collect();
        ids.sort();
        ids
    }

    pub fn contains(&self, cid: i32) -> bool {
        self.by_id.contains_key(&cid)
    }

    pub fn tags(&self, cid: i32) -> &[String] {
        self.by_id
            .get(&cid)
            .map(|c| c.tags.as_slice())
            .unwrap_or(&[])
    }
}
