use egui::{Align, Color32, CornerRadius, Layout, RichText, Sense, Ui, Vec2};

use crate::engine::views::{BansView, ChampRef, ThreatAgainstView, ThreatView};

use crate::images::ImageCache;
use crate::ui::theme::{DIM, GOLD, LINE, MUTED, NEG, PANEL, RAISED, TEXT, TEXT2, fmt_signed, heat_bg, short_lane};

pub fn render_bans(ui: &mut Ui, d: &BansView, imgs: &mut ImageCache) {
    if d.threats.is_empty() {
        ui.centered_and_justified(|ui| {
            ui.label(RichText::new("No ban threats calculated yet.").color(MUTED));
        });
        return;
    }

    // Columns: picks + team hovers
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

    let avail_w = ui.available_width();
    let side_w = (avail_w * 0.34).clamp(240.0, 320.0);
    let main_w = (avail_w - side_w - 12.0).max(320.0);

    ui.horizontal_top(|ui| {
        ui.allocate_ui_with_layout(
            Vec2::new(main_w, ui.available_height()),
            Layout::top_down(Align::Min),
            |ui| {
                egui::ScrollArea::vertical().id_salt("bans_tbl").show(ui, |ui| {
                    // Header
                    ui.horizontal(|ui| {
                        ui.add_sized([22.0, 24.0], egui::Label::new(RichText::new("#").size(10.0).color(DIM)));
                        ui.add_sized([130.0, 24.0], egui::Label::new(RichText::new("BAN").size(10.0).color(MUTED)));
                        ui.add_sized([68.0, 24.0], egui::Label::new(RichText::new("THREAT").size(10.0).color(MUTED)));
                        ui.add_sized([48.0, 24.0], egui::Label::new(RichText::new("BASE").size(10.0).color(MUTED)));
                        for (c, is_pick, lane) in &pcols {
                            let resp = imgs.paint_icon(ui, &c.name, &c.icon, Vec2::splat(22.0), 11.0, Color32::WHITE);
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
                        let bg = if i == 0 {
                            Color32::from_rgba_unmultiplied(0xFF, 0x8A, 0x70, 24)
                        } else {
                            Color32::TRANSPARENT
                        };
                        egui::Frame::new()
                            .fill(bg)
                            .corner_radius(CornerRadius::same(4))
                            .inner_margin(egui::Margin::symmetric(4, 3))
                            .show(ui, |ui| {
                                ui.horizontal(|ui| {
                                    ui.add_sized([18.0, 28.0], egui::Label::new(RichText::new(format!("{}", i + 1)).size(11.0).color(DIM)));
                                    imgs.paint_icon(ui, &t.champ.name, &t.champ.icon, Vec2::splat(26.0), 4.0, Color32::WHITE);
                                    ui.allocate_ui_with_layout(Vec2::new(98.0, 28.0), Layout::top_down(Align::Min), |ui| {
                                        ui.label(RichText::new(&t.champ.name).size(12.0).strong().color(TEXT));
                                        ui.label(
                                            RichText::new(format!("{} · {:.1}%", short_lane(&t.lane), t.pick_rate))
                                                .size(9.5)
                                                .color(MUTED),
                                        );
                                    });
                                    // Threat score + bar
                                    ui.allocate_ui_with_layout(Vec2::new(66.0, 28.0), Layout::top_down(Align::Max), |ui| {
                                        let sc = t.score * 10.0;
                                        ui.label(RichText::new(format!("{sc:.1}")).size(12.5).strong().color(NEG));
                                        let frac = (sc.abs() / top_score).clamp(0.0, 1.0) as f32;
                                        let (brect, _) = ui.allocate_exact_size(Vec2::new(60.0, 3.0), Sense::hover());
                                        ui.painter().rect_filled(brect, CornerRadius::same(2), LINE);
                                        let mut fill = brect;
                                        fill.set_width(brect.width() * frac);
                                        ui.painter().rect_filled(fill, CornerRadius::same(2), NEG);
                                    });
                                    // Base
                                    ui.add_sized(
                                        [46.0, 24.0],
                                        egui::Label::new(RichText::new(fmt_signed(t.strength, 1)).size(11.0).color(TEXT2)),
                                    );
                                    // Against cells
                                    for (c, _, _) in &pcols {
                                        let found = t.against.iter().find(|a| a.champ.id == c.id);
                                        paint_cell(ui, found, max_abs, &t.champ.name, c);
                                    }
                                });
                            });
                    }
                });
            },
        );

        ui.add_space(8.0);
        // Side panel: BAN THIS
        ui.allocate_ui_with_layout(
            Vec2::new(side_w, ui.available_height()),
            Layout::top_down(Align::Min),
            |ui| {
                render_ban_side(ui, d, &pcols, imgs);
            },
        );
    });
}

fn paint_cell(
    ui: &mut Ui,
    a: Option<&ThreatAgainstView>,
    max_abs: f64,
    threat_name: &str,
    col: &ChampRef,
) {
    let size = Vec2::new(42.0, 24.0);
    let (rect, resp) = ui.allocate_exact_size(size, Sense::hover());
    if let Some(x) = a {
        let (bg, fg) = heat_bg(x.weighted, max_abs);
        ui.painter().rect_filled(rect.shrink(1.0), CornerRadius::same(4), bg);
        ui.painter().text(
            rect.center(),
            egui::Align2::CENTER_CENTER,
            fmt_signed(x.weighted, 1),
            egui::FontId::proportional(11.0),
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
            "·",
            egui::FontId::proportional(11.0),
            DIM,
        );
    }
}

fn render_ban_side(
    ui: &mut Ui,
    d: &BansView,
    pcols: &[(ChampRef, bool, String)],
    imgs: &mut ImageCache,
) {
    let t: &ThreatView = &d.threats[0];
    egui::Frame::new()
        .fill(PANEL)
        .stroke(egui::Stroke::new(1.0, LINE))
        .corner_radius(CornerRadius::same(6))
        .inner_margin(egui::Margin::same(10))
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                imgs.paint_icon(ui, &t.champ.name, &t.champ.icon, Vec2::splat(38.0), 6.0, Color32::WHITE);
                ui.vertical(|ui| {
                    ui.label(RichText::new("BAN THIS").size(9.5).strong().color(NEG));
                    ui.label(RichText::new(&t.champ.name).size(15.0).strong().color(TEXT));
                    if let Some(nx) = d.threats.get(1)
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
                        ui.label(RichText::new(format!("{:.1}", t.score * 10.0)).size(18.0).strong().color(NEG));
                        ui.label(RichText::new("threat").size(9.5).color(MUTED));
                    });
                });
            });

            ui.add_space(6.0);
            ui.separator();
            ui.add_space(6.0);

            fact_card(
                ui,
                &format!("Picked in {:.1}% of games", t.pick_rate),
                &format!("{} lane", short_lane(&t.lane)),
                None,
            );

            let worst = pcols
                .iter()
                .filter(|(_, is_pick, _)| *is_pick)
                .filter_map(|(c, _, _)| t.against.iter().find(|a| a.champ.id == c.id))
                .filter(|a| a.weighted < 0.0)
                .min_by(|a, b| a.weighted.total_cmp(&b.weighted));
            if let Some(w) = worst {
                fact_card(
                    ui,
                    &format!("Beats {}", w.champ.name),
                    &format!(
                        "{} loses {:.1} WR pts ({} games)",
                        w.champ.name,
                        w.delta.abs(),
                        w.games
                    ),
                    Some((fmt_signed(w.weighted, 1), NEG)),
                );
            }

            let strong = t.strength >= 0.0;
            fact_card(
                ui,
                if strong { "Strong on its own" } else { "Weak on its own" },
                &format!(
                    "{} avg WR in {}",
                    if strong { "Above" } else { "Below" },
                    short_lane(&t.lane)
                ),
                Some((fmt_signed(t.strength, 1), if strong { GOLD } else { MUTED })),
            );

            if let Some(nx) = d.threats.get(1) {
                ui.add_space(6.0);
                ui.label(
                    RichText::new(format!(
                        "If {} gets banned first, {} is next.",
                        t.champ.name, nx.champ.name
                    ))
                    .size(10.5)
                    .color(MUTED),
                );
            }
        });
}

fn fact_card(ui: &mut Ui, title: &str, sub: &str, right: Option<(String, Color32)>) {
    egui::Frame::new()
        .fill(RAISED)
        .corner_radius(CornerRadius::same(4))
        .inner_margin(egui::Margin::symmetric(8, 6))
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.vertical(|ui| {
                    ui.label(RichText::new(title).size(11.5).strong().color(TEXT));
                    ui.label(RichText::new(sub).size(10.0).color(MUTED));
                });
                if let Some((txt, col)) = right {
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        ui.label(RichText::new(txt).size(12.0).strong().color(col));
                    });
                }
            });
        });
    ui.add_space(4.0);
}
