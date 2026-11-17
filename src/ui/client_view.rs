use egui::{Align, Color32, CornerRadius, Layout, RichText, Sense, Stroke, Ui, Vec2};
use std::sync::Arc;

use crate::domain::scoring::LANES;
use crate::engine::controller::Controller;
use crate::engine::views::{AllySlotView, ClientScreen, EnemySlotView, OddsView, PicksView, TIERS};

use crate::images::ImageCache;
use crate::ui::bans_view::render_bans;
use crate::ui::build_view::{BuildUiState, render_build};
use crate::ui::explore_view::{ExploreUiState, render_explore};
use crate::ui::picks_view::{PicksUiState, render_best_picks, render_blind_picks};
use crate::ui::theme::{AD, ALLY, ALLY_TEXT, AP, BG, BG2, DIM, ENEMY, ENEMY_TEXT, GOLD, LINE2, MUTED, NEG, PANEL, POS, RAISED, TEXT, fmt_signed, short_lane};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DraftTab {
    Bans,
    Blind,
    Best,
    Build,
}

pub struct ClientUiState {
    pub chosen_tab: Option<DraftTab>,
    pub last_auto: Option<DraftTab>,
    pub lane_picker_enemy: Option<i32>,
    pub picks: PicksUiState,
    pub build: BuildUiState,
    pub explore: ExploreUiState,
}

impl Default for ClientUiState {
    fn default() -> Self {
        Self {
            chosen_tab: None,
            last_auto: None,
            lane_picker_enemy: None,
            picks: PicksUiState::default(),
            build: BuildUiState::default(),
            explore: ExploreUiState::default(),
        }
    }
}

fn cap_first(s: &str) -> String {
    let mut c = s.chars();
    match c.next() {
        None => String::new(),
        Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
    }
}

pub fn render_client(
    ui: &mut Ui,
    s: &ClientScreen,
    st: &mut ClientUiState,
    ctl: &Arc<Controller>,
    imgs: &mut ImageCache,
    open_settings: &mut bool,
) {
    // 1. Title bar
    egui::Frame::new()
        .fill(BG)
        .inner_margin(egui::Margin::symmetric(14, 7))
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label(RichText::new("LOL COMPANION").size(12.5).strong().color(GOLD));
                let mut parts = Vec::new();
                if !s.header.role.is_empty() && !(s.explore && s.picks.is_none()) {
                    parts.push(cap_first(&s.header.role));
                }
                if !s.header.tier.is_empty() {
                    parts.push(s.header.tier.clone());
                }
                parts.push(format!("last {} days", s.header.days));
                ui.label(RichText::new(parts.join(" · ")).size(11.5).color(MUTED));

                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if ui
                        .add(egui::Button::new(RichText::new("⚙").size(14.0).color(MUTED)).frame(false))
                        .on_hover_text("Settings")
                        .clicked()
                    {
                        *open_settings = true;
                    }
                    ui.label(RichText::new("hide").size(10.5).color(DIM));
                    egui::Frame::new()
                        .stroke(Stroke::new(1.0, LINE2))
                        .corner_radius(CornerRadius::same(3))
                        .inner_margin(egui::Margin::symmetric(4, 1))
                        .show(ui, |ui| {
                            ui.label(RichText::new("Ctrl+X").size(10.0).strong().color(MUTED));
                        });

                    if s.explore && !s.message.is_empty() && s.picks.is_none() {
                        ui.add_space(6.0);
                        let txt = if s.detail.is_empty() {
                            s.message.clone()
                        } else {
                            format!("{} · {}", s.message, s.detail)
                        };
                        ui.label(RichText::new(txt).size(11.0).color(MUTED));
                        let dot_col = if s.status == "error" { NEG } else { GOLD };
                        let (r, _) = ui.allocate_exact_size(Vec2::splat(7.0), Sense::hover());
                        ui.painter().circle_filled(r.center(), 3.5, dot_col);
                    } else if !s.updated_hms.is_empty() && s.picks.is_some() {
                        ui.add_space(6.0);
                        ui.label(
                            RichText::new(format!("lolalytics · updated {}", s.updated_hms))
                                .size(10.5)
                                .color(MUTED),
                        );
                    }

                    if TIERS.len() > 1 {
                        for &(tid, label) in TIERS {
                            let on = s.header.tier_id == tid;
                            if ui.selectable_label(on, label).clicked() && !on {
                                let ctl_c = Arc::clone(ctl);
                                let t = tid.to_string();
                                tokio::spawn(async move {
                                    ctl_c.set_tier(&t).await;
                                });
                            }
                        }
                    }
                });
            });
        });
    ui.separator();

    // 2. Draft bar (when in champ select with picks)
    if let Some(p) = &s.picks {
        render_draft_bar(ui, p, &s.header.role, st, ctl, imgs);
        ui.separator();
    }

    // 3. Step tabs row
    let has_bans = s.bans.is_some();
    let has_blind = s
        .picks
        .as_ref()
        .is_some_and(|p| !p.blind.is_empty() && p.locked.is_none());
    let has_best = s.picks.is_some();
    let has_build = s.build.is_some();

    let auto_tab = if has_build {
        Some(DraftTab::Build)
    } else if has_bans {
        Some(DraftTab::Bans)
    } else if has_blind {
        Some(DraftTab::Blind)
    } else if has_best {
        Some(DraftTab::Best)
    } else {
        None
    };

    if auto_tab != st.last_auto {
        st.last_auto = auto_tab;
        st.chosen_tab = None;
    }
    if let Some(ch) = st.chosen_tab {
        let valid = match ch {
            DraftTab::Bans => has_bans,
            DraftTab::Blind => has_blind,
            DraftTab::Best => has_best,
            DraftTab::Build => has_build,
        };
        if !valid {
            st.chosen_tab = None;
        }
    }
    let active_tab = st.chosen_tab.or(auto_tab);

    if auto_tab.is_some() {
        egui::Frame::new()
            .fill(BG)
            .inner_margin(egui::Margin::symmetric(14, 5))
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    let tabs = [
                        (DraftTab::Bans, "1", "Bans", has_bans),
                        (DraftTab::Blind, "2", "Blind pick", has_blind),
                        (DraftTab::Best, "3", "Best picks", has_best),
                        (
                            DraftTab::Build,
                            "4",
                            if has_build { "Build" } else { "Build · after lock-in" },
                            has_build,
                        ),
                    ];
                    for (tab, num, lbl, enabled) in tabs {
                        let active = active_tab == Some(tab);
                        let is_auto = auto_tab == Some(tab);
                        ui.add_enabled_ui(enabled, |ui| {
                            let resp = ui.horizontal(|ui| {
                                ui.label(
                                    RichText::new(num)
                                        .size(11.0)
                                        .color(if active { GOLD } else if enabled { MUTED } else { DIM }),
                                );
                                ui.label(
                                    RichText::new(lbl)
                                        .size(12.0)
                                        .strong()
                                        .color(if active { TEXT } else if enabled { MUTED } else { DIM }),
                                );
                                if is_auto {
                                    egui::Frame::new()
                                        .fill(GOLD)
                                        .corner_radius(CornerRadius::same(8))
                                        .inner_margin(egui::Margin::symmetric(6, 1))
                                        .show(ui, |ui| {
                                            ui.label(RichText::new("DRAFT IS HERE").size(9.5).strong().color(BG));
                                        });
                                }
                            })
                            .response
                            .interact(Sense::click());
                            if active {
                                let y = resp.rect.bottom() + 4.0;
                                ui.painter().line_segment(
                                    [egui::pos2(resp.rect.left(), y), egui::pos2(resp.rect.right(), y)],
                                    Stroke::new(2.0, GOLD),
                                );
                            }
                            if resp.clicked() && enabled {
                                st.chosen_tab = if is_auto { None } else { Some(tab) };
                            }
                        });
                        ui.add_space(10.0);
                    }

                    if let Some(p) = &s.picks {
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            for b in p.bans.iter().rev() {
                                imgs.paint_icon(ui, &b.name, &b.icon, Vec2::splat(22.0), 4.0, Color32::from_rgba_unmultiplied(255, 255, 255, 170))
                                    .on_hover_text(format!("Banned: {}", b.name));
                            }
                            if !p.bans.is_empty() {
                                ui.label(RichText::new("Bans").size(10.5).color(MUTED));
                            }
                            if let Some(ad) = p.enemy_ad_pct {
                                ui.add_space(12.0);
                                let ap = 100 - ad;
                                ui.label(RichText::new(format!("{ap}% AP")).size(10.5).strong().color(AP));
                                ui.label(RichText::new("·").size(10.5).color(DIM));
                                ui.label(RichText::new(format!("{ad}% AD")).size(10.5).strong().color(AD));
                                let (bar_r, resp) = ui.allocate_exact_size(Vec2::new(72.0, 5.0), Sense::hover());
                                let w_ad = bar_r.width() * (ad as f32 / 100.0);
                                ui.painter().rect_filled(
                                    egui::Rect::from_min_size(bar_r.min, Vec2::new(w_ad, bar_r.height())),
                                    CornerRadius::same(2),
                                    AD,
                                );
                                ui.painter().rect_filled(
                                    egui::Rect::from_min_size(
                                        egui::pos2(bar_r.left() + w_ad, bar_r.top()),
                                        Vec2::new(bar_r.width() - w_ad, bar_r.height()),
                                    ),
                                    CornerRadius::same(2),
                                    AP,
                                );
                                resp.on_hover_text(format!("Enemy damage estimate: {ad}% AD / {ap}% AP"));
                                ui.label(RichText::new("Their damage").size(10.5).color(MUTED));
                            }
                        });
                    }
                });
            });
        ui.separator();
    }

    // 4. Main content area
    egui::Frame::new()
        .fill(BG)
        .inner_margin(egui::Margin::symmetric(12, 8))
        .show(ui, |ui| {
            let content_h = ui.available_height().max(160.0);
            ui.allocate_ui(Vec2::new(ui.available_width(), content_h), |ui| {
                if !s.message.is_empty() && !s.explore && active_tab.is_none() {
                    ui.centered_and_justified(|ui| {
                        ui.vertical_centered(|ui| {
                            let col = if s.status == "error" { NEG } else { TEXT };
                            ui.label(RichText::new(&s.message).size(14.0).color(col));
                            if !s.detail.is_empty() {
                                ui.label(RichText::new(&s.detail).size(11.5).color(MUTED));
                            }
                        });
                    });
                } else if let Some(tab) = active_tab {
                    match tab {
                        DraftTab::Bans => {
                            if let Some(b) = &s.bans {
                                render_bans(ui, b, imgs);
                            }
                        }
                        DraftTab::Blind => {
                            if let Some(p) = &s.picks {
                                render_blind_picks(ui, p, imgs);
                            }
                        }
                        DraftTab::Best => {
                            if let Some(p) = &s.picks {
                                render_best_picks(ui, p, &mut st.picks, imgs);
                            }
                        }
                        DraftTab::Build => {
                            if let Some(b) = &s.build {
                                render_build(ui, b, &mut st.build, imgs);
                            }
                        }
                    }
                } else if s.explore {
                    render_explore(ui, ctl, &s.header.tier_id, &mut st.explore, imgs);
                }
            });
        });
}

fn place_five<T: Clone>(items: &[T], lane_fn: impl Fn(&T) -> &str) -> [Option<T>; 5] {
    let mut out: [Option<T>; 5] = [None, None, None, None, None];
    let mut rest = Vec::new();
    let short_idx = |l: &str| match short_lane(l) {
        "TOP" => Some(0),
        "JG" => Some(1),
        "MID" => Some(2),
        "BOT" => Some(3),
        "SUP" => Some(4),
        _ => None,
    };
    for it in items {
        if let Some(i) = short_idx(lane_fn(it))
            && out[i].is_none()
        {
            out[i] = Some(it.clone());
        } else {
            rest.push(it.clone());
        }
    }
    for it in rest {
        if let Some(slot) = out.iter_mut().find(|s| s.is_none()) {
            *slot = Some(it);
        }
    }
    out
}

fn render_draft_bar(
    ui: &mut Ui,
    p: &PicksView,
    role: &str,
    st: &mut ClientUiState,
    ctl: &Arc<Controller>,
    imgs: &mut ImageCache,
) {
    let mut allies = p.allies.clone();
    if !allies.iter().any(|a| a.is_me) {
        allies.push(AllySlotView {
            champ: crate::engine::views::ChampRef {
                id: 0,
                name: String::new(),
                icon: String::new(),
            },
            lane: role.to_string(),
            tentative: false,
            is_me: true,
        });
    }
    let ally_slots = place_five(&allies, |a| &a.lane);
    let enemy_slots = place_five(&p.enemies, |e| &e.lane);

    if st.lane_picker_enemy.is_some_and(|id| !p.enemies.iter().any(|e| e.champ.id == id)) {
        st.lane_picker_enemy = None;
    }

    egui::Frame::new()
        .fill(BG2)
        .inner_margin(egui::Margin::symmetric(12, 6))
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                // Allies
                for (i, slot) in ally_slots.iter().enumerate() {
                    paint_ally_slot(ui, slot.as_ref(), i, imgs);
                }

                // Odds center bar (enemies occupy ~255px on the right)
                let rem_w = (ui.available_width() - 260.0).max(160.0);
                ui.allocate_ui(Vec2::new(rem_w, 58.0), |ui| {
                    render_odds(ui, p.odds.as_ref(), &p.note);
                });

                // Enemies
                for (i, slot) in enemy_slots.iter().enumerate() {
                    if let Some(e) = slot {
                        let clicked = paint_enemy_slot(ui, Some(e), i, st.lane_picker_enemy == Some(e.champ.id), imgs);
                        if clicked {
                            st.lane_picker_enemy = if st.lane_picker_enemy == Some(e.champ.id) {
                                None
                            } else {
                                Some(e.champ.id)
                            };
                        }
                    } else {
                        paint_enemy_slot(ui, None, i, false, imgs);
                    }
                }
            });

            // Enemy lane picker inline drawer when an enemy is selected
            if let Some(eid) = st.lane_picker_enemy
                && let Some(e) = p.enemies.iter().find(|e| e.champ.id == eid)
            {
                ui.add_space(4.0);
                egui::Frame::new()
                    .fill(PANEL)
                    .stroke(Stroke::new(1.0, LINE2))
                    .corner_radius(CornerRadius::same(6))
                    .inner_margin(egui::Margin::symmetric(10, 6))
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            ui.label(RichText::new(&e.champ.name).size(11.5).strong().color(TEXT));
                            ui.label(
                                RichText::new(if e.manual {
                                    "· lane set by you"
                                } else {
                                    "· lane guessed from play rate"
                                })
                                .size(10.5)
                                .color(MUTED),
                            );
                            ui.add_space(12.0);
                            for &l in LANES {
                                let on = e.lane == l;
                                let r = e.rates.get(l).copied().unwrap_or(0);
                                let lbl = format!("{} ({r}%)", short_lane(l));
                                if ui.selectable_label(on, lbl).clicked() {
                                    st.lane_picker_enemy = None;
                                    let ctl_c = Arc::clone(ctl);
                                    let lane_s = l.to_string();
                                    tokio::spawn(async move {
                                        ctl_c.set_enemy_lane(eid, &lane_s).await;
                                    });
                                }
                            }
                            if ui.selectable_label(!e.manual, "AUTO").clicked() {
                                st.lane_picker_enemy = None;
                                if e.manual {
                                    let ctl_c = Arc::clone(ctl);
                                    tokio::spawn(async move {
                                        ctl_c.set_enemy_lane(eid, "").await;
                                    });
                                }
                            }
                        });
                    });
            }
        });
}

fn paint_ally_slot(ui: &mut Ui, slot: Option<&AllySlotView>, idx: usize, imgs: &mut ImageCache) {
    let default_lanes = ["TOP", "JG", "MID", "BOT", "SUP"];
    ui.allocate_ui_with_layout(Vec2::new(42.0, 58.0), Layout::top_down(Align::Center), |ui| {
        if let Some(a) = slot {
            let border = if a.is_me { GOLD } else { ALLY };
            let tint = if a.tentative {
                Color32::from_rgba_unmultiplied(255, 255, 255, 145)
            } else {
                Color32::WHITE
            };
            let (rect, resp) = ui.allocate_exact_size(Vec2::splat(44.0), Sense::hover());
            ui.painter().rect_filled(rect, CornerRadius::same(6), RAISED);
            if !a.champ.icon.is_empty() {
                let mut sub = ui.new_child(egui::UiBuilder::new().max_rect(rect.shrink(2.0)));
                imgs.paint_icon(&mut sub, &a.champ.name, &a.champ.icon, Vec2::splat(40.0), 5.0, tint);
            } else if a.is_me {
                ui.painter().text(rect.center(), egui::Align2::CENTER_CENTER, "YOU", egui::FontId::proportional(10.0), GOLD);
            }
            ui.painter().rect_stroke(rect, CornerRadius::same(6), Stroke::new(1.5, border), egui::StrokeKind::Inside);
            let cap = if a.is_me && !a.champ.icon.is_empty() {
                "YOU".into()
            } else {
                format!("{}{}", short_lane(&a.lane), if a.tentative && !a.is_me { "?" } else { "" })
            };
            ui.label(RichText::new(cap).size(9.5).strong().color(if a.is_me { GOLD } else { MUTED }));
            resp.on_hover_text(if a.champ.name.is_empty() {
                format!("You ({})", a.lane)
            } else {
                format!("{}{}", a.champ.name, if a.tentative { " (hovering)" } else { "" })
            });
        } else {
            let (rect, _) = ui.allocate_exact_size(Vec2::splat(44.0), Sense::hover());
            ui.painter().rect_filled(rect, CornerRadius::same(6), RAISED);
            ui.painter().rect_stroke(rect, CornerRadius::same(6), Stroke::new(1.0, LINE2), egui::StrokeKind::Inside);
            ui.label(RichText::new(default_lanes[idx]).size(9.5).color(DIM));
        }
    });
}

fn paint_enemy_slot(
    ui: &mut Ui,
    slot: Option<&EnemySlotView>,
    idx: usize,
    selected: bool,
    imgs: &mut ImageCache,
) -> bool {
    let default_lanes = ["TOP", "JG", "MID", "BOT", "SUP"];
    let mut clicked = false;
    ui.allocate_ui_with_layout(Vec2::new(46.0, 64.0), Layout::top_down(Align::Center), |ui| {
        if let Some(e) = slot {
            let border = if selected { GOLD } else { ENEMY };
            let (rect, resp) = ui.allocate_exact_size(Vec2::splat(44.0), Sense::click());
            ui.painter().rect_filled(rect, CornerRadius::same(6), RAISED);
            let mut sub = ui.new_child(egui::UiBuilder::new().max_rect(rect.shrink(2.0)));
            imgs.paint_icon(&mut sub, &e.champ.name, &e.champ.icon, Vec2::splat(40.0), 5.0, Color32::WHITE);
            ui.painter().rect_stroke(rect, CornerRadius::same(6), Stroke::new(1.5, border), egui::StrokeKind::Inside);
            clicked = resp.clicked();
            let cap = format!("{}{}", short_lane(&e.lane), if e.flex && !e.manual { "?" } else { "" });
            let cap_col = if e.manual { TEXT } else if e.flex { GOLD } else { MUTED };
            ui.label(RichText::new(cap).size(9.5).strong().color(cap_col));
            resp.on_hover_text(format!("{} ({}) · click to set lane", e.champ.name, e.lane));
        } else {
            let (rect, _) = ui.allocate_exact_size(Vec2::splat(44.0), Sense::hover());
            ui.painter().rect_filled(rect, CornerRadius::same(6), RAISED);
            ui.painter().rect_stroke(rect, CornerRadius::same(6), Stroke::new(1.0, LINE2), egui::StrokeKind::Inside);
            ui.label(RichText::new(default_lanes[idx]).size(9.5).color(DIM));
        }
    });
    clicked
}

fn render_odds(ui: &mut Ui, o: Option<&OddsView>, note: &str) {
    let win = o.map_or(50.0, |x| x.win);
    let known = o.map_or(0, |x| x.known);
    let pending = o.map_or(0, |x| x.pending);

    let w = ui.available_width();
    ui.vertical_centered(|ui| {
        ui.horizontal(|ui| {
            let side_lbl = 86.0_f32.min(w * 0.28);
            let mid_w = (w - side_lbl * 2.0 - 16.0).max(80.0);
            ui.add_sized(
                [side_lbl, 20.0],
                egui::Label::new(RichText::new(format!("{win:.1}% you")).size(14.5).strong().color(ALLY_TEXT)),
            );
            let mid = if pending > 0 {
                format!("draft estimate · {known} of 10 · loading {pending}")
            } else {
                format!("draft estimate · {known} of 10 picked")
            };
            ui.add_sized([mid_w, 20.0], egui::Label::new(RichText::new(mid).size(10.0).color(MUTED)));
            ui.add_sized(
                [side_lbl, 20.0],
                egui::Label::new(RichText::new(format!("{:.1}% them", 100.0 - win)).size(14.5).strong().color(ENEMY_TEXT)),
            );
        });

        let (bar, _) = ui.allocate_exact_size(Vec2::new(w, 5.0), Sense::hover());
        ui.painter().rect_filled(bar, CornerRadius::same(3), ENEMY);
        let fill_w = bar.width() * (win as f32 / 100.0).clamp(0.0, 1.0);
        ui.painter().rect_filled(
            egui::Rect::from_min_size(bar.min, Vec2::new(fill_w, bar.height())),
            CornerRadius::same(3),
            ALLY,
        );

        ui.add_space(4.0);
        if let Some(ov) = o
            && (!ov.reasons.is_empty() || !note.is_empty())
        {
            ui.horizontal(|ui| {
                for r in ov.reasons.iter().take(4) {
                    let col = if r.value >= 0.0 { POS } else { NEG };
                    egui::Frame::new()
                        .fill(RAISED)
                        .corner_radius(CornerRadius::same(8))
                        .inner_margin(egui::Margin::symmetric(6, 2))
                        .show(ui, |ui| {
                            ui.horizontal(|ui| {
                                ui.label(RichText::new(fmt_signed(r.value, 1)).size(10.0).strong().color(col));
                                ui.label(RichText::new(&r.label).size(10.0).color(TEXT));
                            });
                        });
                }
                if !note.is_empty() {
                    ui.label(RichText::new(note).size(10.0).color(GOLD));
                }
            });
        } else {
            ui.label(RichText::new("The estimate moves once champions are locked in").size(10.0).color(MUTED));
        }
    });
}
