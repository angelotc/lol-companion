use egui::{Align, Color32, CornerRadius, Layout, RichText, Sense, Stroke, Ui, Vec2};

use crate::engine::views::{BansView, ChampRef, ThreatAgainstView, ThreatView};

use crate::images::ImageCache;
use crate::ui::theme::{ALLY, DIM, GOLD, LINE, LINE2, MUTED, NEG, PANEL, RAISED, TEXT, fmt_signed, heat_bg, short_lane};

pub fn render_bans(ui: &mut Ui, d: &BansView, imgs: &mut ImageCache) {
    if d.threats.is_empty() {
        ui.centered_and_justified(|ui| {
            ui.label(RichText::new("No ban threats calculated yet.").color(MUTED));
        });
        return;
    }

    let sel_id = ui.data_mut(|data| {
        let id = egui::Id::new("bans_sel");
        let cur = data.get_temp::<i32>(id);
        if cur.is_some_and(|cid| d.threats.iter().any(|t| t.champ.id == cid)) {
            cur.unwrap()
        } else {
            d.threats[0].champ.id
        }
    });

    // Columns: picks first, then team hovers
    let mut pcols: Vec<(ChampRef, bool, String)> = Vec::new();
    let mut seen = std::collections::HashSet::new();
    for p in &d.picks {
        if seen.insert(p.id) {
            pcols.push((p.clone(), true, String::new()));
        }
    }
    for a in &d.allies {
        if !a.is_me && seen.insert(a.champ.id) {
            pcols.push((a.champ.clone(), false, a.lane.clone()));
        }
    }

    let n_picks = pcols.iter().filter(|x| x.1).count();
    let n_hovers = pcols.len() - n_picks;

    let max_abs = d
        .threats
        .iter()
        .flat_map(|t| t.against.iter().map(|a| a.weighted.abs()))
        .fold(1.0_f64, f64::max);
    let top_score = d
        .threats
        .iter()
        .map(|t| (t.score * 10.0).abs())
        .fold(0.01_f64, f64::max);
    let top_pr = d
        .threats
        .iter()
        .map(|t| t.pick_rate)
        .fold(1.0_f64, f64::max);

    let avail_w = ui.available_width();
    let avail_h = ui.available_height();
    let side_w = (avail_w * 0.31).clamp(260.0, 340.0);
    let main_w = (avail_w - side_w - 14.0).max(340.0);
    let cell_w = if pcols.len() >= 8 { 42.0 } else { 48.0 };

    ui.horizontal_top(|ui| {
        ui.allocate_ui_with_layout(
            Vec2::new(main_w, avail_h),
            Layout::top_down(Align::Min),
            |ui| {
                let tbl_h = (avail_h - 26.0).max(120.0);
                ui.allocate_ui(Vec2::new(main_w, tbl_h), |ui| {
                    egui::ScrollArea::both().id_salt("bans_tbl").show(ui, |ui| {
                        // Grouped header strip: YOUR LIKELY PICKS | TEAM HOVERS
                        if n_picks > 0 || n_hovers > 0 {
                            ui.horizontal(|ui| {
                                ui.add_space(22.0 + 132.0 + 70.0 + 50.0);
                                if n_picks > 0 {
                                    let w = n_picks as f32 * cell_w + (n_picks.saturating_sub(1) as f32) * 8.0;
                                    let (r, _) = ui.allocate_exact_size(Vec2::new(w, 16.0), Sense::hover());
                                    ui.painter().text(r.left_center(), egui::Align2::LEFT_CENTER, "YOUR LIKELY PICKS", egui::FontId::proportional(9.5), MUTED);
                                    ui.painter().line_segment(
                                        [egui::pos2(r.left(), r.bottom() - 1.0), egui::pos2(r.right(), r.bottom() - 1.0)],
                                        Stroke::new(1.5, LINE2),
                                    );
                                }
                                if n_picks > 0 && n_hovers > 0 {
                                    ui.add_space(6.0);
                                }
                                if n_hovers > 0 {
                                    let w = n_hovers as f32 * cell_w + (n_hovers.saturating_sub(1) as f32) * 8.0;
                                    let (r, _) = ui.allocate_exact_size(Vec2::new(w, 16.0), Sense::hover());
                                    ui.painter().text(r.left_center(), egui::Align2::LEFT_CENTER, "TEAM HOVERS", egui::FontId::proportional(9.5), ALLY);
                                    ui.painter().line_segment(
                                        [egui::pos2(r.left(), r.bottom() - 1.0), egui::pos2(r.right(), r.bottom() - 1.0)],
                                        Stroke::new(1.5, ALLY),
                                    );
                                }
                            });
                        }

                        // Column headers + portrait icons
                        ui.horizontal(|ui| {
                            ui.add_sized([22.0, 26.0], egui::Label::new(RichText::new("").size(10.0)));
                            ui.add_sized([132.0, 26.0], egui::Label::new(RichText::new("Ban").size(10.5).color(MUTED)));
                            ui.add_sized([70.0, 26.0], egui::Label::new(RichText::new("Threat").size(10.5).color(MUTED)));
                            ui.add_sized([50.0, 26.0], egui::Label::new(RichText::new("Base").size(10.5).color(MUTED)));
                            for (i, (c, is_pick, lane)) in pcols.iter().enumerate() {
                                if i == n_picks && n_picks > 0 {
                                    ui.add_space(6.0);
                                }
                                let (rect, resp) = ui.allocate_exact_size(Vec2::new(cell_w, 26.0), Sense::hover());
                                let ico_r = egui::Rect::from_center_size(rect.center(), Vec2::splat(22.0));
                                let mut sub = ui.new_child(egui::UiBuilder::new().max_rect(ico_r));
                                imgs.paint_icon(&mut sub, &c.name, &c.icon, Vec2::splat(22.0), 4.0, Color32::WHITE);
                                let tip = if *is_pick {
                                    format!("{}, one of your likely picks", c.name)
                                } else {
                                    format!("{}, {} hover", c.name, short_lane(lane))
                                };
                                resp.on_hover_text(tip);
                            }
                        });
                        ui.separator();

                        for (i, t) in d.threats.iter().enumerate() {
                            let selected = t.champ.id == sel_id;
                            let bg = if selected {
                                Color32::from_rgba_unmultiplied(0xFF, 0x8A, 0x70, 20)
                            } else {
                                Color32::TRANSPARENT
                            };
                            let stroke = if selected {
                                Stroke::new(1.0, NEG)
                            } else {
                                Stroke::NONE
                            };
                            let resp = egui::Frame::new()
                                .fill(bg)
                                .stroke(stroke)
                                .corner_radius(CornerRadius::same(6))
                                .inner_margin(egui::Margin::symmetric(4, 5))
                                .show(ui, |ui| {
                                    ui.horizontal(|ui| {
                                        ui.add_sized(
                                            [18.0, 32.0],
                                            egui::Label::new(
                                                RichText::new(format!("{}", i + 1))
                                                    .size(11.5)
                                                    .color(if selected { NEG } else { DIM }),
                                            ),
                                        );
                                        imgs.paint_icon(ui, &t.champ.name, &t.champ.icon, Vec2::splat(28.0), 4.0, Color32::WHITE);
                                        ui.allocate_ui_with_layout(Vec2::new(98.0, 32.0), Layout::top_down(Align::Min), |ui| {
                                            ui.label(RichText::new(&t.champ.name).size(12.5).strong().color(TEXT));
                                            ui.label(
                                                RichText::new(format!("{} · picked {:.1}%", short_lane(&t.lane), t.pick_rate))
                                                    .size(9.5)
                                                    .color(MUTED),
                                            );
                                        });
                                        // Threat score + bar
                                        ui.allocate_ui_with_layout(Vec2::new(66.0, 32.0), Layout::top_down(Align::Max), |ui| {
                                            let sc = t.score * 10.0;
                                            ui.label(RichText::new(format!("{sc:.1}")).size(13.5).strong().color(NEG));
                                            let frac = (sc.abs() / top_score).clamp(0.0, 1.0) as f32;
                                            let (brect, _) = ui.allocate_exact_size(Vec2::new(58.0, 3.0), Sense::hover());
                                            ui.painter().rect_filled(brect, CornerRadius::same(2), LINE);
                                            let mut fill = brect;
                                            fill.set_width(brect.width() * frac);
                                            ui.painter().rect_filled(fill, CornerRadius::same(2), NEG);
                                        });
                                        // Base strength pill
                                        let bbg = if t.strength >= 0.05 {
                                            Color32::from_rgba_unmultiplied(0xE6, 0xB8, 0x5C, 28)
                                        } else {
                                            RAISED
                                        };
                                        let bfg = if t.strength >= 0.05 { TEXT } else { MUTED };
                                        let (br, _) = ui.allocate_exact_size(Vec2::new(46.0, 24.0), Sense::hover());
                                        ui.painter().rect_filled(br.shrink(2.0), CornerRadius::same(4), bbg);
                                        ui.painter().text(
                                            br.center(),
                                            egui::Align2::CENTER_CENTER,
                                            fmt_signed(t.strength, 1),
                                            egui::FontId::proportional(11.5),
                                            bfg,
                                        );

                                        // Against cells
                                        for (ci, (c, _, _)) in pcols.iter().enumerate() {
                                            if ci == n_picks && n_picks > 0 {
                                                ui.add_space(6.0);
                                            }
                                            let found = t.against.iter().find(|a| a.champ.id == c.id);
                                            paint_cell(ui, found, max_abs, cell_w, &t.champ.name, c);
                                        }
                                    });
                                })
                                .response
                                .interact(Sense::click());
                            if resp.clicked() {
                                ui.data_mut(|data| data.insert_temp(egui::Id::new("bans_sel"), t.champ.id));
                            }
                            ui.add_space(2.0);
                        }
                    });
                });

                ui.label(
                    RichText::new("Each cell is how that pick's win rate moves against the ban candidate, discounted for small samples and weighted by lane. Coral means it beats that pick, which is what makes it worth banning.")
                        .size(10.0)
                        .color(DIM),
                );
            },
        );

        ui.add_space(12.0);
        // Side panel
        let sel_idx = d.threats.iter().position(|t| t.champ.id == sel_id).unwrap_or(0);
        ui.allocate_ui_with_layout(
            Vec2::new(side_w, avail_h),
            Layout::top_down(Align::Min),
            |ui| {
                render_ban_side(ui, d, sel_idx, top_pr, &pcols, imgs);
            },
        );
    });
}

fn paint_cell(
    ui: &mut Ui,
    a: Option<&ThreatAgainstView>,
    max_abs: f64,
    cell_w: f32,
    threat_name: &str,
    col: &ChampRef,
) {
    let size = Vec2::new(cell_w, 26.0);
    let (rect, resp) = ui.allocate_exact_size(size, Sense::hover());
    if let Some(x) = a {
        let (bg, fg) = heat_bg(x.weighted, max_abs);
        ui.painter().rect_filled(rect.shrink(1.0), CornerRadius::same(4), bg);
        ui.painter().text(
            rect.center(),
            egui::Align2::CENTER_CENTER,
            fmt_signed(x.weighted, 1),
            egui::FontId::proportional(11.5),
            fg,
        );
        resp.on_hover_text(format!(
            "{} vs {}: {} win rate in {} games → {}",
            col.name,
            threat_name,
            fmt_signed(x.delta, 1),
            x.games,
            fmt_signed(x.weighted, 2)
        ));
    } else {
        ui.painter().text(
            rect.center(),
            egui::Align2::CENTER_CENTER,
            "0.0",
            egui::FontId::proportional(11.0),
            DIM,
        );
    }
}

fn render_ban_side(
    ui: &mut Ui,
    d: &BansView,
    sel_idx: usize,
    top_pr: f64,
    pcols: &[(ChampRef, bool, String)],
    imgs: &mut ImageCache,
) {
    let t: &ThreatView = &d.threats[sel_idx];
    let other_idx = if sel_idx == 0 { 1 } else { 0 };
    let nx = d.threats.get(other_idx);

    egui::Frame::new()
        .fill(PANEL)
        .stroke(Stroke::new(1.0, LINE))
        .corner_radius(CornerRadius::same(8))
        .inner_margin(egui::Margin::same(12))
        .show(ui, |ui| {
            ui.set_min_height((ui.available_height() - 4.0).max(160.0));
            ui.horizontal(|ui| {
                imgs.paint_icon(ui, &t.champ.name, &t.champ.icon, Vec2::splat(42.0), 6.0, Color32::WHITE);
                ui.add_space(4.0);
                ui.vertical(|ui| {
                    ui.label(
                        RichText::new(if sel_idx == 0 {
                            "BAN THIS".to_string()
                        } else {
                            format!("BAN #{}", sel_idx + 1)
                        })
                        .size(9.5)
                        .strong()
                        .color(NEG),
                    );
                    ui.label(RichText::new(&t.champ.name).size(16.0).strong().color(TEXT));
                    if sel_idx == 0
                        && let Some(nx) = nx
                        && nx.score > 0.0
                        && t.score > 0.0
                    {
                        let ratio = t.score / nx.score;
                        let sub = if ratio >= 1.15 {
                            format!("{ratio:.1}× the threat of {}", nx.champ.name)
                        } else {
                            format!("Close to {} ({:.1})", nx.champ.name, nx.score * 10.0)
                        };
                        ui.label(RichText::new(sub).size(10.5).color(MUTED));
                    }
                });
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    ui.vertical(|ui| {
                        ui.label(RichText::new(format!("{:.1}", t.score * 10.0)).size(20.0).strong().color(NEG));
                        ui.label(RichText::new("threat").size(9.5).color(MUTED));
                    });
                });
            });

            ui.add_space(8.0);
            ui.separator();
            ui.add_space(6.0);
            ui.label(
                RichText::new("THREAT = HOW OFTEN YOU FACE IT × HOW MUCH IT HURTS")
                    .size(9.5)
                    .strong()
                    .color(MUTED),
            );
            ui.add_space(6.0);

            // Pick rate row with horizontal bar
            ui.horizontal(|ui| {
                ui.vertical(|ui| {
                    ui.label(RichText::new(format!("Picked in {:.1}% of games", t.pick_rate)).size(12.0).strong().color(TEXT));
                    ui.label(
                        RichText::new(if (t.pick_rate - top_pr).abs() < 0.05 {
                            "Most picked on this list".to_string()
                        } else {
                            format!("{} lane", short_lane(&t.lane))
                        })
                        .size(10.0)
                        .color(MUTED),
                    );
                });
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    let frac = (t.pick_rate / top_pr).clamp(0.0, 1.0) as f32;
                    let (r, _) = ui.allocate_exact_size(Vec2::new(56.0, 6.0), Sense::hover());
                    ui.painter().rect_filled(r, CornerRadius::same(3), LINE);
                    let mut fill = r;
                    fill.set_width(r.width() * frac);
                    ui.painter().rect_filled(fill, CornerRadius::same(3), GOLD);
                });
            });
            ui.add_space(8.0);

            // Worst pick hit
            let worst_pick = pcols
                .iter()
                .filter(|(_, is_pick, _)| *is_pick)
                .filter_map(|(c, _, _)| t.against.iter().find(|a| a.champ.id == c.id))
                .filter(|a| a.weighted < 0.0)
                .min_by(|a, b| a.weighted.total_cmp(&b.weighted));
            if let Some(w) = worst_pick {
                reason_line(
                    ui,
                    &format!("Beats {}", w.champ.name),
                    &format!("{} loses {:.1} win-rate points into it ({} games)", w.champ.name, w.delta.abs(), w.games),
                    fmt_signed(w.weighted, 1),
                    NEG,
                );
            }

            // Worst team hover hit
            let worst_hover = pcols
                .iter()
                .filter(|(_, is_pick, _)| !*is_pick)
                .filter_map(|(c, _, _)| t.against.iter().find(|a| a.champ.id == c.id))
                .filter(|a| a.weighted < 0.0)
                .min_by(|a, b| a.weighted.total_cmp(&b.weighted));
            if let Some(w) = worst_hover {
                reason_line(
                    ui,
                    &format!("Bad for {}'s hover too", w.champ.name),
                    "Counted with ally lane weights",
                    fmt_signed(w.weighted, 1),
                    NEG,
                );
            }

            let strong = t.strength >= 0.0;
            reason_line(
                ui,
                if strong { "Strong on its own" } else { "Weak on its own" },
                &format!(
                    "{} the average win rate for {}",
                    if strong { "Above" } else { "Below" },
                    short_lane(&t.lane).to_lowercase()
                ),
                fmt_signed(t.strength, 1),
                if strong { TEXT } else { MUTED },
            );

            if let Some(nx) = d.threats.get(1)
                && sel_idx == 0
            {
                ui.add_space(10.0);
                egui::Frame::new()
                    .fill(RAISED)
                    .stroke(Stroke::new(1.0, LINE))
                    .corner_radius(CornerRadius::same(6))
                    .inner_margin(egui::Margin::same(10))
                    .show(ui, |ui| {
                        ui.label(
                            RichText::new(format!(
                                "If {} gets banned first, {} is next.",
                                t.champ.name, nx.champ.name
                            ))
                            .size(11.0)
                            .color(MUTED),
                        );
                    });
            }
        });
}

fn reason_line(ui: &mut Ui, title: &str, sub: &str, val: String, col: Color32) {
    ui.horizontal(|ui| {
        ui.vertical(|ui| {
            ui.label(RichText::new(title).size(12.0).strong().color(TEXT));
            ui.label(RichText::new(sub).size(10.0).color(MUTED));
        });
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            ui.label(RichText::new(val).size(13.0).strong().color(col));
        });
    });
    ui.add_space(8.0);
}
