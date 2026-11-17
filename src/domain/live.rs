use serde_json::Value;
use std::collections::HashMap;

pub fn live_position(pos: &str) -> &'static str {
    match pos.to_ascii_uppercase().as_str() {
        "TOP" => "top",
        "JUNGLE" => "jungle",
        "MIDDLE" => "middle",
        "BOTTOM" => "bottom",
        "UTILITY" => "support",
        _ => "",
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct LiveGame {
    pub champion: String,
    pub role: String,
    pub level: i32,
    pub gold: f64,
    pub items: Vec<i32>,
    pub game_time: f64,
    pub allies: HashMap<String, String>,
    pub enemies: HashMap<String, String>,
}

impl LiveGame {
    /// Compare ignoring `game_time` (and tiny gold fluctuations if desired) so the 2-second poll
    /// doesn't trigger full rebuilds when only the clock ticked.
    pub fn same_static_state(&self, other: &Self) -> bool {
        self.champion == other.champion
            && self.role == other.role
            && self.level == other.level
            && self.items == other.items
            && self.allies == other.allies
            && self.enemies == other.enemies
            && (self.gold as i64) == (other.gold as i64)
    }
}

pub fn parse_live(data: &Value) -> Option<LiveGame> {
    let ap = data.get("activePlayer")?;
    let players = data.get("allPlayers")?.as_array()?;
    let me_id = ap
        .get("riotId")
        .or_else(|| ap.get("summonerName"))
        .and_then(|v| v.as_str())?;
    let me = players.iter().find(|p| {
        p.get("riotId")
            .or_else(|| p.get("summonerName"))
            .and_then(|v| v.as_str())
            == Some(me_id)
    })?;
    let team = me.get("team").and_then(|v| v.as_str());
    let mut allies = HashMap::new();
    let mut enemies = HashMap::new();

    for p in players {
        if std::ptr::eq(p, me) {
            continue;
        }
        let lane = live_position(p.get("position").and_then(|v| v.as_str()).unwrap_or(""));
        if lane.is_empty() {
            continue;
        }
        let cname = p
            .get("championName")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        if p.get("team").and_then(|v| v.as_str()) == team {
            allies.insert(lane.to_string(), cname);
        } else {
            enemies.insert(lane.to_string(), cname);
        }
    }

    let mut item_rows: Vec<(i64, i32)> = me
        .get("items")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|i| {
                    let id = i.get("itemID").and_then(|v| v.as_i64()).unwrap_or(0) as i32;
                    if id == 0 {
                        return None;
                    }
                    let slot = i.get("slot").and_then(|v| v.as_i64()).unwrap_or(0);
                    Some((slot, id))
                })
                .collect()
        })
        .unwrap_or_default();
    item_rows.sort_by_key(|&(slot, _)| slot);
    let items: Vec<i32> = item_rows.into_iter().map(|(_, id)| id).collect();

    let champion = me
        .get("championName")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    let role = live_position(me.get("position").and_then(|v| v.as_str()).unwrap_or("")).to_string();
    let level = ap
        .get("level")
        .or_else(|| me.get("level"))
        .and_then(|v| v.as_i64())
        .unwrap_or(0) as i32;
    let gold = ap
        .get("currentGold")
        .and_then(|v| v.as_f64())
        .unwrap_or(0.0);
    let game_time = data
        .get("gameData")
        .and_then(|g| g.get("gameTime"))
        .and_then(|v| v.as_f64())
        .unwrap_or(0.0);

    Some(LiveGame {
        champion,
        role,
        level,
        gold,
        items,
        game_time,
        allies,
        enemies,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn test_parse_live_fixture() {
        let p = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/live.json");
        let txt = std::fs::read_to_string(p).unwrap();
        let val: Value = serde_json::from_str(&txt).unwrap();
        let g = parse_live(&val).unwrap();
        assert!(!g.champion.is_empty());
    }
}
