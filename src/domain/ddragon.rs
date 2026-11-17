use serde_json::Value;
use std::collections::HashMap;

use super::champions::DDRAGON;

pub const CDRAGON_PERKS: &str = "https://raw.communitydragon.org/latest/plugins/rcp-be-lol-game-data/global/default/v1/perk-images/statmods";

pub const SHARDS: &[(i32, &str, &str)] = &[
    (5001, "Health (scaling)", "statmodshealthscalingicon.png"),
    (5002, "Armor", "statmodsarmoricon.png"),
    (5003, "Magic Resist", "statmodsmagicresicon.png"),
    (5005, "Attack Speed", "statmodsattackspeedicon.png"),
    (5007, "Ability Haste", "statmodscdrscalingicon.png"),
    (5008, "Adaptive Force", "statmodsadaptiveforceicon.png"),
    (5010, "Move Speed", "statmodsmovementspeedicon.png"),
    (5011, "Health", "statmodshealthplusicon.png"),
    (5013, "Tenacity and Slow Resist", "statmodstenacityicon.png"),
];

pub const SHARD_ROWS: &[[i32; 3]] = &[
    [5008, 5005, 5007],
    [5008, 5010, 5001],
    [5011, 5013, 5001],
];

#[derive(Debug, Clone)]
pub struct Rune {
    pub id: i32,
    pub name: String,
    pub icon: String,
    pub style: i32,
    pub slot: i32,
}

#[derive(Debug, Clone)]
pub struct Item {
    pub id: i32,
    pub name: String,
    pub icon: String,
    pub gold: i32,
    pub tags: Vec<String>,
    pub into: Vec<i32>,
    pub plaintext: String,
}

#[derive(Debug, Clone, Default)]
pub struct Assets {
    pub version: String,
    pub items: HashMap<i32, Item>,
    pub runes: HashMap<i32, Rune>,
    pub styles: HashMap<i32, Rune>,
    pub trees: HashMap<i32, Vec<Vec<i32>>>,
    pub summoners: HashMap<i32, (String, String)>,
}

impl Assets {
    pub fn from_json(version: impl Into<String>, items: &Value, runes: &Value, summoners: &Value) -> Self {
        let version = version.into();
        let mut item_map = HashMap::new();
        if let Some(data) = items.get("data").and_then(|v| v.as_object()) {
            for (k, v) in data {
                let Ok(id) = k.parse::<i32>() else {
                    continue;
                };
                let name = v.get("name").and_then(|x| x.as_str()).unwrap_or(k).to_string();
                let icon = format!("{DDRAGON}/cdn/{version}/img/item/{k}.png");
                let gold = v
                    .get("gold")
                    .and_then(|g| g.get("total"))
                    .and_then(|x| x.as_i64())
                    .unwrap_or(0) as i32;
                let tags = v
                    .get("tags")
                    .and_then(|x| x.as_array())
                    .map(|a| a.iter().filter_map(|t| t.as_str().map(String::from)).collect())
                    .unwrap_or_default();
                let into = v
                    .get("into")
                    .and_then(|x| x.as_array())
                    .map(|a| {
                        a.iter()
                            .filter_map(|t| t.as_str().and_then(|s| s.parse::<i32>().ok()).or_else(|| t.as_i64().map(|n| n as i32)))
                            .collect()
                    })
                    .unwrap_or_default();
                let plaintext = v
                    .get("plaintext")
                    .and_then(|x| x.as_str())
                    .unwrap_or("")
                    .to_string();
                item_map.insert(
                    id,
                    Item {
                        id,
                        name,
                        icon,
                        gold,
                        tags,
                        into,
                        plaintext,
                    },
                );
            }
        }

        let mut rune_map = HashMap::new();
        let mut style_map = HashMap::new();
        let mut trees = HashMap::new();
        if let Some(arr) = runes.as_array() {
            for style in arr {
                let Some(sid) = style.get("id").and_then(|v| v.as_i64()).map(|n| n as i32) else {
                    continue;
                };
                let sname = style.get("name").and_then(|v| v.as_str()).unwrap_or("").to_string();
                let sicon = style.get("icon").and_then(|v| v.as_str()).unwrap_or("");
                style_map.insert(
                    sid,
                    Rune {
                        id: sid,
                        name: sname,
                        icon: format!("{DDRAGON}/cdn/img/{sicon}"),
                        style: sid,
                        slot: -2,
                    },
                );
                let mut rows = Vec::new();
                if let Some(slots) = style.get("slots").and_then(|v| v.as_array()) {
                    for (i, slot) in slots.iter().enumerate() {
                        let mut row_ids = Vec::new();
                        if let Some(rs) = slot.get("runes").and_then(|v| v.as_array()) {
                            for r in rs {
                                let Some(rid) = r.get("id").and_then(|v| v.as_i64()).map(|n| n as i32) else {
                                    continue;
                                };
                                row_ids.push(rid);
                                let rname = r.get("name").and_then(|v| v.as_str()).unwrap_or("").to_string();
                                let ricon = r.get("icon").and_then(|v| v.as_str()).unwrap_or("");
                                rune_map.insert(
                                    rid,
                                    Rune {
                                        id: rid,
                                        name: rname,
                                        icon: format!("{DDRAGON}/cdn/img/{ricon}"),
                                        style: sid,
                                        slot: i as i32,
                                    },
                                );
                            }
                        }
                        rows.push(row_ids);
                    }
                }
                trees.insert(sid, rows);
            }
        }
        for &(sid, name, icon) in SHARDS {
            rune_map.insert(
                sid,
                Rune {
                    id: sid,
                    name: name.to_string(),
                    icon: format!("{CDRAGON_PERKS}/{icon}"),
                    style: 0,
                    slot: -1,
                },
            );
        }

        let mut summoner_map = HashMap::new();
        if let Some(data) = summoners.get("data").and_then(|v| v.as_object()) {
            for v in data.values() {
                let Some(key) = v
                    .get("key")
                    .and_then(|k| k.as_str())
                    .and_then(|s| s.parse::<i32>().ok())
                else {
                    continue;
                };
                let name = v.get("name").and_then(|x| x.as_str()).unwrap_or("").to_string();
                let full = v
                    .get("image")
                    .and_then(|i| i.get("full"))
                    .and_then(|x| x.as_str())
                    .unwrap_or("");
                summoner_map.insert(
                    key,
                    (name, format!("{DDRAGON}/cdn/{version}/img/spell/{full}")),
                );
            }
        }

        Self {
            version,
            items: item_map,
            runes: rune_map,
            styles: style_map,
            trees,
            summoners: summoner_map,
        }
    }

    pub fn item_info(&self, iid: i32) -> (i32, String, String, String) {
        if let Some(it) = self.items.get(&iid) {
            (iid, it.name.clone(), it.icon.clone(), it.plaintext.clone())
        } else {
            (iid, format!("#{iid}"), String::new(), String::new())
        }
    }

    pub fn rune_info(&self, rid: i32) -> (i32, String, String) {
        if let Some(r) = self.runes.get(&rid).or_else(|| self.styles.get(&rid)) {
            (rid, r.name.clone(), r.icon.clone())
        } else {
            (rid, format!("#{rid}"), String::new())
        }
    }

    pub fn summoner_info(&self, sid: i32) -> (i32, String, String) {
        if let Some((name, icon)) = self.summoners.get(&sid) {
            (sid, name.clone(), icon.clone())
        } else {
            (sid, format!("#{sid}"), String::new())
        }
    }
}
