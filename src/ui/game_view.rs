use egui::{Align, Color32, CornerRadius, Layout, RichText, Sense, Stroke, Ui, Vec2};

use crate::engine::views::{GameScreen, IngameBuildView, PartView, PlayerView, TeamView};

use crate::images::ImageCache;
use crate::ui::build_view::render_damage_bar;
use crate::ui::theme::{ACCENT, ALLY, BG, BG2, CORAL, DIM, ENEMY, LINE2, MUTED, NEG, PANEL, POS, RAISED, TEXT, fmt_signed, strong, strong_font, tier_color};

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
            let width = ui.available_width();
            let body_h = (ui.available_height() - 21.0).max(1.0);
            ui.spacing_mut().item_spacing.x = 12.0;
            ui.columns(2, |cols| {
                let players_w = cols[0].available_width();
                cols[0].allocate_ui_with_layout(
                    Vec2::new(players_w, body_h),
                    Layout::top_down(Align::Min),
                    |ui| {
                        ui.set_min_size(Vec2::new(players_w, body_h));
                        if let Some(teams) = &g.teams {
                            render_roster(ui, teams, body_h, imgs);
                        } else {
                            ui.centered_and_justified(|ui| {
                                ui.label(RichText::new(&g.players_note).size(11.0).color(MUTED));
                            });
                        }
                    },
                );

                let build_w = cols[1].available_width();
                cols[1].allocate_ui_with_layout(
                    Vec2::new(build_w, body_h),
                    Layout::top_down(Align::Min),
                    |ui| {
                        egui::ScrollArea::vertical()
                            .id_salt("ig_build")
                            .auto_shrink([false, false])
                            .max_height(body_h)
                            .show(ui, |ui| {
                                ui.spacing_mut().item_spacing = Vec2::new(4.0, 2.0);
                                if let Some(b) = &g.build {
                                    render_ig_build(ui, b, imgs);
                                } else {
                                    ui.centered_and_justified(|ui| {
                                        ui.label(RichText::new(&g.build_note).size(11.0).color(MUTED));
                                    });
                                }
                            });
                    },
                );
            });

            let (footer, _) = ui.allocate_exact_size(Vec2::new(width, 16.0), Sense::hover());
            ui.painter().hline(footer.x_range(), footer.top(), Stroke::new(1.0, LINE2));
            ui.painter().text(
                footer.center() + Vec2::new(0.0, 1.0),
                egui::Align2::CENTER_CENTER,
                "Ctrl+X to hide · Hover for details",
                egui::FontId::proportional(9.5),
                MUTED,
            );
        });
}

fn render_roster(ui: &mut Ui, teams: &[TeamView], height: f32, imgs: &mut ImageCache) {
    const HEADER_H: f32 = 16.0;
    const ROW_GAP: f32 = 3.0;
    const TEAM_GAP: f32 = 5.0;
    const MIN_ROW_H: f32 = 43.0;
    let players = teams.iter().map(|team| team.players.len()).sum::<usize>() as f32;
    let empty_teams = teams.iter().filter(|team| team.players.is_empty()).count() as f32;
    let headers = teams.len() as f32;
    let fixed_h = (headers + empty_teams) * (HEADER_H + ROW_GAP)
        + players * ROW_GAP
        + (headers - 1.0).max(0.0) * TEAM_GAP;
    let row_h = ((height - fixed_h) / players.max(1.0)).floor().clamp(MIN_ROW_H, 48.0);

    let mut render = |ui: &mut Ui| {
        ui.spacing_mut().item_spacing.y = ROW_GAP;
        for (i, team) in teams.iter().enumerate() {
            if i > 0 {
                ui.add_space(TEAM_GAP);
            }
            let ally = team.side == "ally";
            let accent = if ally { ALLY } else { ENEMY };
            let (header, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), HEADER_H), Sense::hover());
            ui.painter().rect_filled(
                egui::Rect::from_center_size(
                    egui::pos2(header.left() + 1.0, header.center().y),
                    Vec2::new(2.0, 12.0),
                ),
                0,
                accent,
            );
            ui.painter().text(
                egui::pos2(header.left() + 8.0, header.center().y),
                egui::Align2::LEFT_CENTER,
                team.label.to_uppercase(),
                strong_font(11.0),
                accent,
            );
            if team.players.is_empty() {
                ui.label(RichText::new("No players").size(10.0).color(DIM));
            }
            for player in &team.players {
                render_player_row(ui, player, ally, row_h, imgs);
            }
        }
    };

    if fixed_h + players * MIN_ROW_H <= height {
        render(ui);
    } else {
        egui::ScrollArea::vertical()
            .id_salt("ig_players")
            .max_height(height)
            .show(ui, render);
    }
}

fn render_player_row(ui: &mut Ui, p: &PlayerView, ally: bool, height: f32, imgs: &mut ImageCache) {
    let (card, response) = ui.allocate_exact_size(Vec2::new(ui.available_width(), height), Sense::hover());
    let bg = if p.me {
        Color32::from_rgba_unmultiplied(ACCENT.r(), ACCENT.g(), ACCENT.b(), 28)
    } else if ally {
        PANEL
    } else {
        Color32::from_rgb(0x2B, 0x20, 0x2F)
    };
    let accent = if p.me { ACCENT } else if ally { ALLY } else { ENEMY };
    ui.painter().rect_filled(card, CornerRadius::same(4), bg);
    ui.painter().rect_filled(
        egui::Rect::from_min_size(card.left_top() + Vec2::new(0.0, 3.0), Vec2::new(2.0, height - 6.0)),
        0,
        accent,
    );

    let portrait = egui::Rect::from_center_size(
        egui::pos2(card.left() + 23.0, card.center().y),
        Vec2::splat(32.0),
    );
    let mut icon_ui = ui.new_child(egui::UiBuilder::new().max_rect(portrait));
    let (champ, icon) = p.champ.as_ref().map_or(("?", ""), |c| (c.name.as_str(), c.icon.as_str()));
    imgs.paint_icon(&mut icon_ui, champ, icon, Vec2::splat(32.0), 4.0, Color32::WHITE);

    let left = portrait.right() + 7.0;
    let right = card.right() - 6.0;
    let top = card.center().y - 20.0;
    let name_y = top + 7.0;
    let stats_y = top + 21.0;
    let tags_y = top + 34.0;
    let painter = ui.painter().with_clip_rect(card);
    let line = |text: String, size, color, width: f32, bold| {
        let font = if bold { strong_font(size) } else { egui::FontId::proportional(size) };
        let mut job = egui::text::LayoutJob::simple_singleline(text, font, color);
        job.wrap.max_rows = 1;
        job.wrap.max_width = width.max(1.0);
        job.wrap.break_anywhere = true;
        painter.layout_job(job)
    };
    let season = if let Some(wr) = p.wr {
        let text = if p.season_games > 0 { format!("{wr}% ({})", p.season_games) } else { format!("{wr}%") };
        Some((text, win_rate_color(wr)))
    } else if p.wins > 0 {
        Some((format!("{}W", p.wins), MUTED))
    } else {
        None
    };
    let mut name_right = right;
    if let Some((text, color)) = &season {
        let galley = line(text.clone(), 10.0, *color, (right - left) * 0.32, false);
        let size = galley.size();
        painter.galley(egui::pos2(right - size.x, name_y - size.y * 0.5), galley, *color);
        name_right -= size.x + 5.0;
    }
    let rank = line(p.rank.clone(), 10.0, tier_color(&p.tier), (name_right - left) * 0.5, true);
    let rank_size = rank.size();
    let gap = if p.rank.is_empty() { 0.0 } else { 5.0 };
    let name = line(p.name.clone(), 12.0, TEXT, name_right - left - rank_size.x - gap, true);
    let name_size = name.size();
    painter.galley(egui::pos2(left, name_y - name_size.y * 0.5), name, TEXT);
    painter.galley(
        egui::pos2(left + name_size.x + gap, name_y - rank_size.y * 0.5 + 0.5),
        rank,
        tier_color(&p.tier),
    );

    let recent = if p.champ_games > 0 {
        let wr = p.champ_wr.map(|w| format!(" · {w}%")).unwrap_or_default();
        let kda = p.kda.map(|k| format!(" · {k:.1} KDA")).unwrap_or_default();
        format!("{} games{wr}{kda}", p.champ_games)
    } else if p.games > 0 {
        format!("Not in last {} games", p.games)
    } else {
        "No recent games".into()
    };
    let dot_count = p.recent.chars().count();
    let dot_step = 6.0_f32.min((right - left).max(1.0) * 0.35 / dot_count.max(1) as f32);
    let stats_w = (right - left) - dot_count as f32 * dot_step - 4.0;
    let stats = line(recent.clone(), 10.0, MUTED, stats_w, false);
    painter.galley(egui::pos2(left, stats_y - stats.size().y * 0.5), stats, MUTED);
    for (i, ch) in p.recent.chars().rev().enumerate() {
        painter.circle_filled(
            egui::pos2(right - (i as f32 + 0.5) * dot_step, stats_y),
            (dot_step * 0.36).min(2.2),
            if ch == 'W' { POS } else { NEG },
        );
    }

    let mut tag_x = left;
    for tag in &p.tags {
        if right - tag_x < 18.0 {
            break;
        }
        let bad = tag.starts_with("First time") || tag.contains("losses in a row");
        let (fill, color) = if bad {
            (Color32::from_rgba_unmultiplied(0xFF, 0x5F, 0x5F, 45), NEG)
        } else {
            (Color32::from_rgba_unmultiplied(0x5F, 0xFF, 0x87, 40), POS)
        };
        let galley = line(tag.clone(), 9.0, color, right - tag_x - 8.0, true);
        let size = galley.size();
        let badge = egui::Rect::from_min_size(egui::pos2(tag_x, tags_y - 6.0), Vec2::new(size.x + 8.0, 12.0));
        painter.rect_filled(badge, 4, fill);
        painter.galley(egui::pos2(tag_x + 4.0, tags_y - size.y * 0.5), galley, color);
        tag_x = badge.right() + 4.0;
    }

    let mut tip = p.name.clone();
    if !p.tag.is_empty() {
        tip.push_str(&format!("#{}", p.tag));
    }
    if !p.lane.is_empty() {
        tip.push_str(&format!(" ({})", p.lane));
    }
    if !p.rank_full.is_empty() {
        let queue = if p.queue == "flex" { " (Flex)" } else { "" };
        tip.push_str(&format!("\n{}{queue}", p.rank_full));
    }
    if let Some((text, _)) = season {
        tip.push_str(&format!("\nRanked season: {text}"));
    }
    if let Some(games) = p.champ_season_games {
        tip.push_str(&format!("\nChampion season: {games} games"));
        if games > 0 && let Some(wr) = p.champ_season_wr {
            tip.push_str(&format!(" · {wr}% WR"));
        }
    }
    tip.push_str(&format!("\nRecent champion games: {recent}"));
    if !p.recent.is_empty() {
        tip.push_str(&format!("\nRecent results: {}", p.recent));
    }
    if !p.tags.is_empty() {
        tip.push_str(&format!("\n{}", p.tags.join(" · ")));
    }
    if let (Some(k), Some(d), Some(a)) = (p.kills, p.deaths, p.assists) {
        tip.push_str(&format!("\nAvg K/D/A: {k:.1} / {d:.1} / {a:.1}"));
    }
    if let Some(m) = p.mastery {
        tip.push_str(&format!("\nMastery: {m} pts"));
    }
    if p.error {
        tip.push_str("\nSome data could not be loaded");
    }
    response.on_hover_text(tip);
}

fn win_rate_color(wr: i32) -> Color32 {
    if wr >= 55 { POS } else if wr <= 45 { NEG } else { MUTED }
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
            ui.label(strong(format!("{cname} · Lvl {}", s.level)).size(15.0).color(TEXT));
            ui.horizontal(|ui| {
                ui.label(RichText::new(fmt_clock(s.time_secs)).size(10.0).color(MUTED));
                ui.label(strong(format!("{} gold", fmt_k(s.gold))).size(11.5).color(ACCENT))
                    .on_hover_text(format!("{} gold available", s.gold));
            });
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
        ui.add_space(3.0);
        ui.horizontal(|ui| {
            ui.label(strong("CORE").size(10.0).color(MUTED));
            for (i, p) in s.core.iter().enumerate() {
                if i > 0 {
                    ui.label(RichText::new("›").size(10.0).color(DIM));
                }
                paint_item_icon(ui, p, 24.0, imgs);
            }
        });
    }
    if let Some(next) = &s.next {
        let need = (next.gold - s.gold).max(0);
        ui.add_space(3.0);
        egui::Frame::new()
            .fill(Color32::from_rgba_unmultiplied(CORAL.r(), CORAL.g(), CORAL.b(), 17))
            .stroke(Stroke::new(1.0, CORAL.gamma_multiply(0.5)))
            .corner_radius(CornerRadius::same(5))
            .inner_margin(egui::Margin::symmetric(6, 4))
            .show(ui, |ui| {
                ui.label(strong("NEXT PURCHASE").size(9.0).color(CORAL));
                ui.allocate_ui_with_layout(
                    Vec2::new(ui.available_width(), 17.0),
                    Layout::right_to_left(Align::Center),
                    |ui| {
                        let (cost, color) = if need > 0 {
                            (format!("{} gold more", fmt_k(need)), CORAL)
                        } else {
                            ("Affordable".into(), POS)
                        };
                        ui.label(strong(cost).size(10.5).color(color));
                        ui.with_layout(Layout::left_to_right(Align::Center), |ui| {
                            ui.add(egui::Label::new(strong(&next.name).size(12.0).color(TEXT)).truncate())
                                .on_hover_text(format!("{} · {} gold", next.name, next.gold));
                        });
                    },
                );
            });
    }

    let opp_cols: Vec<_> = s.opponents.iter().take(4).collect();
    for sec in &s.sections {
        ui.add_space(4.0);
        let is_next = sec.state == "next";
        let bg = if is_next {
            Color32::from_rgba_unmultiplied(CORAL.r(), CORAL.g(), CORAL.b(), 18)
        } else {
            Color32::TRANSPARENT
        };
        let stroke = if is_next {
            Stroke::new(1.0, Color32::from_rgba_unmultiplied(CORAL.r(), CORAL.g(), CORAL.b(), 140))
        } else {
            Stroke::NONE
        };
        egui::Frame::new()
            .fill(bg)
            .stroke(stroke)
            .corner_radius(CornerRadius::same(4))
            .inner_margin(egui::Margin::symmetric(4, 2))
            .show(ui, |ui| {
                let col_w = (ui.available_width() * 0.105).clamp(18.0, 28.0);
                let total_w = col_w + 3.0;
                let title = if is_next {
                    format!("{} · NEXT", sec.title.to_uppercase())
                } else {
                    sec.title.to_uppercase()
                };
                ui.horizontal(|ui| {
                    ui.label(
                        strong(title)
                            .size(10.0)
                            .color(if is_next { CORAL } else { MUTED }),
                    );
                    if !opp_cols.is_empty() && sec.options.iter().any(|o| !o.vs.is_empty()) {
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            ui.allocate_space(Vec2::new(total_w, 16.0));
                            for (e, lane) in opp_cols.iter().rev() {
                                let (r, resp) = ui.allocate_exact_size(Vec2::new(col_w, 16.0), Sense::hover());
                                let ico_r = egui::Rect::from_center_size(r.center(), Vec2::splat(16.0));
                                let mut sub = ui.new_child(
                                    egui::UiBuilder::new().max_rect(ico_r).layout(Layout::top_down(Align::Min)),
                                );
                                imgs.paint_icon(&mut sub, &e.name, &e.icon, Vec2::splat(16.0), 4.0, Color32::WHITE);
                                resp.on_hover_text(format!("{} ({lane})", e.name));
                            }
                        });
                    }
                });
                for o in &sec.options {
                    ui.horizontal(|ui| {
                        for p in &o.parts {
                            paint_item_icon(ui, p, 22.0, imgs);
                        }
                        ui.label(strong(format!("{:.1}%", o.wr)).size(10.5).color(TEXT));
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
                                ui.add_sized([total_w, 20.0], egui::Label::new(strong(txt).size(10.5).color(col)).halign(Align::Center));
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
                                            [col_w, 20.0],
                                            egui::Label::new(RichText::new(fmt_signed(v.weighted, 1)).size(9.5).color(vc)).halign(Align::Center),
                                        )
                                        .on_hover_text(format!(
                                            "{} ({}): {} of this option's edge",
                                            v.champ.name,
                                            v.lane,
                                            fmt_signed(v.weighted, 1)
                                        ));
                                    } else {
                                        ui.add_sized([col_w, 20.0], egui::Label::new(RichText::new("·").size(9.5).color(DIM)).halign(Align::Center));
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
        ui.label(strong("SKILLS").size(10.0).color(MUTED));
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 2.0;
            let count = s.skill_order.chars().take(15).count();
            let tile_w = ((ui.available_width() + 2.0) / count as f32 - 2.0).clamp(8.0, 18.0);
            for (i, ch) in s.skill_order.chars().take(15).enumerate() {
                let lvl = i as i32 + 1;
                let (bg, fg) = if ch == 'R' {
                    (ACCENT, BG)
                } else if lvl < s.level {
                    (BG2, DIM)
                } else {
                    (RAISED, TEXT)
                };
                let (rect, _) = ui.allocate_exact_size(Vec2::new(tile_w, 16.0), Sense::hover());
                ui.painter().rect_filled(rect, CornerRadius::same(2), bg);
                if lvl == s.level {
                    ui.painter().rect_stroke(rect, CornerRadius::same(2), Stroke::new(1.0, TEXT), egui::StrokeKind::Inside);
                }
                ui.painter().text(
                    rect.center(),
                    egui::Align2::CENTER_CENTER,
                    ch,
                    strong_font(9.0),
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
