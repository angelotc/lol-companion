use serde_json::Value;

use super::live::live_position;

pub const SR_MAP: i64 = 11;
pub const SR_QUEUES: &[i64] = &[400, 420, 430, 440, 480, 490, 700, 710];
pub const RANKED_QUEUES: &[(&str, &str)] = &[("RANKED_SOLO_5x5", "solo"), ("RANKED_FLEX_SR", "flex")];
pub const APEX_TIERS: &[&str] = &["MASTER", "GRANDMASTER", "CHALLENGER"];
pub const STREAK: usize = 3;
pub const MAIN_RECENT: u32 = 5;
pub const SEASON_MAIN: u32 = 30;
pub const FIRST_TIME: u64 = 1_500;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlayerRef {
    pub puuid: String,
    pub name: String,
    pub tag: String,
    pub champion_id: i32,
    pub team: i32, // 100 or 200
    pub position: String,
    pub is_me: bool,
}

#[derive(Debug, Clone, Default)]
pub struct PlayerStats {
    pub r: Option<PlayerRef>,
    pub tier: String,
    pub division: String,
    pub lp: i32,
    pub wins: u32,
    pub losses: Option<u32>,
    pub queue: String,
    pub games: u32,
    pub champ_games: u32,
    pub champ_wins: u32,
    pub kills: u32,
    pub deaths: u32,
    pub assists: u32,
    pub recent: Vec<bool>,
    pub mastery: Option<u64>,
    pub season_games: Option<u32>,
    pub season_wins: Option<u32>,
    pub tags: Vec<String>,
    pub error: bool,
}

impl PlayerStats {
    pub fn from_ref(r: PlayerRef) -> Self {
        Self {
            r: Some(r),
            losses: Some(0),
            ..Default::default()
        }
    }

    pub fn kda(&self) -> Option<f64> {
        if self.champ_games == 0 {
            return None;
        }
        Some((self.kills + self.assists) as f64 / self.deaths.max(1) as f64)
    }
}

pub fn parse_teams(session: &Value, my_puuid: &str) -> Vec<PlayerRef> {
    let Some(gd) = session.get("gameData") else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for (team, key) in [(100, "teamOne"), (200, "teamTwo")] {
        let Some(arr) = gd.get(key).and_then(|v| v.as_array()) else {
            continue;
        };
        for p in arr {
            let Some(puuid) = p.get("puuid").and_then(|v| v.as_str()).filter(|s| !s.is_empty()) else {
                continue;
            };
            let name = p
                .get("gameName")
                .or_else(|| p.get("riotIdGameName"))
                .or_else(|| p.get("summonerName"))
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let tag = p
                .get("tagLine")
                .or_else(|| p.get("riotIdTagLine"))
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let champion_id = p.get("championId").and_then(|v| v.as_i64()).unwrap_or(0) as i32;
            let position = live_position(p.get("selectedPosition").and_then(|v| v.as_str()).unwrap_or("")).to_string();
            out.push(PlayerRef {
                puuid: puuid.to_string(),
                name,
                tag,
                champion_id,
                team,
                position,
                is_me: !my_puuid.is_empty() && puuid == my_puuid,
            });
        }
    }
    let mut picks = std::collections::HashMap::new();
    if let Some(arr) = gd.get("playerChampionSelections").and_then(|v| v.as_array()) {
        for c in arr {
            if let Some(puuid) = c.get("puuid").and_then(|v| v.as_str()) {
                let cid = c.get("championId").and_then(|v| v.as_i64()).unwrap_or(0) as i32;
                picks.insert(puuid.to_string(), cid);
            }
        }
    }
    for r in &mut out {
        if r.champion_id == 0
            && let Some(&cid) = picks.get(&r.puuid)
        {
            r.champion_id = cid;
        }
    }
    out
}

pub fn refs_from_live(data: &Value, champion_id: impl Fn(&str) -> Option<i32>) -> Vec<PlayerRef> {
    let ap = data.get("activePlayer");
    let me = ap
        .and_then(|a| a.get("riotId").or_else(|| a.get("summonerName")))
        .and_then(|v| v.as_str())
        .unwrap_or("");
    let Some(players) = data.get("allPlayers").and_then(|v| v.as_array()) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for p in players {
        let rid = p
            .get("riotId")
            .or_else(|| p.get("summonerName"))
            .and_then(|v| v.as_str())
            .unwrap_or("");
        let (part_name, part_tag) = rid.split_once('#').unwrap_or((rid, ""));
        let name = p
            .get("riotIdGameName")
            .and_then(|v| v.as_str())
            .unwrap_or(part_name)
            .to_string();
        let tag = p
            .get("riotIdTagLine")
            .and_then(|v| v.as_str())
            .unwrap_or(part_tag)
            .to_string();
        let cname = p.get("championName").and_then(|v| v.as_str()).unwrap_or("");
        let cid = champion_id(cname).unwrap_or(0);
        let team = if p.get("team").and_then(|v| v.as_str()) == Some("ORDER") {
            100
        } else {
            200
        };
        let pos = live_position(p.get("position").and_then(|v| v.as_str()).unwrap_or("")).to_string();
        out.push(PlayerRef {
            puuid: String::new(),
            name,
            tag,
            champion_id: cid,
            team,
            position: pos,
            is_me: !me.is_empty() && rid == me,
        });
    }
    out
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RankedSummary {
    pub tier: String,
    pub division: String,
    pub lp: i32,
    pub wins: u32,
    pub losses: Option<u32>,
    pub queue: String,
}

fn is_valid_tier(t: &str) -> bool {
    matches!(
        t,
        "IRON"
            | "BRONZE"
            | "SILVER"
            | "GOLD"
            | "PLATINUM"
            | "EMERALD"
            | "DIAMOND"
            | "MASTER"
            | "GRANDMASTER"
            | "CHALLENGER"
    )
}

pub fn parse_ranked(data: &Value) -> Option<RankedSummary> {
    let qmap = data.get("queueMap")?;
    for &(key, label) in RANKED_QUEUES {
        let Some(q) = qmap.get(key) else {
            continue;
        };
        let tier = q
            .get("tier")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_ascii_uppercase();
        if !is_valid_tier(&tier) {
            continue;
        }
        let div = q
            .get("division")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_ascii_uppercase();
        let apex = APEX_TIERS.contains(&tier.as_str());
        let division = if apex || div == "NA" || div == "NONE" {
            String::new()
        } else {
            div
        };
        let lp = q.get("leaguePoints").and_then(|v| v.as_i64()).unwrap_or(0) as i32;
        let wins = q.get("wins").and_then(|v| v.as_u64()).unwrap_or(0) as u32;
        let raw_losses = q.get("losses").and_then(|v| v.as_u64()).unwrap_or(0) as u32;
        let losses = if raw_losses == 0 && !apex && wins > 0 {
            None
        } else {
            Some(raw_losses)
        };
        return Some(RankedSummary {
            tier,
            division,
            lp,
            wins,
            losses,
            queue: label.to_string(),
        });
    }
    None
}

fn counts_game(g: &Value) -> bool {
    if let Some(map_id) = g.get("mapId").and_then(|v| v.as_i64()) {
        let mode = g.get("gameMode").and_then(|v| v.as_str()).unwrap_or("CLASSIC");
        let gtype = g.get("gameType").and_then(|v| v.as_str()).unwrap_or("MATCHED_GAME");
        map_id == SR_MAP && mode == "CLASSIC" && gtype == "MATCHED_GAME"
    } else {
        let qid = g.get("queueId").and_then(|v| v.as_i64()).unwrap_or(0);
        SR_QUEUES.contains(&qid)
    }
}

#[derive(Debug, Clone, Default)]
pub struct HistorySummary {
    pub games: u32,
    pub recent: Vec<bool>,
    pub champ_games: u32,
    pub champ_wins: u32,
    pub kills: u32,
    pub deaths: u32,
    pub assists: u32,
}

pub fn summarize_history(data: &Value, champion_id: i32) -> HistorySummary {
    let mut games: Vec<&Value> = data
        .get("games")
        .and_then(|g| g.get("games"))
        .and_then(|g| g.as_array())
        .map(|arr| {
            arr.iter()
                .filter(|g| g.is_object() && counts_game(g))
                .collect()
        })
        .unwrap_or_default();

    games.sort_by(|a, b| {
        let ca = a.get("gameCreation").and_then(|v| v.as_i64()).unwrap_or(0);
        let cb = b.get("gameCreation").and_then(|v| v.as_i64()).unwrap_or(0);
        cb.cmp(&ca)
    });

    let mut out = HistorySummary::default();
    for g in games {
        let Some(parts) = g.get("participants").and_then(|v| v.as_array()) else {
            continue;
        };
        if parts.len() != 1 {
            continue;
        }
        let p = &parts[0];
        let st = p.get("stats");
        let win = st
            .and_then(|s| s.get("win"))
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        out.games += 1;
        out.recent.push(win);
        let cid = p.get("championId").and_then(|v| v.as_i64()).unwrap_or(0) as i32;
        if cid == champion_id {
            out.champ_games += 1;
            if win {
                out.champ_wins += 1;
            }
            out.kills += st.and_then(|s| s.get("kills")).and_then(|v| v.as_u64()).unwrap_or(0) as u32;
            out.deaths += st.and_then(|s| s.get("deaths")).and_then(|v| v.as_u64()).unwrap_or(0) as u32;
            out.assists += st.and_then(|s| s.get("assists")).and_then(|v| v.as_u64()).unwrap_or(0) as u32;
        }
    }
    out
}

pub fn parse_mastery(data: &Value, champion_id: i32) -> Option<u64> {
    let arr = data.as_array()?;
    for m in arr {
        let cid = m.get("championId").and_then(|v| v.as_i64()).unwrap_or(0) as i32;
        if cid == champion_id {
            return Some(m.get("championPoints").and_then(|v| v.as_u64()).unwrap_or(0));
        }
    }
    Some(0)
}

pub fn compute_tags(s: &PlayerStats) -> Vec<String> {
    let mut out = Vec::new();
    if s.champ_games == 0
        && s.season_games.unwrap_or(0) == 0
        && s.mastery.is_some_and(|m| m < FIRST_TIME)
    {
        out.push("First time".to_string());
    } else if s.champ_games >= MAIN_RECENT || s.season_games.unwrap_or(0) >= SEASON_MAIN {
        out.push("Main".to_string());
    }
    if let Some(&first) = s.recent.first() {
        let run = s.recent.iter().take_while(|&&w| w == first).count();
        if run >= STREAK {
            out.push(format!(
                "{run} {} in a row",
                if first { "wins" } else { "losses" }
            ));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_parse_teams_maps_teams_lanes_me_and_picks() {
        let session = json!({"gameData": {
            "teamOne": [
                {"puuid": "a", "gameName": "Ann", "tagLine": "NA1", "championId": 0, "selectedPosition": "UTILITY"},
                {"puuid": "b", "riotIdGameName": "Bob", "riotIdTagLine": "X", "championId": 5, "selectedPosition": "TOP"},
                {"gameName": "NoPuuid", "championId": 9, "selectedPosition": "JUNGLE"},
            ],
            "teamTwo": [
                {"puuid": "c", "summonerName": "Cy", "championId": 0, "selectedPosition": "BOTTOM"},
            ],
            "playerChampionSelections": [{"puuid": "a", "championId": 99}, {"puuid": "c", "championId": 77}],
        }});
        let refs = parse_teams(&session, "b");
        assert_eq!(refs.len(), 3);
        assert_eq!(refs.iter().map(|r| r.team).collect::<Vec<_>>(), vec![100, 100, 200]);
        assert_eq!(refs[0].position, "support");
        assert_eq!(refs[0].champion_id, 99);
        assert!(refs[1].is_me);
        assert_eq!(refs[2].champion_id, 77);
    }

    #[test]
    fn test_parse_ranked_cases() {
        let solo = json!({"queueMap": {"RANKED_SOLO_5x5": {"tier": "gold", "division": "ii", "leaguePoints": 5, "wins": 3, "losses": 2}}});
        let r = parse_ranked(&solo).unwrap();
        assert_eq!(r.tier, "GOLD");
        assert_eq!(r.division, "II");
        assert_eq!(r.queue, "solo");

        let apex = json!({"queueMap": {"RANKED_SOLO_5x5": {"tier": "MASTER", "division": "I", "leaguePoints": 150}}});
        let r = parse_ranked(&apex).unwrap();
        assert_eq!(r.division, "");
        assert_eq!(r.lp, 150);

        assert!(parse_ranked(&json!({"queueMap": {}})).is_none());
    }

    #[test]
    fn test_summarize_history_filters() {
        let game = |qid: i64, created: i64, champ: i32, win: bool, k: u32, d: u32, a: u32| {
            json!({"queueId": qid, "gameCreation": created, "participants": [{"championId": champ, "stats": {"win": win, "kills": k, "deaths": d, "assists": a}}]})
        };
        let data = json!({"games": {"games": [
            game(450, 900, 1, true, 30, 0, 0),
            game(420, 100, 1, false, 2, 4, 6),
            game(420, 200, 1, true, 5, 1, 3),
            game(420, 150, 2, true, 9, 9, 9),
        ]}});
        let out = summarize_history(&data, 1);
        assert_eq!(out.games, 3);
        assert_eq!(out.recent, vec![true, true, false]);
        assert_eq!(out.champ_games, 2);
        assert_eq!(out.champ_wins, 1);
        assert_eq!((out.kills, out.deaths, out.assists), (7, 5, 9));
    }
}
