# LoL Companion

A window that ranks your best picks in champ select, using matchup and synergy stats, your team and
theirs, and your own record. It is one 16:9 window that shows the blind-pick explorer between games
and bans, picks and build as the draft moves on; in game, a second window over the game's right edge
shows your build beside all 10 players.

![Champ select, best picks: your best picks top to bottom, every matchup and synergy beside them](docs/views/picks.png)

## Install
Download **[LoLCompanion-win-Setup.exe](https://github.com/angelotc/lol-companion/releases/latest/download/LoLCompanion-win-Setup.exe)**
and run it. It installs for your Windows account (no admin) and updates itself.

Portable instead: [LoLCompanion.zip](https://github.com/angelotc/lol-companion/releases/latest/download/LoLCompanion.zip)
(unzip, run `LoLCompanion.exe`; no auto-updates).

Windows may warn that the app is unrecognized (it isn't code-signed): **More info → Run anyway**.

## Use
The panel is a normal window: it opens with the app and stays open until you close it. It is one
window for every stage, client-sized (1280×720) on the right edge of your main monitor the first
time, then wherever you leave it. It keeps its 16:9 shape: drag any edge or corner and it resizes as
a whole, and everything in it scales with it. Maximized or snapped, it fills the space and zooms to
the shorter side. Closing it hides it to the tray; a new champ select brings a hidden panel back.
Between games it shows the blind-pick explorer; in champ select, four views under one tab strip
(below).

| Key | Does |
|---|---|
| **Ctrl+X** | Toggle the panel (client or panel in front) or the in-game window (in game) |
| **Ctrl+Shift+L** | Toggle the panel from any window |

The **tray icon** (bottom-right) shows/hides the panel, checks for updates, opens settings, turns
on Start with Windows, and quits.

### 1 · Champ select

Your team, the draft win estimate with its main reasons, and the enemy team stay across the top.
Under them, one view at a time: the step row **follows the draft** (1 while bans last, 2 while your
lane opponent is unknown, 3 once they're picked, 4 after you lock in); steps that don't apply are
greyed out. Click another step to look at it; a **DRAFT IS HERE** tag marks where the draft is, and
the next phase brings you back. The step row also holds the bans and the enemy team's damage split.
Every view is a table: candidates top to bottom, best first, and beside each one the numbers behind
its score. Hover a cell for its games; the column under the pointer lights up.

#### 1 · Ban phase

<img src="docs/views/bans.png" alt="Bans: ban candidates with how your likely picks and your team's hovers do against each">

Enemy champions worth banning (how likely you face it × how much it hurts you), with each one's
strength, then one column per likely pick of yours and per teammate's hover: how that pick's win
rate moves against it, shrunk by games and weighted by lane (coral = it beats you). The panel on the
right explains the top ban: how often it's picked, who it hurts most, and what comes next.

#### 2 · Blind pick (while your lane opponent is unknown)

<img src="docs/views/blind.png" alt="Blind pick: blind-safe picks with strength, team fit and counter risk">

Picks ranked by strength, fit with your team, and how much they lose if the enemy laner counters
them: a track from if countered (the worst matchups that make up 15% of their picks) to the average
matchup, the blind risk that adds up to, the worst counters, and how many matchups have data. Tune
it in `config.toml` under `[blind]`.

#### 3 · Best picks

The screenshot at the top. Picks ranked against the enemies picked so far and with your team: their
strength, then one column per enemy (**against**) and teammate (**with**) showing what that matchup
or duo adds to the score (shrunk by games and weighted by lane), and a **you** column for your own
games and mastery, so a row's cells add up to its score. Click a pick for its breakdown on the
right: the raw win-rate change behind every term. After you
lock in, these stay as they were at lock-in.

#### 4 · Build (after lock-in)

<img src="docs/views/build.png" alt="Build: rune page, spells, skills and items with their edge vs your lane">

The rune page (most picked or highest win rate), laid out like the client's with the chosen runes
lit, summoner spells, skill order and items for your champion, each with its Δ vs your lane. Hover
any rune, chosen or not, for its win rate and Δ. Each item slot shows its top options; hover "+N more" for
the rest.

### 2 · In game

<img src="docs/views/ingame.png" width="700" alt="In game: your build beside all 10 players">

Your build (next item, gold, win rate and Δ vs each enemy lane) beside all 10 players.

### 3 · Out of game

<img src="docs/views/explore.png" alt="The blind-pick explorer">

The blind-pick explorer, between games (lobby, queue, or with no draft at all): every champion's
strength vs how much it loses if countered, for any role (your most played by default). Click a
portrait, or a row in the best blind picks list, for its verdict and worst counters.

- **Before champ select** stats preload in the background for your usual roles (your lobby roles once you queue).

- **Picks** are ranked from the moment champ select opens (blind pick while your lane opponent is unknown). Every term behind a pick's score is in its row.
- **Rank tier**: stats are from Master+ games.
- **Enemy lanes** are guessed from play rates; flex picks get a gold lane label with a "?". Click an enemy to set their lane.
- **Build:** each option shows its Δ: how much better it does against these enemies than usual, in win-rate points, discounted for small samples. Bot lane is measured vs the enemy duo, top/mid vs their laner, jungler (and mid vs the roaming support), jungle vs everyone. The best is tagged **BEST VS LANE**. Hover for each enemy's share.
- **Their damage** (step row and in game): the enemy team's split of physical, magic and true damage, from each enemy's average damage to champions in its lane. Hover for each enemy's own split.
- **In game** (Ctrl+X hides it) one window pinned to the game's right edge shows the players and your build side by side; it grows with the game's resolution (`ingame_scale` in the config enlarges it). It never takes focus from the game and needs borderless or windowed mode. Build: each item's win rate and its Δ vs each enemy, one column per lane (enemy icons on top); boots and item slots you've already filled are hidden; hover an item or row for details. Players: rank and season win rate, games/win rate/KDA on the champion they're playing (from their last ~20 games), last 10 results, and tags like "First time" or "3 losses in a row", read from the League client when the game starts.

## Settings
Tray → Settings (or the gear in the panel's title bar). Saved changes apply immediately. They live in `config.toml` (installed copy in `%LOCALAPPDATA%\league-analysis\`), which also holds what the window doesn't cover:

| Key | Default | |
|---|---|---|
| `min_pick_rate` | 1.0 | Min pick rate % to be a candidate. Lower for off-meta picks. |
| `comfort_only` | false | Only recommend champions you've played. |
| `[comfort]` | | `weight` (0 = off), `k`, `history`, `half_life`, `min_games`, `mastery`, `mastery_cap` |
| `[weights.<role>]` | | Per-role lane weights. |
| `[build_weights.<role>]` | | Which enemies build Δs are measured against, and their weights. |
| `build_min_games` | 20 | Ignore an enemy's Δ for an option below this many games. |
| `ingame_scale` | 1.0 | In-game window size; it already grows with the game's resolution. |
| `[odds]` | | Draft win estimate weights. |
| `record_games` | false | Save the raw in-game data every `record_every` (15) seconds to `recordings\` in the data folder, one `.jsonl` file per game. Research only; nothing changes on screen. |

## How picks are scored
One score per candidate, in win-rate points:

```
score(c) = base + Σ vs + Σ with + you + mastery

base    = shrink(wr(c) − avg_wr, games(c), k)                   × w_base
vs      = shrink(Δ(c vs enemy in lane L), games, k)             × w_enemy[L]   each known enemy
with    = shrink(Δ(c with ally in lane L), games, k)            × w_ally[L]    each ally (× 0.5 if hovered)
you     = shrink(your_wr(c) − wr(c), your_games(c), 40) × 0.5   × w_comfort   (3+ games)
mastery = min(0.15 × ln(1 + points / 1000), 1.0)                × w_comfort

shrink(δ, n, k) = δ × n / (n + k)     k = 500: a 500-game sample counts half
your games are weighted 0.5^(age / half_life), half_life = 90 days
```

- **Δ**: lolalytics' normalized delta: how much better `c` does with/against a champion than
  both champions' win rates predict.
- **wr, games, avg_wr**: your role, your tier, last 14 days.

| Lane weight | your lane | duo lane (bot/sup) | jungle | other |
|---|---|---|---|---|
| `w_enemy` | 1.0 | 1.0 | 0.5 | 0.3 |
| `w_ally` | – | 1.0 | 0.5 | 0.3 |

- **Ban threat** = `pick_rate × (harm to your picks + harm to each teammate's hover + strength × w_enemy)`, summed over lanes. Harm to your picks is averaged (you play one); each hover counts in full, with that teammate's own lane weights.
- **Build Δ** = `shrink((wr_option_vs − wr_vs) − (wr_option − wr), games, 300)`, hidden under 20 games (`build_min_games`).
- **Draft win estimate**: champion strength (half weight) + cross-team matchups (1.0 same lane, 0.7
  bot duos, 0.35 other) + synergy (half weight), capped 25–75%. Calibrate it on your own drafts with
  `scripts/backtest.py`, which fits `cal_a`/`cal_b` for `[odds]` in the config (the estimate then
  passes through `sigmoid(cal_a · total + cal_b)` instead of the cap).

## APIs
| API | Used for |
|---|---|
| [League Client API](https://developer.riotgames.com/docs/lol#league-client-api) (local, read-only) | Champ select (`/lol-champ-select/v1/session`, pickable champions), game phase, lobby, match history, mastery, patch version. In game: every player's rank, recent games and mastery, the same public profile data the client shows |
| [Live Client Data API](https://developer.riotgames.com/docs/lol#game-client-api) (`127.0.0.1:2999`) | In game: your champion, items, gold, level, and each player's champion/position. No enemy items or timers. |
| [lolalytics](https://lolalytics.com) (unofficial, may change) | Win rates, matchups, synergies, builds. Cached 24h. |
| [op.gg](https://op.gg) (unofficial, may change) | In-game players: each player's season solo queue record on their champion. Cached 6h; `opgg = false` in config turns it off. |
| [Data Dragon](https://ddragon.leagueoflegends.com) + [CommunityDragon](https://raw.communitydragon.org) | Champion, item and rune data and images |
| GitHub Releases (via [Velopack](https://velopack.io)) | Auto-updates |

Nothing is injected into the client or the game.

## Performance: Python + WebView2 vs. Native Rust

Rewritten from Python + `pywebview` (Edge WebView2) to native Rust (`egui` / `eframe`). Measured on Windows 11 with a 5-second settle and 5-second sample:

| Runtime | Processes | Working Set | Private Mem | Idle CPU (% of 1 core) |
|---|---:|---:|---:|---:|
| **Python + WebView2** *(live tree, idle)* | 8 | 149.5–309.7 MB | 580.4–641.5 MB | 7.17% |
| **Rust (`egui`)** — Out of game *(draft)* | 1 | 126.9 MB | 116.4 MB | 3.43% (`0.21%` total) |
| **Rust (`egui`)** — In game *(overlay)* | 1 | 107.1 MB | 120.5 MB | 0.00% |

The Rust build ships as a single native executable with zero Edge WebView2 child processes, ~5× lower private memory, and 0% idle CPU overhead while the in-game overlay is open.

## Develop
```powershell
cargo run --release                                               # live
cargo run -- --record drafts/x.jsonl                              # live + record
cargo run -- --replay drafts/x.jsonl                              # replay
cargo run -- --simulate --role support --allies jinx --enemies nautilus
cargo run -- --simulate --role support --items 3158,3109          # in-game window: build for items held
cargo run -- --simulate --role support --players                  # in-game window: players from tests/fixtures/players
cargo test
cargo build --release
cargo install --path . --force
```

Every push to `main` runs `cargo test`, builds the release binary, and publishes version `0.1.<run>` (Velopack installer + update feed) plus the portable zip.

| Path | |
|---|---|
| [src/main.rs](src/main.rs) | Entry point: `eframe` windows, deferred overlay viewport, CLI simulation/replay, hotkeys |
| [src/domain/](src/domain/) | Config (`config.toml`), LCU/live state parsing, builds, scoring, blind-pick report |
| [src/data/](src/data/) | League client (`lcu.rs`), lolalytics, Data Dragon, op.gg, HTTP/disk cache, SQLite `store.rs` |
| [src/engine/](src/engine/) | `controller.rs` (client updates → screen updates), `recommender.rs`, `views.rs` |
| [src/ui/](src/ui/) | Native `egui` views: `client_view.rs`, `explore_view.rs`, `bans_view.rs`, `picks_view.rs`, `build_view.rs`, `game_view.rs`, `settings_view.rs` |
| [src/win32.rs](src/win32.rs), [src/tray.rs](src/tray.rs), [src/images.rs](src/images.rs) | Win32 window placement/16:9 lock/hotkeys, system tray, async image cache |
