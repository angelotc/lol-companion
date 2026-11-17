use egui::{Align, Align2, Color32, CornerRadius, FontId, Layout, Pos2, Rect, RichText, Sense, Stroke, Ui, Vec2};

use crate::engine::views::{BlindView, ChampRef, ContribView, PicksView, RecView};

use crate::images::ImageCache;
use crate::ui::theme::{ALLY, DIM, ENEMY, GOLD, LINE, LINE2, MUTED, NEG, PANEL, POS, RAISED, TEXT, TEXT2, fmt_signed, heat_bg, left_label, short_lane};

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
    let n_vs = tcols.iter().filter(|c| c.kind == "vs").count();
    let n_with = tcols.len() - n_vs;

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
    let avail_h = ui.available_height();
    let side_w = (avail_w * 0.30).clamp(260.0, 350.0);
    let main_w = (avail_w - side_w - 14.0).max(340.0);
    let cell_w = if tcols.len() >= 8 { 40.0 } else { 46.0 };

    ui.horizontal_top(|ui| {
        ui.allocate_ui_with_layout(
            Vec2::new(main_w, avail_h),
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

                let tbl_h = (ui.available_height() - 22.0).max(120.0);
                ui.allocate_ui(Vec2::new(main_w, tbl_h), |ui| {
                    egui::ScrollArea::both().id_salt("picks_tbl").show(ui, |ui| {
                        // Group headers: AGAINST | WITH | YOU
                        let lead_w = 18.0 + 122.0 + 62.0 + cell_w + 4.0 * 8.0;
                        ui.horizontal(|ui| {
                            ui.add_space(lead_w);
                            if n_vs > 0 {
                                let w = n_vs as f32 * cell_w + (n_vs.saturating_sub(1) as f32) * 8.0;
                                let (r, _) = ui.allocate_exact_size(Vec2::new(w, 16.0), Sense::hover());
                                ui.painter().text(r.left_center(), Align2::LEFT_CENTER, "AGAINST", FontId::proportional(9.5), ENEMY);
                                ui.painter().line_segment(
                                    [Pos2::new(r.left(), r.bottom() - 1.0), Pos2::new(r.right(), r.bottom() - 1.0)],
                                    Stroke::new(1.5, ENEMY),
                                );
                            }
                            if n_with > 0 {
                                ui.add_space(8.0);
                                let w = n_with as f32 * cell_w + (n_with.saturating_sub(1) as f32) * 8.0;
                                let (r, _) = ui.allocate_exact_size(Vec2::new(w, 16.0), Sense::hover());
                                ui.painter().text(r.left_center(), Align2::LEFT_CENTER, "WITH", FontId::proportional(9.5), ALLY);
                                ui.painter().line_segment(
                                    [Pos2::new(r.left(), r.bottom() - 1.0), Pos2::new(r.right(), r.bottom() - 1.0)],
                                    Stroke::new(1.5, ALLY),
                                );
                            }
                            ui.add_space(8.0);
                            let (r, _) = ui.allocate_exact_size(Vec2::new(cell_w, 16.0), Sense::hover());
                            ui.painter().text(r.center(), Align2::CENTER_CENTER, "YOU", FontId::proportional(9.5), GOLD);
                            ui.painter().line_segment(
                                [Pos2::new(r.left(), r.bottom() - 1.0), Pos2::new(r.right(), r.bottom() - 1.0)],
                                Stroke::new(1.5, GOLD),
                            );
                        });

                        // Column headers + portrait icons
                        ui.horizontal(|ui| {
                            ui.add_sized([18.0, 28.0], egui::Label::new(RichText::new("").size(10.0)));
                            left_label(ui, 128.0, 28.0, RichText::new("Pick").size(10.5).color(MUTED));
                            ui.add_sized([62.0, 28.0], egui::Label::new(RichText::new("Score").size(10.5).color(MUTED)));
                            ui.add_sized([cell_w, 28.0], egui::Label::new(RichText::new("Base").size(10.5).color(MUTED)));
                            for (idx, tc) in tcols.iter().enumerate() {
                                if idx == n_vs && n_vs > 0 {
                                    ui.add_space(8.0);
                                }
                                let tint = if tc.tentative {
                                    Color32::from_rgba_unmultiplied(255, 255, 255, 130)
                                } else {
                                    Color32::WHITE
                                };
                                let (rect, resp) = ui.allocate_exact_size(Vec2::new(cell_w, 28.0), Sense::hover());
                                let ico_r = Rect::from_center_size(rect.center(), Vec2::splat(26.0));
                                let mut sub = ui.new_child(egui::UiBuilder::new().max_rect(ico_r));
                                imgs.paint_icon(&mut sub, &tc.champ.name, &tc.champ.icon, Vec2::splat(26.0), 5.0, tint);
                                resp.on_hover_text(format!(
                                    "{} {} ({}){}",
                                    if tc.kind == "vs" { "vs" } else { "with" },
                                    tc.champ.name,
                                    short_lane(&tc.lane),
                                    if tc.tentative { " · hovering" } else { "" }
                                ));
                            }
                            ui.add_space(8.0);
                            ui.add_sized([cell_w, 26.0], egui::Label::new(RichText::new("").size(10.0)));
                        });
                        ui.separator();

                        for (i, r) in d.recs.iter().enumerate() {
                            let selected = state.selected_champ == Some(r.champ.id);
                            let is_locked = d.locked.as_ref().is_some_and(|l| l.id == r.champ.id);
                            let bg = if selected {
                                Color32::from_rgba_unmultiplied(0xE6, 0xB8, 0x5C, 22)
                            } else if is_locked {
                                Color32::from_rgba_unmultiplied(0x3D, 0xDC, 0x97, 20)
                            } else {
                                Color32::TRANSPARENT
                            };
                            let stroke = if selected {
                                Stroke::new(1.0, GOLD)
                            } else {
                                Stroke::NONE
                            };
                            let frame_resp = egui::Frame::new()
                                .fill(bg)
                                .stroke(stroke)
                                .corner_radius(CornerRadius::same(6))
                                .inner_margin(egui::Margin::symmetric(4, 4))
                                .show(ui, |ui| {
                                    ui.horizontal(|ui| {
                                        ui.add_sized(
                                            [16.0, 32.0],
                                            egui::Label::new(
                                                RichText::new(format!("{}", i + 1))
                                                    .size(11.5)
                                                    .color(if selected { GOLD } else { DIM }),
                                            ),
                                        );
                                        imgs.paint_icon(ui, &r.champ.name, &r.champ.icon, Vec2::splat(32.0), 5.0, Color32::WHITE);
                                        left_label(ui, 88.0, 32.0, RichText::new(&r.champ.name).size(13.0).strong().color(TEXT));
                                        let sc_col = if r.score >= 0.0 { POS } else { NEG };
                                        ui.allocate_ui_with_layout(Vec2::new(60.0, 28.0), Layout::top_down(Align::Max), |ui| {
                                            ui.label(RichText::new(fmt_signed(r.score, 2)).size(13.0).strong().color(sc_col));
                                            let frac = (r.score.abs() / top_score).clamp(0.0, 1.0) as f32;
                                            let (brect, _) = ui.allocate_exact_size(Vec2::new(54.0, 3.0), Sense::hover());
                                            ui.painter().rect_filled(brect, CornerRadius::same(2), LINE);
                                            let mut fill = brect;
                                            fill.set_width(brect.width() * frac);
                                            ui.painter().rect_filled(fill, CornerRadius::same(2), sc_col);
                                        });
                                        paint_heat(ui, Some(r.base), max_cell, cell_w, &format!("{}: base {}", r.champ.name, fmt_signed(r.base, 2)));

                                        for (ci, tc) in tcols.iter().enumerate() {
                                            if ci == n_vs && n_vs > 0 {
                                                ui.add_space(8.0);
                                            }
                                            let m = r
                                                .contributions
                                                .iter()
                                                .find(|c| c.kind == tc.kind && c.champ.id == tc.champ.id);
                                            let tip = m.map(|c| contrib_tip(&r.champ.name, c)).unwrap_or_default();
                                            paint_heat(ui, m.map(|c| c.weighted), max_cell, cell_w, &tip);
                                        }

                                        ui.add_space(8.0);
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
                            ui.add_space(2.0);
                        }
                    });
                });

                ui.label(
                    RichText::new("Each cell is the win-rate points that term adds to the score: strength, each matchup and duo, and your own games and mastery. Click a pick for the raw numbers.")
                        .size(10.0)
                        .color(DIM),
                );
            },
        );

        ui.add_space(12.0);
        // Side detail panel
        if let Some((idx, r)) = d
            .recs
            .iter()
            .enumerate()
            .find(|(_, r)| Some(r.champ.id) == state.selected_champ)
        {
            ui.allocate_ui_with_layout(
                Vec2::new(side_w, avail_h),
                Layout::top_down(Align::Min),
                |ui| {
                    render_pick_detail(ui, r, idx, d.locked.as_ref(), imgs);
                },
            );
        }
    });
}

fn paint_heat(ui: &mut Ui, val: Option<f64>, max: f64, width: f32, tip: &str) {
    let (rect, resp) = ui.allocate_exact_size(Vec2::new(width, 26.0), Sense::hover());
    if let Some(v) = val {
        let (bg, fg) = heat_bg(v, max);
        ui.painter().rect_filled(rect.shrink(1.0), CornerRadius::same(4), bg);
        ui.painter().text(
            rect.center(),
            Align2::CENTER_CENTER,
            fmt_signed(v, 1),
            FontId::proportional(11.5),
            fg,
        );
        if !tip.is_empty() {
            resp.on_hover_text(tip);
        }
    } else {
        ui.painter().text(
            rect.center(),
            Align2::CENTER_CENTER,
            "·",
            FontId::proportional(11.5),
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
    let max_add = r
        .contributions
        .iter()
        .map(|c| c.weighted.abs())
        .fold(r.base.abs().max(0.5), f64::max);

    egui::Frame::new()
        .fill(PANEL)
        .stroke(Stroke::new(1.0, LINE))
        .corner_radius(CornerRadius::same(8))
        .inner_margin(egui::Margin::same(12))
        .show(ui, |ui| {
            ui.set_min_height((ui.available_height() - 4.0).max(160.0));
            ui.horizontal(|ui| {
                imgs.paint_icon(ui, &r.champ.name, &r.champ.icon, Vec2::splat(48.0), 8.0, Color32::WHITE);
                ui.add_space(6.0);
                ui.vertical(|ui| {
                    ui.label(RichText::new(kicker).size(9.5).strong().color(GOLD));
                    ui.label(RichText::new(&r.champ.name).size(16.0).strong().color(TEXT));
                });
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    ui.vertical(|ui| {
                        ui.label(RichText::new(fmt_signed(r.score, 2)).size(20.0).strong().color(sc_col));
                        ui.label(RichText::new("win-rate points").size(9.5).color(MUTED));
                    });
                });
            });
            ui.add_space(8.0);

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
                tile(&mut cols[1], "WATCH OUT FOR", worst, "None", imgs);
            });
            ui.add_space(8.0);

            ui.horizontal(|ui| {
                ui.add_sized([120.0, 16.0], egui::Label::new(RichText::new("WHY THIS SCORE").size(9.5).strong().color(DIM)));
                ui.add_sized([52.0, 16.0], egui::Label::new(RichText::new("Δ WIN %").size(9.5).strong().color(DIM)));
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    ui.label(RichText::new("ADDS").size(9.5).strong().color(DIM));
                });
            });
            ui.separator();

            egui::ScrollArea::vertical().id_salt("pick_det").show(ui, |ui| {
                detail_row(ui, None, "Base strength", &fmt_signed(r.base, 1), r.base, max_add, imgs);
                for c in &r.contributions {
                    let (label, raw) = match c.kind.as_str() {
                        "you" => ("Your games".to_string(), fmt_signed(c.delta, 1)),
                        "mastery" => (
                            "Mastery".to_string(),
                            if c.delta >= 1_000_000.0 {
                                format!("{:.1}M pts", c.delta / 1_000_000.0)
                            } else if c.delta >= 1000.0 {
                                format!("{:.0}k pts", c.delta / 1000.0)
                            } else {
                                format!("{:.0} pts", c.delta)
                            },
                        ),
                        _ => (
                            format!("{} {}", if c.kind == "vs" { "vs" } else { "w/" }, c.champ.name),
                            fmt_signed(c.delta, 1),
                        ),
                    };
                    detail_row(ui, Some(&c.champ), &label, &raw, c.weighted, max_add, imgs);
                }
            });
        });
}

fn tile(ui: &mut Ui, cap: &str, c: Option<&ContribView>, none: &str, imgs: &mut ImageCache) {
    egui::Frame::new()
        .fill(RAISED)
        .corner_radius(CornerRadius::same(6))
        .inner_margin(egui::Margin::same(8))
        .show(ui, |ui| {
            ui.label(RichText::new(cap).size(9.0).strong().color(MUTED));
            ui.add_space(2.0);
            if let Some(c) = c {
                ui.horizontal(|ui| {
                    imgs.paint_icon(ui, &c.champ.name, &c.champ.icon, Vec2::splat(16.0), 3.0, Color32::WHITE);
                    ui.label(RichText::new(format!("{} {}", if c.kind == "vs" { "vs" } else { "w/" }, c.champ.name)).size(10.5).color(TEXT));
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
    max_add: f64,
    imgs: &mut ImageCache,
) {
    ui.horizontal(|ui| {
        if let Some(c) = champ {
            imgs.paint_icon(ui, &c.name, &c.icon, Vec2::splat(18.0), 4.0, Color32::WHITE);
        } else {
            ui.add_space(22.0);
        }
        left_label(ui, 94.0, 20.0, RichText::new(label).size(11.0).color(TEXT2));
        ui.add_sized([52.0, 20.0], egui::Label::new(RichText::new(raw).size(10.5).color(MUTED)));
        let col = if adds.abs() < 0.05 {
            DIM
        } else if adds > 0.0 {
            POS
        } else {
            NEG
        };
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            ui.add_sized([34.0, 20.0], egui::Label::new(RichText::new(fmt_signed(adds, 1)).size(11.0).strong().color(col)));
            let frac = (adds.abs() / max_add).clamp(0.0, 1.0) as f32;
            let (br, _) = ui.allocate_exact_size(Vec2::new(42.0, 7.0), Sense::hover());
            if frac > 0.02 {
                let fill = Rect::from_min_size(br.min, Vec2::new((br.width() * frac).max(3.0), br.height()));
                ui.painter().rect_filled(fill, CornerRadius::same(2), col);
            }
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
    let top_score = list.iter().map(|r| r.score.abs()).fold(0.01_f64, f64::max);

    let lo = list
        .iter()
        .map(|r| r.tail.min(r.mean))
        .fold(0.0_f64, f64::min)
        .floor()
        .min(-3.0);
    let hi = ((list
        .iter()
        .map(|r| r.tail.max(r.mean))
        .fold(0.0_f64, f64::max)
        .max(0.5)
        * 2.0)
        .ceil()
        / 2.0)
        .max(lo + 1.5);

    let avail_w = ui.available_width();
    let avail_h = ui.available_height();
    let track_w = (avail_w - 690.0).clamp(220.0, 420.0);
    let tbl_h = (avail_h - 24.0).max(140.0);

    ui.allocate_ui(Vec2::new(avail_w, tbl_h), |ui| {
        egui::ScrollArea::vertical().id_salt("blind_tbl").show(ui, |ui| {
            // Header row with track legend & ticks
            ui.horizontal(|ui| {
                ui.add_sized([22.0, 28.0], egui::Label::new(RichText::new("").size(10.0)));
                ui.add_sized([155.0, 28.0], egui::Label::new(RichText::new("Pick").size(10.5).color(MUTED)));
                ui.add_sized([68.0, 28.0], egui::Label::new(RichText::new("Score").size(10.5).color(MUTED)));
                ui.add_sized([64.0, 28.0], egui::Label::new(RichText::new("Strength").size(10.5).color(MUTED)));
                ui.add_sized([64.0, 28.0], egui::Label::new(RichText::new("Team fit").size(10.5).color(MUTED)));

                let (hdr_r, _) = ui.allocate_exact_size(Vec2::new(track_w, 28.0), Sense::hover());
                let p = ui.painter();
                let left_x = hdr_r.left() + 34.0;
                let right_x = hdr_r.right() - 34.0;
                let x_of = |v: f64| left_x + ((v - lo) / (hi - lo)) as f32 * (right_x - left_x);
                p.circle_filled(Pos2::new(hdr_r.left() + 26.0, hdr_r.top() + 7.0), 3.5, NEG);
                p.text(Pos2::new(hdr_r.left() + 34.0, hdr_r.top() + 7.0), Align2::LEFT_CENTER, "if countered", FontId::proportional(9.5), MUTED);
                p.circle_filled(Pos2::new(hdr_r.right() - 26.0, hdr_r.top() + 7.0), 3.5, TEXT);
                p.text(Pos2::new(hdr_r.right() - 34.0, hdr_r.top() + 7.0), Align2::RIGHT_CENTER, "average matchup", FontId::proportional(9.5), MUTED);
                let mut tv = lo.ceil() as i32;
                while (tv as f64) <= hi {
                    let tx = x_of(tv as f64);
                    p.text(Pos2::new(tx, hdr_r.bottom() - 6.0), Align2::CENTER_CENTER, format!("{tv}"), FontId::proportional(9.5), DIM);
                    tv += 1;
                }

                ui.add_sized([68.0, 28.0], egui::Label::new(RichText::new("Blind risk").size(10.5).color(MUTED)));
                ui.add_sized([92.0, 28.0], egui::Label::new(RichText::new("Worst counters").size(10.5).color(MUTED)));
                ui.label(RichText::new("Matchup data").size(10.5).color(MUTED));
            });
            ui.separator();

            for (i, r) in list.iter().enumerate() {
                let strength = r.score - r.value - r.synergy;
                let top = i == 0;
                egui::Frame::new()
                    .fill(if top { Color32::from_rgba_unmultiplied(0xE6, 0xB8, 0x5C, 12) } else { Color32::TRANSPARENT })
                    .stroke(if top { Stroke::new(1.0, GOLD) } else { Stroke::NONE })
                    .corner_radius(CornerRadius::same(6))
                    .inner_margin(egui::Margin::symmetric(4, 4))
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            ui.add_sized([18.0, 32.0], egui::Label::new(RichText::new(format!("{}", i + 1)).size(11.5).color(if top { GOLD } else { DIM })));
                            imgs.paint_icon(ui, &r.champ.name, &r.champ.icon, Vec2::splat(30.0), 5.0, Color32::WHITE);
                            left_label(ui, 120.0, 32.0, RichText::new(&r.champ.name).size(13.0).strong().color(TEXT));
                            let sc_col = if r.score >= 0.0 { POS } else { NEG };
                            ui.allocate_ui_with_layout(Vec2::new(66.0, 30.0), Layout::top_down(Align::Max), |ui| {
                                ui.label(RichText::new(fmt_signed(r.score, 2)).size(13.0).strong().color(sc_col));
                                let frac = (r.score.abs() / top_score).clamp(0.0, 1.0) as f32;
                                let (brect, _) = ui.allocate_exact_size(Vec2::new(56.0, 3.0), Sense::hover());
                                ui.painter().rect_filled(brect, CornerRadius::same(2), LINE);
                                let mut fill = brect;
                                fill.set_width(brect.width() * frac);
                                ui.painter().rect_filled(fill, CornerRadius::same(2), sc_col);
                            });
                            paint_heat(ui, Some(strength), max_str, 62.0, "Overall strength in this role");
                            paint_heat(ui, Some(r.synergy), max_fit, 62.0, "Fit with your team's picks");

                            // Range track with "-1.6 ●━━━● +0.1" labels
                            let (rect, resp) = ui.allocate_exact_size(Vec2::new(track_w, 28.0), Sense::hover());
                            let left_x = rect.left() + 34.0;
                            let right_x = rect.right() - 34.0;
                            let x_of = |v: f64| left_x + ((v - lo) / (hi - lo)) as f32 * (right_x - left_x);
                            let cy = rect.center().y;
                            let p = ui.painter();
                            p.line_segment([Pos2::new(left_x, cy), Pos2::new(right_x, cy)], Stroke::new(1.0, LINE));
                            let zx = x_of(0.0);
                            p.line_segment([Pos2::new(zx, rect.top() + 5.0), Pos2::new(zx, rect.bottom() - 5.0)], Stroke::new(1.0, LINE2));
                            let x_tail = x_of(r.tail);
                            let x_mean = x_of(r.mean);
                            p.line_segment(
                                [Pos2::new(x_tail, cy), Pos2::new(x_mean, cy)],
                                Stroke::new(3.0, Color32::from_rgb(0x6E, 0x3D, 0x38)),
                            );
                            p.circle_filled(Pos2::new(x_tail, cy), 4.0, NEG);
                            p.circle_filled(Pos2::new(x_mean, cy), 4.0, TEXT);
                            p.text(Pos2::new(x_tail - 7.0, cy), Align2::RIGHT_CENTER, fmt_signed(r.tail, 1), FontId::proportional(10.5), NEG);
                            p.text(Pos2::new(x_mean + 7.0, cy), Align2::LEFT_CENTER, fmt_signed(r.mean, 1), FontId::proportional(10.5), TEXT2);
                            resp.on_hover_text(format!(
                                "{}: {} avg matchup, {} if countered",
                                r.champ.name,
                                fmt_signed(r.mean, 1),
                                fmt_signed(r.tail, 1)
                            ));

                            paint_heat(ui, Some(r.value), max_risk, 66.0, "Blind risk contribution");

                            ui.allocate_ui_with_layout(Vec2::new(94.0, 30.0), Layout::left_to_right(Align::Center), |ui| {
                                for c in r.counters.iter().take(3) {
                                    imgs.paint_icon(ui, &c.champ.name, &c.champ.icon, Vec2::splat(26.0), 5.0, Color32::WHITE)
                                        .on_hover_text(format!("{} {} ({} games)", c.champ.name, fmt_signed(c.delta, 1), c.games));
                                }
                            });

                            ui.horizontal(|ui| {
                                let (bar_r, _) = ui.allocate_exact_size(Vec2::new(32.0, 4.0), Sense::hover());
                                ui.painter().rect_filled(bar_r, CornerRadius::same(2), LINE);
                                let mut fill = bar_r;
                                fill.set_width(bar_r.width() * (r.coverage as f32 / 100.0).clamp(0.0, 1.0));
                                ui.painter().rect_filled(fill, CornerRadius::same(2), if r.coverage < 50 { GOLD } else { MUTED });
                                ui.label(
                                    RichText::new(format!("{}%", r.coverage))
                                        .size(10.5)
                                        .color(if r.coverage < 50 { GOLD } else { MUTED }),
                                );
                            });
                        });
                    });
                ui.add_space(2.0);
            }
        });
    });

    ui.label(
        RichText::new("Enemy laner not picked yet. Score = strength + team fit + blind risk. \"If countered\" is the average of the worst matchups that make up 15% of enemy picks.")
            .size(10.0)
            .color(DIM),
    );
}
