use egui::{Align, Color32, CornerRadius, Layout, RichText, Sense, Ui, Vec2};

use crate::engine::views::{BlindView, ChampRef, ContribView, PicksView, RecView};

use crate::images::ImageCache;
use crate::ui::theme::{DIM, GOLD, LINE, MUTED, NEG, PANEL, POS, RAISED, TEXT, TEXT2, fmt_signed, heat_bg, short_lane};

#[derive(Default)]
pub struct PicksUiState {
    pub selected_champ: Option<i32>,
}

struct TermCol {
    kind: String, // "vs" | "with"
    champ: ChampRef,
    lane: String,
    tentative: bool,
}

fn collect_term_cols(d: &PicksView) -> Vec<TermCol> {
    let mut cols: Vec<TermCol> = Vec::new();
    let mut seen = std::collections::HashMap::<String, usize>::new();
    let mut add = |kind: &str, champ: &ChampRef, lane: &str, tent: bool| {
        let key = format!("{kind}:{}", champ.id);
        if let Some(&idx) = seen.get(&key) {
            if tent {
                cols[idx].tentative = true;
            }
            return;
        }
        seen.insert(key, cols.len());
        cols.push(TermCol {
            kind: kind.to_string(),
            champ: champ.clone(),
            lane: lane.to_string(),
            tentative: tent,
        });
    };
    for e in &d.enemies {
        add("vs", &e.champ, &e.lane, false);
    }
    for a in &d.allies {
        if !a.is_me {
            add("with", &a.champ, &a.lane, a.tentative);
        }
    }
    for r in &d.recs {
        for c in &r.contributions {
            if c.kind == "vs" || c.kind == "with" {
                add(&c.kind, &c.champ, &c.lane, c.tentative);
            }
        }
    }
    let lane_idx = |l: &str| match l {
        "top" => 0,
        "jungle" => 1,
        "middle" => 2,
        "bottom" => 3,
        "support" => 4,
        _ => 9,
    };
    cols.sort_by(|a, b| {
        let ka = if a.kind == "vs" { 0 } else { 1 };
        let kb = if b.kind == "vs" { 0 } else { 1 };
        ka.cmp(&kb).then_with(|| lane_idx(&a.lane).cmp(&lane_idx(&b.lane)))
    });
    cols
}

pub fn render_best_picks(
    ui: &mut Ui,
    d: &PicksView,
    state: &mut PicksUiState,
    imgs: &mut ImageCache,
) {
    if d.recs.is_empty() {
        ui.centered_and_justified(|ui| {
            ui.label(RichText::new("No champion recommendations available.").color(MUTED));
        });
        return;
    }

    if let Some(locked) = &d.locked
        && state.selected_champ.is_none()
        && d.recs.iter().any(|r| r.champ.id == locked.id)
    {
        state.selected_champ = Some(locked.id);
    }
    if state
        .selected_champ
        .is_none_or(|id| !d.recs.iter().any(|r| r.champ.id == id))
    {
        state.selected_champ = d.recs.first().map(|r| r.champ.id);
    }

    let tcols = collect_term_cols(d);
    let mut vals = Vec::new();
    for r in &d.recs {
        vals.push(r.base);
        let mut own_sum = 0.0;
        for c in &r.contributions {
            if c.kind == "vs" || c.kind == "with" {
                vals.push(c.weighted);
            } else {
                own_sum += c.weighted;
            }
        }
        vals.push(own_sum);
    }
    let max_cell = vals.into_iter().map(f64::abs).fold(1.0_f64, f64::max);
    let top_score = d.recs.iter().map(|r| r.score.abs()).fold(0.01_f64, f64::max);

    let avail_w = ui.available_width();
    let side_w = (avail_w * 0.34).clamp(240.0, 320.0);
    let main_w = (avail_w - side_w - 10.0).max(340.0);
    let cell_w = if tcols.len() >= 8 { 36.0 } else { 42.0 };

    ui.horizontal_top(|ui| {
        ui.allocate_ui_with_layout(
            Vec2::new(main_w, ui.available_height()),
            Layout::top_down(Align::Min),
            |ui| {
                if let Some(locked) = &d.locked {
                    ui.horizontal(|ui| {
                        imgs.paint_icon(ui, &locked.name, &locked.icon, Vec2::splat(18.0), 4.0, Color32::WHITE);
                        ui.label(RichText::new(format!("Locked in: {}", locked.name)).size(11.5).strong().color(GOLD));
                        ui.label(RichText::new("picks as they were at lock-in").size(10.5).color(MUTED));
                    });
                    ui.add_space(4.0);
                }

                egui::ScrollArea::both().id_salt("picks_tbl").show(ui, |ui| {
                    // Header row
                    ui.horizontal(|ui| {
                        ui.add_sized([20.0, 24.0], egui::Label::new(RichText::new("#").size(10.0).color(DIM)));
                        ui.add_sized([115.0, 24.0], egui::Label::new(RichText::new("PICK").size(10.0).color(MUTED)));
                        ui.add_sized([62.0, 24.0], egui::Label::new(RichText::new("SCORE").size(10.0).color(MUTED)));
                        ui.add_sized([cell_w, 24.0], egui::Label::new(RichText::new("BASE").size(10.0).color(MUTED)));
                        for tc in &tcols {
                            let tint = if tc.tentative {
                                Color32::from_rgba_unmultiplied(255, 255, 255, 130)
                            } else {
                                Color32::WHITE
                            };
                            let (rect, resp) = ui.allocate_exact_size(Vec2::new(cell_w, 24.0), Sense::hover());
                            let ico_r = egui::Rect::from_center_size(rect.center(), Vec2::splat(22.0));
                            let mut sub = ui.new_child(egui::UiBuilder::new().max_rect(ico_r));
                            imgs.paint_icon(&mut sub, &tc.champ.name, &tc.champ.icon, Vec2::splat(22.0), 11.0, tint);
                            resp.on_hover_text(format!(
                                "{} {} ({}){}",
                                if tc.kind == "vs" { "vs" } else { "with" },
                                tc.champ.name,
                                short_lane(&tc.lane),
                                if tc.tentative { " · hovering" } else { "" }
                            ));
                        }
                        ui.add_sized([cell_w, 24.0], egui::Label::new(RichText::new("YOU").size(10.0).color(GOLD)));
                    });
                    ui.separator();

                    for (i, r) in d.recs.iter().enumerate() {
                        let selected = state.selected_champ == Some(r.champ.id);
                        let is_locked = d.locked.as_ref().is_some_and(|l| l.id == r.champ.id);
                        let bg = if selected {
                            Color32::from_rgba_unmultiplied(0xE6, 0xB8, 0x5C, 28)
                        } else if is_locked {
                            Color32::from_rgba_unmultiplied(0x3D, 0xDC, 0x97, 20)
                        } else {
                            Color32::TRANSPARENT
                        };
                        let stroke = if selected {
                            egui::Stroke::new(1.0, GOLD)
                        } else {
                            egui::Stroke::NONE
                        };
                        let frame_resp = egui::Frame::new()
                            .fill(bg)
                            .stroke(stroke)
                            .corner_radius(CornerRadius::same(4))
                            .inner_margin(egui::Margin::symmetric(4, 3))
                            .show(ui, |ui| {
                                ui.horizontal(|ui| {
                                    ui.add_sized([16.0, 26.0], egui::Label::new(RichText::new(format!("{}", i + 1)).size(11.0).color(DIM)));
                                    imgs.paint_icon(ui, &r.champ.name, &r.champ.icon, Vec2::splat(24.0), 4.0, Color32::WHITE);
                                    ui.add_sized(
                                        [84.0, 26.0],
                                        egui::Label::new(RichText::new(&r.champ.name).size(12.0).strong().color(TEXT)).truncate(),
                                    );
                                    // Score + bar
                                    let sc_col = if r.score >= 0.0 { POS } else { NEG };
                                    ui.allocate_ui_with_layout(Vec2::new(60.0, 26.0), Layout::top_down(Align::Max), |ui| {
                                        ui.label(RichText::new(fmt_signed(r.score, 2)).size(12.0).strong().color(sc_col));
                                        let frac = (r.score.abs() / top_score).clamp(0.0, 1.0) as f32;
                                        let (brect, _) = ui.allocate_exact_size(Vec2::new(54.0, 3.0), Sense::hover());
                                        ui.painter().rect_filled(brect, CornerRadius::same(2), LINE);
                                        let mut fill = brect;
                                        fill.set_width(brect.width() * frac);
                                        ui.painter().rect_filled(fill, CornerRadius::same(2), sc_col);
                                    });
                                    paint_heat(ui, Some(r.base), max_cell, cell_w, &format!("{}: base {}", r.champ.name, fmt_signed(r.base, 2)));
                                    for tc in &tcols {
                                        let m = r
                                            .contributions
                                            .iter()
                                            .find(|c| c.kind == tc.kind && c.champ.id == tc.champ.id);
                                        let tip = m.map(|c| contrib_tip(&r.champ.name, c)).unwrap_or_default();
                                        paint_heat(ui, m.map(|c| c.weighted), max_cell, cell_w, &tip);
                                    }
                                    let own: Vec<&ContribView> = r
                                        .contributions
                                        .iter()
                                        .filter(|c| c.kind == "you" || c.kind == "mastery")
                                        .collect();
                                    let own_val = if own.is_empty() {
                                        None
                                    } else {
                                        Some(own.iter().map(|c| c.weighted).sum())
                                    };
                                    let own_tip = own
                                        .iter()
                                        .map(|c| contrib_tip(&r.champ.name, c))
                                        .collect::<Vec<_>>()
                                        .join("\n");
                                    paint_heat(ui, own_val, max_cell, cell_w, &own_tip);
                                });
                            });
                        if frame_resp.response.interact(Sense::click()).clicked() {
                            state.selected_champ = Some(r.champ.id);
                        }
                    }
                });
            },
        );

        ui.add_space(8.0);
        // Side detail panel
        if let Some((idx, r)) = d
            .recs
            .iter()
            .enumerate()
            .find(|(_, r)| Some(r.champ.id) == state.selected_champ)
        {
            ui.allocate_ui_with_layout(
                Vec2::new(side_w, ui.available_height()),
                Layout::top_down(Align::Min),
                |ui| {
                    render_pick_detail(ui, r, idx, d.locked.as_ref(), imgs);
                },
            );
        }
    });
}

fn paint_heat(ui: &mut Ui, val: Option<f64>, max: f64, width: f32, tip: &str) {
    let (rect, resp) = ui.allocate_exact_size(Vec2::new(width, 24.0), Sense::hover());
    if let Some(v) = val {
        let (bg, fg) = heat_bg(v, max);
        ui.painter().rect_filled(rect.shrink(1.0), CornerRadius::same(4), bg);
        ui.painter().text(
            rect.center(),
            egui::Align2::CENTER_CENTER,
            fmt_signed(v, 1),
            egui::FontId::proportional(11.0),
            fg,
        );
        if !tip.is_empty() {
            resp.on_hover_text(tip);
        }
    } else {
        ui.painter().text(
            rect.center(),
            egui::Align2::CENTER_CENTER,
            "·",
            egui::FontId::proportional(11.0),
            DIM,
        );
    }
}

fn contrib_tip(pick_name: &str, c: &ContribView) -> String {
    match c.kind.as_str() {
        "you" => format!(
            "Your games on {pick_name}: {} WR vs usual in {} games → {}",
            fmt_signed(c.delta, 1),
            c.games,
            fmt_signed(c.weighted, 2)
        ),
        "mastery" => format!(
            "{:.0} mastery points on {pick_name} → {}",
            c.delta,
            fmt_signed(c.weighted, 2)
        ),
        _ => format!(
            "{pick_name} {} {} ({}): {} WR in {} games → {}{}",
            if c.kind == "with" { "with" } else { "vs" },
            c.champ.name,
            short_lane(&c.lane),
            fmt_signed(c.delta, 1),
            c.games,
            fmt_signed(c.weighted, 2),
            if c.tentative { " (hovering)" } else { "" }
        ),
    }
}

fn render_pick_detail(
    ui: &mut Ui,
    r: &RecView,
    idx: usize,
    locked: Option<&ChampRef>,
    imgs: &mut ImageCache,
) {
    let is_locked = locked.is_some_and(|l| l.id == r.champ.id);
    let kicker = if is_locked {
        "LOCKED IN".to_string()
    } else if idx == 0 {
        "TOP PICK".to_string()
    } else {
        format!("PICK #{}", idx + 1)
    };
    let sc_col = if r.score >= 0.0 { POS } else { NEG };

    egui::Frame::new()
        .fill(PANEL)
        .stroke(egui::Stroke::new(1.0, LINE))
        .corner_radius(CornerRadius::same(6))
        .inner_margin(egui::Margin::same(10))
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                imgs.paint_icon(ui, &r.champ.name, &r.champ.icon, Vec2::splat(38.0), 6.0, Color32::WHITE);
                ui.vertical(|ui| {
                    ui.label(RichText::new(kicker).size(9.5).strong().color(GOLD));
                    ui.label(RichText::new(&r.champ.name).size(15.0).strong().color(TEXT));
                });
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    ui.vertical(|ui| {
                        ui.label(RichText::new(fmt_signed(r.score, 2)).size(18.0).strong().color(sc_col));
                        ui.label(RichText::new("win-rate points").size(9.0).color(MUTED));
                    });
                });
            });
            ui.add_space(6.0);
            ui.separator();
            ui.add_space(6.0);

            // Best / watch out tiles
            let best = r
                .contributions
                .iter()
                .filter(|c| (c.kind == "vs" || c.kind == "with") && c.weighted > 0.0)
                .max_by(|a, b| a.weighted.total_cmp(&b.weighted));
            let worst = r
                .contributions
                .iter()
                .filter(|c| (c.kind == "vs" || c.kind == "with") && c.weighted < 0.0)
                .min_by(|a, b| a.weighted.total_cmp(&b.weighted));

            ui.columns(2, |cols| {
                tile(&mut cols[0], "BEST INTO", best, "None", imgs);
                tile(&mut cols[1], "WATCH OUT", worst, "None", imgs);
            });
            ui.add_space(6.0);

            egui::ScrollArea::vertical().id_salt("pick_det").show(ui, |ui| {
                detail_row(ui, None, "Base strength", &fmt_signed(r.base, 1), r.base, imgs);
                for c in &r.contributions {
                    let (label, raw) = match c.kind.as_str() {
                        "you" => ("Your games".to_string(), fmt_signed(c.delta, 1)),
                        "mastery" => ("Mastery".to_string(), format!("{:.0} pts", c.delta)),
                        _ => (
                            format!("{} {}", if c.kind == "vs" { "vs" } else { "w/" }, c.champ.name),
                            fmt_signed(c.delta, 1),
                        ),
                    };
                    detail_row(ui, Some(&c.champ), &label, &raw, c.weighted, imgs);
                }
            });
        });
}

fn tile(ui: &mut Ui, cap: &str, c: Option<&ContribView>, none: &str, imgs: &mut ImageCache) {
    egui::Frame::new()
        .fill(RAISED)
        .corner_radius(CornerRadius::same(4))
        .inner_margin(egui::Margin::same(6))
        .show(ui, |ui| {
            ui.label(RichText::new(cap).size(9.0).color(MUTED));
            if let Some(c) = c {
                ui.horizontal(|ui| {
                    imgs.paint_icon(ui, &c.champ.name, &c.champ.icon, Vec2::splat(16.0), 3.0, Color32::WHITE);
                    ui.label(RichText::new(&c.champ.name).size(10.5).color(TEXT));
                    let col = if c.weighted >= 0.0 { POS } else { NEG };
                    ui.label(RichText::new(fmt_signed(c.weighted, 1)).size(10.5).strong().color(col));
                });
            } else {
                ui.label(RichText::new(none).size(10.5).color(DIM));
            }
        });
}

fn detail_row(
    ui: &mut Ui,
    champ: Option<&ChampRef>,
    label: &str,
    raw: &str,
    adds: f64,
    imgs: &mut ImageCache,
) {
    ui.horizontal(|ui| {
        if let Some(c) = champ {
            imgs.paint_icon(ui, &c.name, &c.icon, Vec2::splat(16.0), 3.0, Color32::WHITE);
        } else {
            ui.add_space(20.0);
        }
        ui.add_sized([100.0, 18.0], egui::Label::new(RichText::new(label).size(11.0).color(TEXT2)).truncate());
        ui.add_sized([46.0, 18.0], egui::Label::new(RichText::new(raw).size(10.5).color(MUTED)));
        let col = if adds.abs() < 0.05 {
            DIM
        } else if adds > 0.0 {
            POS
        } else {
            NEG
        };
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            ui.label(RichText::new(fmt_signed(adds, 1)).size(11.0).strong().color(col));
        });
    });
}

pub fn render_blind_picks(ui: &mut Ui, d: &PicksView, imgs: &mut ImageCache) {
    let list: &[BlindView] = &d.blind;
    if list.is_empty() {
        ui.centered_and_justified(|ui| {
            ui.label(RichText::new("No blind-pick ranking for this draft.").color(MUTED));
        });
        return;
    }

    let max_str = list
        .iter()
        .map(|r| (r.score - r.value - r.synergy).abs())
        .fold(0.5_f64, f64::max);
    let max_fit = list.iter().map(|r| r.synergy.abs()).fold(0.5_f64, f64::max);
    let max_risk = list.iter().map(|r| r.value.abs()).fold(0.5_f64, f64::max);

    let lo = list
        .iter()
        .map(|r| r.tail.min(r.mean))
        .fold(0.0_f64, f64::min)
        .floor();
    let hi = list
        .iter()
        .map(|r| r.tail.max(r.mean))
        .fold(0.0_f64, f64::max)
        .max(0.5);
    let hi = ((hi * 2.0).ceil() / 2.0).max(lo + 1.5);

    egui::ScrollArea::vertical().id_salt("blind_tbl").show(ui, |ui| {
        ui.horizontal(|ui| {
            ui.add_sized([22.0, 22.0], egui::Label::new(RichText::new("#").size(10.0).color(DIM)));
            ui.add_sized([130.0, 22.0], egui::Label::new(RichText::new("PICK").size(10.0).color(MUTED)));
            ui.add_sized([54.0, 22.0], egui::Label::new(RichText::new("SCORE").size(10.0).color(MUTED)));
            ui.add_sized([56.0, 22.0], egui::Label::new(RichText::new("STRENGTH").size(10.0).color(MUTED)));
            ui.add_sized([52.0, 22.0], egui::Label::new(RichText::new("FIT").size(10.0).color(MUTED)));
            ui.add_sized([190.0, 22.0], egui::Label::new(RichText::new("IF COUNTERED  —  AVG MATCHUP").size(10.0).color(MUTED)));
            ui.add_sized([52.0, 22.0], egui::Label::new(RichText::new("RISK").size(10.0).color(MUTED)));
            ui.add_sized([90.0, 22.0], egui::Label::new(RichText::new("WORST").size(10.0).color(MUTED)));
            ui.label(RichText::new("DATA").size(10.0).color(MUTED));
        });
        ui.separator();

        for (i, r) in list.iter().enumerate() {
            let strength = r.score - r.value - r.synergy;
            ui.horizontal(|ui| {
                ui.add_sized([22.0, 28.0], egui::Label::new(RichText::new(format!("{}", i + 1)).size(11.0).color(DIM)));
                imgs.paint_icon(ui, &r.champ.name, &r.champ.icon, Vec2::splat(24.0), 4.0, Color32::WHITE);
                ui.add_sized([100.0, 28.0], egui::Label::new(RichText::new(&r.champ.name).size(12.0).strong().color(TEXT)).truncate());
                let sc_col = if r.score >= 0.0 { POS } else { NEG };
                ui.add_sized([54.0, 28.0], egui::Label::new(RichText::new(fmt_signed(r.score, 2)).size(12.0).strong().color(sc_col)));
                paint_heat(ui, Some(strength), max_str, 54.0, "Overall strength in this role");
                paint_heat(ui, Some(r.synergy), max_fit, 50.0, "Fit with your team's picks");

                // Range bar
                let (rect, resp) = ui.allocate_exact_size(Vec2::new(190.0, 26.0), Sense::hover());
                let x_of = |v: f64| rect.left() + 14.0 + ((v - lo) / (hi - lo)) as f32 * (rect.width() - 28.0);
                let cy = rect.center().y;
                let p = ui.painter();
                p.line_segment([egui::pos2(rect.left() + 8.0, cy), egui::pos2(rect.right() - 8.0, cy)], egui::Stroke::new(1.0, LINE));
                let zx = x_of(0.0);
                p.line_segment([egui::pos2(zx, rect.top() + 4.0), egui::pos2(zx, rect.bottom() - 4.0)], egui::Stroke::new(1.0, DIM));
                let x_tail = x_of(r.tail);
                let x_mean = x_of(r.mean);
                p.line_segment([egui::pos2(x_tail, cy), egui::pos2(x_mean, cy)], egui::Stroke::new(3.0, MUTED));
                p.circle_filled(egui::pos2(x_tail, cy), 4.0, NEG);
                p.circle_filled(egui::pos2(x_mean, cy), 4.0, POS);
                resp.on_hover_text(format!(
                    "{}: {} avg matchup, {} if countered",
                    r.champ.name,
                    fmt_signed(r.mean, 1),
                    fmt_signed(r.tail, 1)
                ));

                paint_heat(ui, Some(r.value), max_risk, 50.0, "Blind risk contribution");
                ui.horizontal(|ui| {
                    for c in r.counters.iter().take(3) {
                        imgs.paint_icon(ui, &c.champ.name, &c.champ.icon, Vec2::splat(20.0), 3.0, Color32::WHITE)
                            .on_hover_text(format!("{} {} ({} games)", c.champ.name, fmt_signed(c.delta, 1), c.games));
                    }
                });
                ui.label(
                    RichText::new(format!("{}%", r.coverage))
                        .size(10.5)
                        .color(if r.coverage < 50 { GOLD } else { MUTED }),
                );
            });
        }
    });
}
