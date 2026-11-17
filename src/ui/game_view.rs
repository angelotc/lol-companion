use egui::{Align, Color32, CornerRadius, Layout, RichText, Sense, Stroke, Ui, Vec2};

use crate::engine::views::{GameScreen, IngameBuildView, PartView, PlayerView, TeamView};

use crate::images::ImageCache;
use crate::ui::build_view::render_damage_bar;
use crate::ui::theme::{ALLY, BG, BG2, DIM, ENEMY, GOLD, LINE2, MUTED, NEG, PANEL, POS, RAISED, TEXT, fmt_signed, tier_color};

fn fmt_clock(secs: i32) -> String {
    let m = secs.max(0) / 60;
    let s = secs.max(0) % 60;
    format!("{m}:{s:02}")
}

fn fmt_k(n: i32) -> String {
    if n >= 1000 {
        format!("{:.1}k", n as f64 / 1000.0)
    } else {
        n.to_string()
    }
}

pub fn render_game_overlay(ui: &mut Ui, g: &GameScreen, imgs: &mut ImageCache) {
    egui::Frame::new()
        .fill(BG)
        .stroke(Stroke::new(1.0, LINE2))
        .inner_margin(egui::Margin::same(6))
        .show(ui, |ui| {
            let avail_h = (ui.available_height() - 18.0).max(100.0);
            ui.columns(2, |cols| {
                // Left column: Players
                cols[0].allocate_ui(Vec2::new(cols[0].available_width(), avail_h), |ui| {
                    egui::ScrollArea::vertical()
                        .id_salt("ig_players")
                        .auto_shrink([false, false])
                        .show(ui, |ui| {
                            if let Some(teams) = &g.teams {
                                for t in teams {
                                    render_team(ui, t, imgs);
                                    ui.add_space(6.0);
                                }
                            } else {
                                ui.centered_and_justified(|ui| {
                                    ui.label(RichText::new(&g.players_note).size(11.0).color(MUTED));
                                });
                            }
                        });
                });

                // Right column: Build
                cols[1].allocate_ui(Vec2::new(cols[1].available_width(), avail_h), |ui| {
                    egui::ScrollArea::vertical()
                        .id_salt("ig_build")
                        .auto_shrink([false, false])
                        .show(ui, |ui| {
                            if let Some(b) = &g.build {
                                render_ig_build(ui, b, imgs);
                            } else {
                                ui.centered_and_justified(|ui| {
                                    ui.label(RichText::new(&g.build_note).size(11.0).color(MUTED));
                                });
                            }
                        });
                });
            });

            ui.separator();
            ui.centered_and_justified(|ui| {
                ui.label(RichText::new("Ctrl+X to hide · hover for details").size(9.5).color(DIM));
            });
        });
}

fn render_team(ui: &mut Ui, t: &TeamView, imgs: &mut ImageCache) {
    let is_ally = t.side == "ally";
    let accent = if is_ally { ALLY } else { ENEMY };
    ui.horizontal(|ui| {
        let (r, _) = ui.allocate_exact_size(Vec2::new(2.0, 12.0), Sense::hover());
        ui.painter().rect_filled(r, CornerRadius::ZERO, accent);
        ui.label(RichText::new(t.label.to_uppercase()).size(9.5).strong().color(accent));
    });
    ui.add_space(2.0);
    if t.players.is_empty() {
        ui.label(RichText::new("No players").size(10.0).color(DIM));
        return;
    }
    for p in &t.players {
        render_player_row(ui, p, is_ally, imgs);
        ui.add_space(2.0);
    }
}

fn render_player_row(ui: &mut Ui, p: &PlayerView, is_ally: bool, imgs: &mut ImageCache) {
    let bg = if p.me {
        Color32::from_rgba_unmultiplied(0xE6, 0xB8, 0x5C, 28)
    } else if is_ally {
        PANEL
    } else {
        Color32::from_rgb(0x14, 0x07, 0x07)
    };
    let border_col = if p.me { GOLD } else if is_ally { ALLY } else { ENEMY };

    let resp = egui::Frame::new()
        .fill(bg)
        .corner_radius(CornerRadius::same(4))
        .inner_margin(egui::Margin::symmetric(5, 3))
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                let (bar, _) = ui.allocate_exact_size(Vec2::new(2.0, 30.0), Sense::hover());
                ui.painter().rect_filled(bar, CornerRadius::ZERO, border_col);

                if let Some(c) = &p.champ {
                    imgs.paint_icon(ui, &c.name, &c.icon, Vec2::splat(32.0), 5.0, Color32::WHITE);
                } else {
                    imgs.paint_icon(ui, "?", "", Vec2::splat(32.0), 5.0, Color32::WHITE);
                }

                ui.vertical(|ui| {
                    ui.horizontal(|ui| {
                        ui.add(egui::Label::new(RichText::new(&p.name).size(10.5).strong().color(TEXT)).truncate());
                        if !p.rank.is_empty() {
                            ui.label(RichText::new(&p.rank).size(9.5).strong().color(tier_color(&p.tier)));
                        }
                        if let Some(wr) = p.wr {
                            let col = if wr >= 55 { POS } else if wr <= 45 { NEG } else { MUTED };
                            let txt = if p.season_games > 0 {
                                format!("{wr}% ({})", p.season_games)
                            } else {
                                format!("{wr}%")
                            };
                            ui.label(RichText::new(txt).size(9.5).color(col));
                        } else if p.wins > 0 {
                            ui.label(RichText::new(format!("{}W", p.wins)).size(9.5).color(MUTED));
                        }
                    });

                    if let Some(sg) = p.champ_season_games {
                        ui.horizontal(|ui| {
                            ui.label(RichText::new(format!("{sg} games")).size(9.5).strong().color(TEXT));
                            if sg > 0
                                && let Some(swr) = p.champ_season_wr
                            {
                                let col = if swr >= 55 { POS } else if swr <= 45 { NEG } else { MUTED };
                                ui.label(RichText::new(format!("{swr}% WR")).size(9.5).color(col));
                            }
                        });
                    }

                    ui.horizontal(|ui| {
                        let rec_txt = if p.champ_games > 0 {
                            let prefix = if p.champ_season_games.is_some() { "Recent: " } else { "" };
                            let wr = p.champ_wr.map(|w| format!(" · {w}%")).unwrap_or_default();
                            let kda = p.kda.map(|k| format!(" · {k:.1} KDA")).unwrap_or_default();
                            format!("{prefix}{} games{wr}{kda}", p.champ_games)
                        } else if p.games > 0 {
                            format!("not in last {} games", p.games)
                        } else {
                            "no recent games".into()
                        };
                        ui.label(RichText::new(rec_txt).size(9.0).color(MUTED));

                        if !p.recent.is_empty() {
                            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                for ch in p.recent.chars().rev() {
                                    let (d, _) = ui.allocate_exact_size(Vec2::splat(4.5), Sense::hover());
                                    ui.painter().circle_filled(
                                        d.center(),
                                        2.2,
                                        if ch == 'W' { POS } else { NEG },
                                    );
                                }
                            });
                        }
                    });

                    if !p.tags.is_empty() {
                        ui.horizontal_wrapped(|ui| {
                            for tag in &p.tags {
                                let bad = tag.starts_with("First time") || tag.contains("losses in a row");
                                let (tbg, tfg) = if bad {
                                    (Color32::from_rgba_unmultiplied(0xFF, 0x5F, 0x5F, 45), NEG)
                                } else {
                                    (Color32::from_rgba_unmultiplied(0x5F, 0xFF, 0x87, 40), POS)
                                };
                                egui::Frame::new()
                                    .fill(tbg)
                                    .corner_radius(CornerRadius::same(6))
                                    .inner_margin(egui::Margin::symmetric(4, 1))
                                    .show(ui, |ui| {
                                        ui.label(RichText::new(tag).size(8.5).strong().color(tfg));
                                    });
                            }
                        });
                    }
                });
            });
        })
        .response;

    let mut tip = format!(
        "{}{}{}\n{}{}",
        p.name,
        if p.tag.is_empty() { String::new() } else { format!("#{}", p.tag) },
        if p.lane.is_empty() { String::new() } else { format!(" ({})", p.lane) },
        p.rank_full,
        if p.queue == "flex" { " (Flex)" } else { "" }
    );
    if let (Some(k), Some(d), Some(a)) = (p.kills, p.deaths, p.assists) {
        tip.push_str(&format!("\nAvg K/D/A: {k:.1} / {d:.1} / {a:.1}"));
    }
    if let Some(m) = p.mastery {
        tip.push_str(&format!("\nMastery: {m} pts"));
    }
    if p.error {
        tip.push_str("\nSome data could not be loaded");
    }
    resp.on_hover_text(tip);
}

fn paint_item_icon(ui: &mut Ui, p: &PartView, sz: f32, imgs: &mut ImageCache) {
    let tint = if p.owned {
        Color32::from_rgba_unmultiplied(160, 160, 160, 100)
    } else {
        Color32::WHITE
    };
    let resp = imgs.paint_icon(ui, &p.name, &p.icon, Vec2::splat(sz), 3.0, tint);
    if p.owned {
        let c = resp.rect.center();
        let stroke = Stroke::new(2.0, POS);
        ui.painter().line_segment(
            [egui::pos2(c.x - sz * 0.22, c.y + sz * 0.02), egui::pos2(c.x - sz * 0.04, c.y + sz * 0.20)],
            stroke,
        );
        ui.painter().line_segment(
            [egui::pos2(c.x - sz * 0.04, c.y + sz * 0.20), egui::pos2(c.x + sz * 0.24, c.y - sz * 0.18)],
            stroke,
        );
    }
    let tip = if p.gold > 0 {
        format!("{} · {}g", p.name, p.gold)
    } else {
        p.name.clone()
    };
    resp.on_hover_text(tip);
}

fn render_ig_build(ui: &mut Ui, s: &IngameBuildView, imgs: &mut ImageCache) {
    ui.horizontal(|ui| {
        if let Some(c) = &s.champ {
            imgs.paint_icon(ui, &c.name, &c.icon, Vec2::splat(32.0), 5.0, Color32::WHITE);
        }
        ui.vertical(|ui| {
            let cname = s.champ.as_ref().map_or("", |c| c.name.as_str());
            ui.label(RichText::new(format!("{cname} · lvl {}", s.level)).size(12.0).strong().color(TEXT));
            ui.label(
                RichText::new(format!("{} · {} gold", fmt_clock(s.time_secs), fmt_k(s.gold)))
                    .size(10.0)
                    .color(MUTED),
            );
        });
        if let Some(k) = &s.keystone {
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                imgs.paint_icon(ui, &k.name, &k.icon, Vec2::splat(26.0), 13.0, Color32::WHITE)
                    .on_hover_text(&k.name);
            });
        }
    });

    if let Some(dmg) = &s.damage {
        ui.add_space(4.0);
        render_damage_bar(ui, dmg);
    }

    if let Some(empty) = &s.empty {
        ui.add_space(6.0);
        ui.label(RichText::new(empty).size(10.5).color(DIM));
        return;
    }

    if !s.core.is_empty() {
        ui.add_space(6.0);
        ui.label(RichText::new("CORE").size(9.0).strong().color(DIM));
        ui.horizontal(|ui| {
            for (i, p) in s.core.iter().enumerate() {
                if i > 0 {
                    ui.label(RichText::new("›").size(10.0).color(DIM));
                }
                paint_item_icon(ui, p, 26.0, imgs);
            }
        });
        if let Some(nxt) = &s.next {
            let need = (nxt.gold - s.gold).max(0);
            ui.horizontal(|ui| {
                ui.label(RichText::new(format!("Next: {}", nxt.name)).size(10.0).color(MUTED));
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if need > 0 {
                        ui.label(RichText::new(format!("{} more", fmt_k(need))).size(10.0).strong().color(TEXT));
                    } else {
                        ui.label(RichText::new("affordable").size(10.0).strong().color(POS));
                    }
                });
            });
        }
    }

    let opp_cols: Vec<_> = s.opponents.iter().take(4).collect();
    for sec in &s.sections {
        ui.add_space(5.0);
        let is_next = sec.state == "next";
        let bg = if is_next {
            Color32::from_rgba_unmultiplied(0xFF, 0xD7, 0x5F, 18)
        } else {
            Color32::TRANSPARENT
        };
        let stroke = if is_next {
            Stroke::new(1.0, Color32::from_rgba_unmultiplied(0xFF, 0xD7, 0x5F, 140))
        } else {
            Stroke::NONE
        };
        egui::Frame::new()
            .fill(bg)
            .stroke(stroke)
            .corner_radius(CornerRadius::same(4))
            .inner_margin(egui::Margin::same(4))
            .show(ui, |ui| {
                let title = if is_next {
                    format!("{} · NEXT", sec.title.to_uppercase())
                } else {
                    sec.title.to_uppercase()
                };
                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new(title)
                            .size(9.0)
                            .strong()
                            .color(if is_next { GOLD } else { DIM }),
                    );
                    if !opp_cols.is_empty() && sec.options.iter().any(|o| !o.vs.is_empty()) {
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            ui.add_space(34.0);
                            for (e, lane) in opp_cols.iter().rev() {
                                let (r, resp) = ui.allocate_exact_size(Vec2::new(28.0, 18.0), Sense::hover());
                                let ico_r = egui::Rect::from_center_size(r.center(), Vec2::splat(18.0));
                                let mut sub = ui.new_child(egui::UiBuilder::new().max_rect(ico_r));
                                imgs.paint_icon(&mut sub, &e.name, &e.icon, Vec2::splat(18.0), 4.0, Color32::WHITE);
                                resp.on_hover_text(format!("{} ({lane})", e.name));
                            }
                        });
                    }
                });
                for o in &sec.options {
                    ui.horizontal(|ui| {
                        for p in &o.parts {
                            paint_item_icon(ui, p, 26.0, imgs);
                        }
                        ui.label(RichText::new(format!("{:.1}%", o.wr)).size(10.5).strong().color(TEXT));
                        if o.yours {
                            ui.label(RichText::new("YOURS").size(8.5).strong().color(POS));
                        }
                        if o.has_vs {
                            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                let col = if o.matchup >= 0.3 {
                                    POS
                                } else if o.matchup <= -0.3 {
                                    NEG
                                } else {
                                    DIM
                                };
                                let txt = if o.matchup.abs() < 0.3 {
                                    "±0".into()
                                } else {
                                    fmt_signed(o.matchup, 1)
                                };
                                ui.add_sized([30.0, 20.0], egui::Label::new(RichText::new(txt).size(10.5).strong().color(col)));
                                for (e, _) in opp_cols.iter().rev() {
                                    if let Some(v) = o.vs.iter().find(|v| v.champ.id == e.id) {
                                        let vc = if v.weighted >= 0.05 {
                                            POS
                                        } else if v.weighted <= -0.05 {
                                            NEG
                                        } else {
                                            DIM
                                        };
                                        ui.add_sized(
                                            [28.0, 20.0],
                                            egui::Label::new(RichText::new(fmt_signed(v.weighted, 1)).size(9.5).color(vc)),
                                        )
                                        .on_hover_text(format!(
                                            "{} ({}): {} of this option's edge",
                                            v.champ.name,
                                            v.lane,
                                            fmt_signed(v.weighted, 1)
                                        ));
                                    } else {
                                        ui.add_sized([28.0, 20.0], egui::Label::new(RichText::new("·").size(9.5).color(DIM)));
                                    }
                                }
                            });
                        }
                    });
                }
            });
    }

    if !s.skill_order.is_empty() {
        ui.add_space(5.0);
        ui.label(RichText::new("SKILLS").size(9.0).strong().color(DIM));
        ui.horizontal(|ui| {
            for (i, ch) in s.skill_order.chars().take(15).enumerate() {
                let lvl = i as i32 + 1;
                let (bg, fg) = if ch == 'R' {
                    (GOLD, BG)
                } else if lvl < s.level {
                    (BG2, DIM)
                } else {
                    (RAISED, TEXT)
                };
                let (rect, _) = ui.allocate_exact_size(Vec2::splat(13.5), Sense::hover());
                ui.painter().rect_filled(rect, CornerRadius::same(2), bg);
                if lvl == s.level {
                    ui.painter().rect_stroke(rect, CornerRadius::same(2), Stroke::new(1.0, TEXT), egui::StrokeKind::Inside);
                }
                ui.painter().text(
                    rect.center(),
                    egui::Align2::CENTER_CENTER,
                    ch.to_string(),
                    egui::FontId::proportional(8.5),
                    fg,
                );
            }
        });
    }

    if !opp_cols.is_empty() {
        ui.add_space(4.0);
        ui.horizontal(|ui| {
            ui.label(RichText::new("Δ vs").size(9.0).color(DIM));
            for (e, lane) in &opp_cols {
                imgs.paint_icon(ui, &e.name, &e.icon, Vec2::splat(14.0), 3.0, Color32::WHITE)
                    .on_hover_text(format!("{} ({lane})", e.name));
            }
            ui.label(RichText::new("· from before the game").size(9.0).color(DIM));
        });
    }
}
