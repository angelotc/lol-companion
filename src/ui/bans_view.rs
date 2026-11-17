use egui::{Align, Color32, CornerRadius, FontId, Layout, Rect, RichText, Sense, Stroke, Ui, Vec2};

use crate::engine::views::{BansView, ChampRef, ThreatAgainstView, ThreatView};

use crate::images::ImageCache;
use crate::ui::picks_view::{
    TableColumns, detail_heading, paint_rank, paint_row, paint_score, paint_table_champion, paint_value,
    table_divider, table_icon, table_panel_widths, table_text, table_with_note,
};
use crate::ui::theme::{ACCENT, ALLY, DIM, LINE, MUTED, NEG, PANEL, RAISED, TEXT, fmt_signed, heat_bg, short_lane, strong, strong_font};

pub fn render_bans(ui: &mut Ui, d: &BansView, imgs: &mut ImageCache) {
    if d.threats.is_empty() {
        ui.centered_and_justified(|ui| {
            ui.label(RichText::new("No ban threats calculated yet.").color(MUTED));
        });
        return;
    }

    let mut sel_id = ui.data_mut(|data| {
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

    let (main_w, side_w, gap) = table_panel_widths(ui);
    let height = ui.available_height();

    ui.horizontal_top(|ui| {
        ui.allocate_ui_with_layout(Vec2::new(main_w, height), Layout::top_down(Align::Min), |ui| {
            table_with_note(
                ui,
                "bans_tbl",
                "Each cell is how that pick's win rate moves against the ban candidate, discounted for small samples and weighted by lane. Coral means it beats that pick, which is what makes it worth banning.",
                |ui| {
                    const FIRST_MATCHUP: usize = 4;
                    let hovers_start = FIRST_MATCHUP + n_picks;
                    let groups = if n_picks > 0 && n_hovers > 0 { &[hovers_start][..] } else { &[] };
                    let columns = TableColumns::matchups(ui.available_width(), pcols.len() + 1, groups);
                    if !pcols.is_empty() {
                        let group_height = if n_picks <= 2 || n_hovers == 1 { 28.0 } else { 19.0 };
                        let (group_row, _) = columns.row(ui, group_height, Sense::hover());
                        columns.group(ui, group_row, FIRST_MATCHUP..hovers_start, "YOUR LIKELY PICKS", ACCENT);
                        columns.group(ui, group_row, hovers_start..FIRST_MATCHUP + pcols.len(), "TEAM HOVERS", ALLY);
                    }

                    let (header, header_resp) = columns.row(ui, 30.0, Sense::hover());
                    columns.champion_heading(ui, header, "Ban");
                    columns.heading(ui, header, 2, "Threat", Align::Max);
                    columns.heading(ui, header, 3, "Base", Align::Center);
                    for (idx, (champ, is_pick, lane)) in pcols.iter().enumerate() {
                        let col = FIRST_MATCHUP + idx;
                        let cell = columns.cell(header, col);
                        table_icon(ui, imgs, champ, Rect::from_center_size(cell.center(), Vec2::splat(26.0)), Color32::WHITE);
                        let tip = if *is_pick {
                            format!("{}, one of your likely picks", champ.name)
                        } else {
                            format!("{}, {} hover", champ.name, short_lane(lane))
                        };
                        ui.interact(cell, header_resp.id.with(col), Sense::hover()).on_hover_text(tip);
                    }
                    table_divider(ui, header);

                    for (i, t) in d.threats.iter().enumerate() {
                        let selected = t.champ.id == sel_id;
                        let (row, resp) = columns.row(ui, 55.0, Sense::click());
                        let resp = resp.on_hover_cursor(egui::CursorIcon::PointingHand);
                        if resp.clicked() {
                            sel_id = t.champ.id;
                            ui.data_mut(|data| data.insert_temp(egui::Id::new("bans_sel"), sel_id));
                        }
                        paint_row(ui, row, selected.then_some(NEG), resp.hovered(), false);
                        paint_rank(ui, columns.cell(row, 0), i, if selected { NEG } else { DIM });
                        paint_table_champion(ui, imgs, columns.cell(row, 1), &t.champ);
                        let name = columns.champion_name(row);
                        table_text(ui, name.with_max_y(row.top() + 44.0), Align::Min, &t.champ.name, strong_font(14.0), TEXT);
                        table_text(
                            ui,
                            name.with_min_y(row.top() + 26.0),
                            Align::Min,
                            &format!("{} · picked {:.1}%", short_lane(&t.lane), t.pick_rate),
                            FontId::proportional(11.0),
                            MUTED,
                        );
                        let score = t.score * 10.0;
                        paint_score(ui, columns.cell(row, 2), &format!("{score:.1}"), NEG, (score.abs() / top_score) as f32);

                        let (base_bg, base_fg) = if t.strength >= 0.05 {
                            (ACCENT.gamma_multiply(0.12), TEXT)
                        } else {
                            (RAISED, MUTED)
                        };
                        paint_value(ui, columns.cell(row, 3), &fmt_signed(t.strength, 1), base_bg, base_fg);

                        for (idx, (champ, _, _)) in pcols.iter().enumerate() {
                            let col = FIRST_MATCHUP + idx;
                            let against = t.against.iter().find(|a| a.champ.id == champ.id);
                            paint_cell(ui, columns.cell(row, col), resp.id.with(col), against, max_abs, &t.champ.name, champ);
                        }
                    }
                },
            );
        });

        ui.add_space(gap);
        let sel_idx = d.threats.iter().position(|t| t.champ.id == sel_id).unwrap_or(0);
        ui.allocate_ui_with_layout(Vec2::new(side_w, height), Layout::top_down(Align::Min), |ui| {
            render_ban_side(ui, d, sel_idx, top_pr, &pcols, imgs);
        });
    });
}

fn paint_cell(
    ui: &Ui,
    cell: Rect,
    id: egui::Id,
    against: Option<&ThreatAgainstView>,
    max_abs: f64,
    threat_name: &str,
    col: &ChampRef,
) {
    if let Some(x) = against {
        let (bg, fg) = heat_bg(x.weighted, max_abs);
        paint_value(ui, cell, &fmt_signed(x.weighted, 1), bg, fg);
        ui.interact(cell, id, Sense::hover()).on_hover_text(format!(
            "{} vs {}: {} win rate in {} games → {}",
            col.name,
            threat_name,
            fmt_signed(x.delta, 1),
            x.games,
            fmt_signed(x.weighted, 2)
        ));
    } else {
        table_text(ui, cell, Align::Center, "0.0", strong_font(12.0), DIM);
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

    let panel_size = ui.available_size();
    egui::Frame::new()
        .fill(PANEL)
        .stroke(Stroke::new(1.0, LINE))
        .corner_radius(CornerRadius::same(8))
        .inner_margin(egui::Margin::same(12))
        .show(ui, |ui| {
            ui.set_min_size(panel_size - Vec2::splat(26.0));
            let kicker = if sel_idx == 0 { "BAN THIS".to_owned() } else { format!("BAN #{}", sel_idx + 1) };
            detail_heading(ui, imgs, &t.champ, &kicker, NEG, &format!("{:.1}", t.score * 10.0), NEG, "threat");
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
                ui.label(RichText::new(sub).size(11.5).color(MUTED));
            }

            ui.add_space(8.0);
            ui.separator();
            ui.add_space(6.0);
            ui.label(
                strong("THREAT = HOW OFTEN YOU FACE IT × HOW MUCH IT HURTS")
                    .size(10.0)
                    .color(MUTED),
            );
            ui.add_space(6.0);

            // Pick rate row with horizontal bar
            ui.horizontal(|ui| {
                ui.vertical(|ui| {
                    ui.label(strong(format!("Picked in {:.1}% of games", t.pick_rate)).size(13.0).color(TEXT));
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
                    ui.painter().rect_filled(fill, CornerRadius::same(3), ACCENT);
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
            ui.label(strong(title).size(13.0).color(TEXT));
            ui.label(RichText::new(sub).size(10.0).color(MUTED));
        });
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            ui.label(strong(val).size(16.0).color(col));
        });
    });
    ui.add_space(8.0);
}
