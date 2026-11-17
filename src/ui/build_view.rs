use egui::{Align, Color32, CornerRadius, Layout, RichText, Sense, Stroke, Ui, Vec2};

use crate::engine::views::{BuildSectionView, BuildView, DamageSplitView, OptionView, RuneNodeView, RunePageView};

use crate::images::ImageCache;
use crate::ui::theme::{AD, AP, BG, BG2, DIM, GOLD, LINE, LINE2, MUTED, NEG, PANEL, POS, RAISED, TEXT, TEXT2, fmt_signed, left_label, strong};

#[derive(Default)]
pub struct BuildUiState {
    pub page_idx: usize,
    pub last_champ: i32,
}

fn fmt_games_short(g: u32) -> String {
    if g >= 1000 {
        format!("{:.0}k", g as f64 / 1000.0)
    } else {
        g.to_string()
    }
}

fn fmt_delta(v: f64) -> (String, Color32) {
    if v.abs() < 0.3 {
        ("±0".into(), DIM)
    } else if v > 0.0 {
        (fmt_signed(v, 1), POS)
    } else {
        (fmt_signed(v, 1), NEG)
    }
}

pub fn render_build(ui: &mut Ui, d: &BuildView, st: &mut BuildUiState, imgs: &mut ImageCache) {
    if st.last_champ != d.champ.id {
        st.last_champ = d.champ.id;
        st.page_idx = 0;
    }

    if let Some(empty) = &d.empty {
        ui.centered_and_justified(|ui| {
            ui.label(RichText::new(empty).color(MUTED));
        });
        return;
    }

    let split = ui.available_width() >= 760.0;
    let left_w = if split {
        (ui.available_width() * 0.26).clamp(286.0, 326.0)
    } else {
        ui.available_width()
    };
    let gap = 12.0;

    ui.with_layout(Layout::bottom_up(Align::Min), |ui| {
        if !d.skill_order.is_empty() {
            render_build_footer(ui, &d.skill_order, if split { left_w + gap } else { 0.0 });
        }

        ui.with_layout(Layout::top_down(Align::Min), |ui| {
            egui::ScrollArea::vertical()
                .id_salt("build_scroll")
                .show(ui, |ui| {
                    ui.set_width((ui.available_width() - 8.0).max(1.0));
                    render_build_header(ui, d, left_w, imgs);
                    ui.add_space(8.0);

                    let mut side = |ui: &mut Ui, imgs: &mut ImageCache| {
                        ui.set_width(left_w.min(ui.available_width()));
                        render_rune_card(ui, d, st, imgs);
                        if !d.spells.is_empty() {
                            ui.add_space(5.0);
                            render_spells_card(ui, &d.spells, ui.available_width(), imgs);
                        }
                    };
                    if split {
                        ui.horizontal_top(|ui| {
                            ui.spacing_mut().item_spacing.x = gap;
                            ui.vertical(|ui| side(ui, imgs));
                            ui.vertical(|ui| render_build_items(ui, d, imgs));
                        });
                    } else {
                        side(ui, imgs);
                        ui.add_space(8.0);
                        render_build_items(ui, d, imgs);
                    }
                });
        });
    });
}

fn render_build_header(ui: &mut Ui, d: &BuildView, left_w: f32, imgs: &mut ImageCache) {
    let start = d.items.iter().find(|s| s.key == "start").and_then(|s| s.options.first());
    let core = d.items.iter().find(|s| s.key == "core").and_then(|s| s.options.first());
    let max_order = d.skills.first().map(|sk| {
        sk.key
            .chars()
            .filter(|c| matches!(c, 'Q' | 'W' | 'E' | 'R'))
            .map(|c| c.to_string())
            .collect::<Vec<_>>()
            .join(" › ")
    });
    let width = ui.available_width();
    let gap = 12.0;
    let wide = width >= 1000.0;
    let render_champ = |ui: &mut Ui, imgs: &mut ImageCache| {
        ui.horizontal(|ui| {
            imgs.paint_icon(ui, &d.champ.name, &d.champ.icon, Vec2::splat(52.0), 8.0, Color32::WHITE);
            ui.vertical(|ui| {
                ui.spacing_mut().item_spacing.y = 1.0;
                ui.label(strong(&d.champ.name).size(21.0).color(TEXT));
                ui.label(
                    RichText::new(format!("{:.1}% win · {} games", d.wr, fmt_games_short(d.games)))
                        .size(11.0)
                        .color(MUTED),
                );
                if !d.opponents.is_empty() {
                    ui.horizontal_wrapped(|ui| {
                        ui.spacing_mut().item_spacing.x = 4.0;
                        ui.label(RichText::new("Δ measured vs").size(10.0).color(MUTED));
                        for (e, lane, games, wr) in &d.opponents {
                            imgs.paint_icon(ui, &e.name, &e.icon, Vec2::splat(16.0), 3.0, Color32::WHITE)
                                .on_hover_text(format!("vs {} ({lane}): {wr:.1}% WR · {games} games", e.name));
                        }
                    });
                }
            });
        });
    };
    let render_cards = |ui: &mut Ui, imgs: &mut ImageCache| {
        let remaining = ui.available_width();
        let start_w = if wide && core.is_some() {
            (remaining * 0.39).min(336.0)
        } else {
            remaining
        };
        if let Some(opt) = start {
            render_build_summary(ui, start_w, opt, None, imgs);
        }
        if let Some(opt) = core {
            let core_w = if wide && start.is_some() { remaining - start_w - gap } else { remaining };
            render_build_summary(ui, core_w, opt, Some(max_order.as_deref().unwrap_or_default()), imgs);
        }
    };
    if wide {
        ui.horizontal_top(|ui| {
            ui.spacing_mut().item_spacing.x = gap;
            ui.vertical(|ui| {
                ui.set_width(left_w);
                render_champ(ui, imgs);
            });
            render_cards(ui, imgs);
        });
    } else {
        render_champ(ui, imgs);
        render_cards(ui, imgs);
    }
}

fn render_build_summary(
    ui: &mut Ui,
    width: f32,
    opt: &OptionView,
    max_order: Option<&str>,
    imgs: &mut ImageCache,
) {
    let is_core = max_order.is_some();
    egui::Frame::new()
        .fill(PANEL)
        .stroke(Stroke::new(1.0, LINE))
        .corner_radius(CornerRadius::same(6))
        .inner_margin(egui::Margin::symmetric(9, 7))
        .show(ui, |ui| {
            ui.set_width((width - 20.0).max(1.0));
            ui.horizontal_wrapped(|ui| {
                ui.spacing_mut().item_spacing.x = 5.0;
                ui.label(strong(if is_core { "MOST BUILT CORE" } else { "START" }).size(10.0).color(MUTED));
                for (i, p) in opt.parts.iter().enumerate() {
                    if is_core && i > 0 {
                        ui.label(RichText::new("›").size(11.0).color(DIM));
                    }
                    imgs.paint_icon(ui, &p.name, &p.icon, Vec2::splat(24.0), 4.0, Color32::WHITE)
                        .on_hover_text(if p.plaintext.is_empty() {
                            p.name.clone()
                        } else {
                            format!("{}\n{}", p.name, p.plaintext)
                        });
                }
                ui.label(strong(format!("{:.1}%", opt.wr)).size(11.5).color(TEXT));
                let picked = if is_core {
                    format!("win · {:.1}% picked · {} games", opt.pr, fmt_games_short(opt.games))
                } else {
                    format!("win · {:.1}% picked", opt.pr)
                };
                ui.label(RichText::new(picked).size(10.0).color(MUTED));
                if let Some(order) = max_order.filter(|order| !order.is_empty()) {
                    ui.label(strong(format!("Max {order}")).size(11.0).color(TEXT));
                } else if !is_core && opt.has_vs {
                    let (txt, col) = fmt_delta(opt.matchup);
                    ui.label(strong(txt).size(11.0).color(col));
                }
            });
        });
}

fn render_build_items(ui: &mut Ui, d: &BuildView, imgs: &mut ImageCache) {
    let sections: Vec<_> = d.items.iter().filter(|s| s.key != "start" && s.key != "core").collect();
    let width = ui.available_width();
    let gap = 12.0;
    let cols = (((width + gap) / (212.0 + gap)) as usize).clamp(1, 3);
    let col_w = (width - gap * (cols - 1) as f32) / cols as f32;
    for row in sections.chunks(cols) {
        ui.horizontal_top(|ui| {
            ui.spacing_mut().item_spacing.x = gap;
            for section in row {
                ui.vertical(|ui| {
                    ui.set_width(col_w);
                    render_item_section(ui, section, col_w, imgs);
                });
            }
        });
        ui.add_space(5.0);
    }
}

fn render_build_footer(ui: &mut Ui, order: &str, offset: f32) {
    let footer_w = (ui.available_width() - 8.0).max(1.0);
    let width = footer_w - offset;
    let inline = width >= 660.0;
    let height = if inline { 36.0 } else if width >= 430.0 { 55.0 } else { 70.0 };
    let render_note = |ui: &mut Ui| {
        ui.add(egui::Label::new(
            RichText::new("Δ shows how many win-rate points an option gains or loses against these enemies versus its usual rate. Options with too few games show no Δ.")
                .size(10.0)
                .color(MUTED),
        ).wrap());
    };
    ui.allocate_ui_with_layout(Vec2::new(footer_w, height), Layout::top_down(Align::Min), |ui| {
        ui.set_min_height(height);
        ui.add_space(3.0);
        ui.horizontal(|ui| {
            if offset > 0.0 {
                ui.add_space(offset);
            }
            ui.vertical(|ui| {
                ui.horizontal(|ui| {
                    ui.label(strong("SKILLS").size(10.0).color(MUTED));
                    let count = order.chars().take(15).count();
                    ui.spacing_mut().item_spacing.x = 4.0;
                    let size = if inline {
                        21.0
                    } else {
                        ((ui.available_width() + 4.0) / count as f32 - 4.0).clamp(10.0, 21.0)
                    };
                    for (i, ch) in order.chars().take(15).enumerate() {
                        let (bg, fg) = if ch == 'R' { (GOLD, BG) } else { (RAISED, TEXT2) };
                        let (cell, _) = ui.allocate_exact_size(Vec2::new(size, 30.0), Sense::hover());
                        let tile = egui::Rect::from_min_size(cell.min, Vec2::new(size, 20.0));
                        let p = ui.painter();
                        p.rect_filled(tile, CornerRadius::same(3), bg);
                        p.text(tile.center(), egui::Align2::CENTER_CENTER, ch, egui::FontId::proportional(11.0), fg);
                        p.text(
                            egui::pos2(cell.center().x, cell.bottom() - 4.5),
                            egui::Align2::CENTER_CENTER,
                            i + 1,
                            egui::FontId::proportional(8.5),
                            DIM,
                        );
                    }
                    if inline {
                        ui.add_space(8.0);
                        render_note(ui);
                    }
                });
                if !inline {
                    render_note(ui);
                }
            });
        });
    });
}

pub fn render_damage_bar(ui: &mut Ui, d: &DamageSplitView) {
    ui.vertical(|ui| {
        ui.horizontal(|ui| {
            ui.label(RichText::new("Their damage").size(10.0).color(MUTED));
            ui.label(RichText::new(format!("{}% AD", d.physical)).size(10.0).strong().color(AD));
            ui.label(RichText::new(format!("{}% AP", d.magic)).size(10.0).strong().color(AP));
            if d.true_dmg >= 3 {
                ui.label(RichText::new(format!("{}% true", d.true_dmg)).size(10.0).strong().color(TEXT));
            }
        });
        let (rect, resp) = ui.allocate_exact_size(Vec2::new(ui.available_width().min(280.0), 5.0), Sense::hover());
        let p = ui.painter();
        p.rect_filled(rect, CornerRadius::same(2), LINE);
        let w = rect.width();
        let sum = (d.physical + d.magic + d.true_dmg).max(1) as f32;
        let wad = w * (d.physical as f32 / sum);
        let wap = w * (d.magic as f32 / sum);
        p.rect_filled(egui::Rect::from_min_size(rect.min, Vec2::new(wad, rect.height())), CornerRadius::ZERO, AD);
        p.rect_filled(egui::Rect::from_min_size(egui::pos2(rect.left() + wad, rect.top()), Vec2::new(wap, rect.height())), CornerRadius::ZERO, AP);
        p.rect_filled(egui::Rect::from_min_size(egui::pos2(rect.left() + wad + wap, rect.top()), Vec2::new((w - wad - wap).max(0.0), rect.height())), CornerRadius::ZERO, Color32::WHITE);

        if !d.enemies.is_empty() {
            let tip = d
                .enemies
                .iter()
                .map(|e| format!("{}: {}% AD, {}% AP, {}% true", e.name, e.physical, e.magic, e.true_dmg))
                .collect::<Vec<_>>()
                .join("\n");
            resp.on_hover_text(tip);
        }
    });
}

fn render_rune_card(ui: &mut Ui, d: &BuildView, st: &mut BuildUiState, imgs: &mut ImageCache) {
    if d.pages.is_empty() {
        return;
    }
    if st.page_idx >= d.pages.len() {
        st.page_idx = 0;
    }
    let page: &RunePageView = &d.pages[st.page_idx];

    egui::Frame::new()
        .fill(PANEL)
        .stroke(Stroke::new(1.0, LINE))
        .corner_radius(CornerRadius::same(8))
        .inner_margin(egui::Margin::same(10))
        .show(ui, |ui| {
            // Segmented Most picked | Highest win rate + wr/games
            ui.horizontal(|ui| {
                if d.pages.len() > 1 {
                    egui::Frame::new()
                        .fill(BG2)
                        .stroke(Stroke::new(1.0, LINE2))
                        .corner_radius(CornerRadius::same(12))
                        .inner_margin(egui::Margin::symmetric(2, 2))
                        .show(ui, |ui| {
                            ui.horizontal(|ui| {
                                for (i, pg) in d.pages.iter().enumerate() {
                                    let on = i == st.page_idx;
                                    let btn = egui::Button::new(
                                        RichText::new(&pg.label)
                                            .size(10.0)
                                            .strong()
                                            .color(if on { BG } else { MUTED }),
                                    )
                                    .fill(if on { GOLD } else { Color32::TRANSPARENT })
                                    .stroke(Stroke::NONE)
                                    .corner_radius(CornerRadius::same(10));
                                    if ui.add(btn).on_hover_cursor(egui::CursorIcon::PointingHand).clicked() {
                                        st.page_idx = i;
                                    }
                                }
                            });
                        });
                } else {
                    ui.label(RichText::new(&page.label).size(11.0).strong().color(GOLD));
                }
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    ui.label(
                        RichText::new(format!("{:.1}% · {}", page.wr, fmt_games_short(page.games)))
                            .size(10.0)
                            .color(MUTED),
                    );
                });
            });
            ui.add_space(5.0);

            ui.columns(2, |cols| {
                // Primary tree
                if let Some(s) = page.styles.first() {
                    cols[0].horizontal(|ui| {
                        imgs.paint_icon(ui, &s.name, &s.icon, Vec2::splat(18.0), 9.0, Color32::WHITE);
                        ui.label(RichText::new(&s.name).size(11.5).strong().color(POS));
                    });
                    cols[0].add_space(4.0);
                }
                for (ri, row) in page.tree_primary.iter().enumerate() {
                    rune_tree_row(&mut cols[0], row, ri == 0, imgs);
                    cols[0].add_space(2.0);
                }

                // Secondary tree + shards
                if let Some(s) = page.styles.get(1) {
                    cols[1].horizontal(|ui| {
                        imgs.paint_icon(ui, &s.name, &s.icon, Vec2::splat(18.0), 9.0, Color32::WHITE);
                        ui.label(RichText::new(&s.name).size(11.5).strong().color(AP));
                    });
                    cols[1].add_space(4.0);
                }
                for row in &page.tree_secondary {
                    rune_tree_row(&mut cols[1], row, false, imgs);
                    cols[1].add_space(2.0);
                }
                cols[1].add_space(4.0);
                for row in &page.tree_shards {
                    rune_shard_row(&mut cols[1], row, imgs);
                    cols[1].add_space(2.0);
                }
            });
        });
}

fn rune_tree_row(ui: &mut Ui, row: &[RuneNodeView], is_keystone: bool, imgs: &mut ImageCache) {
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 4.0;
        let max_size = if is_keystone { 34.0 } else { 24.0 };
        let sz = ((ui.available_width() - 31.0) / row.len().max(1) as f32 - 4.0).clamp(14.0, max_size);
        let mut chosen_node = None;
        for r in row {
            let tint = if r.chosen {
                Color32::WHITE
            } else {
                Color32::from_rgba_unmultiplied(255, 255, 255, 70)
            };
            if r.chosen {
                chosen_node = Some(r);
            }
            ui.vertical(|ui| {
                let (rect, resp) = ui.allocate_exact_size(Vec2::splat(sz), Sense::hover());
                let mut sub = ui.new_child(egui::UiBuilder::new().max_rect(rect));
                imgs.paint_icon(&mut sub, &r.name, &r.icon, Vec2::splat(sz), sz * 0.5, tint);
                if r.chosen {
                    ui.painter().circle_stroke(rect.center(), sz * 0.52, Stroke::new(1.5, POS));
                }
                let mut tip = r.name.clone();
                if let (Some(wr), Some(pr), Some(g)) = (r.wr, r.pr, r.games) {
                    tip.push_str(&format!("\n{wr:.1}% WR · {pr:.1}% picked · {g} games"));
                }
                if r.has_vs {
                    tip.push_str(&format!("\nΔ vs lane: {}", fmt_signed(r.matchup, 1)));
                }
                resp.on_hover_text(tip);

                if is_keystone && let Some(pr) = r.pr {
                    ui.label(
                        RichText::new(format!("{pr:.1}%"))
                            .size(8.5)
                            .color(if r.chosen { TEXT2 } else { DIM }),
                    );
                }
            });
        }
        if let Some(p) = chosen_node {
            let (txt, col) = if p.has_vs {
                fmt_delta(p.matchup)
            } else {
                ("±0".into(), DIM)
            };
            ui.label(RichText::new(txt).size(10.0).strong().color(col));
        }
    });
}

fn rune_shard_row(ui: &mut Ui, row: &[RuneNodeView], imgs: &mut ImageCache) {
    ui.horizontal(|ui| {
        let mut chosen_node = None;
        for r in row {
            let tint = if r.chosen {
                Color32::WHITE
            } else {
                Color32::from_rgba_unmultiplied(255, 255, 255, 70)
            };
            if r.chosen {
                chosen_node = Some(r);
            }
            let (rect, resp) = ui.allocate_exact_size(Vec2::splat(18.0), Sense::hover());
            let mut sub = ui.new_child(egui::UiBuilder::new().max_rect(rect));
            imgs.paint_icon(&mut sub, &r.name, &r.icon, Vec2::splat(18.0), 9.0, tint);
            if r.chosen {
                ui.painter().circle_stroke(rect.center(), 9.5, Stroke::new(1.2, POS));
            }
            let mut tip = r.name.clone();
            if let (Some(wr), Some(pr), Some(g)) = (r.wr, r.pr, r.games) {
                tip.push_str(&format!("\n{wr:.1}% WR · {pr:.1}% picked · {g} games"));
            }
            resp.on_hover_text(tip);
        }
        if let Some(p) = chosen_node {
            let (txt, col) = if p.has_vs {
                fmt_delta(p.matchup)
            } else {
                ("±0".into(), DIM)
            };
            ui.label(RichText::new(txt).size(9.5).strong().color(col));
        }
    });
}

fn render_spells_card(ui: &mut Ui, spells: &[OptionView], width: f32, imgs: &mut ImageCache) {
    egui::Frame::new()
        .fill(PANEL)
        .stroke(Stroke::new(1.0, LINE))
        .corner_radius(CornerRadius::same(8))
        .inner_margin(egui::Margin::same(10))
        .show(ui, |ui| {
            ui.set_width((width - 22.0).max(1.0));
            ui.label(RichText::new("SUMMONER SPELLS").size(9.5).strong().color(MUTED));
            ui.add_space(6.0);
            for o in spells.iter().take(3) {
                let bg = if o.best {
                    Color32::from_rgba_unmultiplied(0x3D, 0xDC, 0x97, 16)
                } else {
                    Color32::TRANSPARENT
                };
                let stroke = if o.best { Stroke::new(1.0, POS) } else { Stroke::NONE };
                egui::Frame::new()
                    .fill(bg)
                    .stroke(stroke)
                    .corner_radius(CornerRadius::same(4))
                    .inner_margin(egui::Margin::symmetric(4, 3))
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            ui.spacing_mut().item_spacing.x = 5.0;
                            for p in &o.parts {
                                imgs.paint_icon(ui, &p.name, &p.icon, Vec2::splat(22.0), 4.0, Color32::WHITE)
                                    .on_hover_text(&p.name);
                            }
                            let names = o.parts.iter().map(|p| p.name.as_str()).collect::<Vec<_>>().join(" + ");
                            let name_w = (ui.available_width() - 98.0).max(1.0);
                            left_label(ui, name_w, 22.0, strong(names).size(10.5).color(TEXT));
                            ui.label(RichText::new(format!("{:.1}%", o.wr)).size(10.5).strong().color(TEXT2));
                            ui.label(RichText::new(format!("{:.1}%", o.pr)).size(10.0).color(MUTED));
                            if o.has_vs {
                                let (txt, col) = fmt_delta(o.matchup);
                                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                    ui.label(RichText::new(txt).size(10.5).strong().color(col));
                                });
                            }
                        });
                    });
                ui.add_space(2.0);
            }
        });
}

fn render_item_section(ui: &mut Ui, sec: &BuildSectionView, col_w: f32, imgs: &mut ImageCache) {
    let shown: Vec<&OptionView> = sec.options.iter().take(3).collect();
    let extra = sec.options.len().saturating_sub(3);
    let max_pr = shown.iter().map(|o| o.pr).fold(1.0_f64, f64::max);

    ui.horizontal(|ui| {
        ui.label(strong(sec.title.to_uppercase()).size(10.5).color(MUTED));
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            ui.label(RichText::new("Δ VS LANE").size(8.5).color(DIM));
            if extra > 0 {
                let extra_tip = sec
                    .options
                    .iter()
                    .skip(3)
                    .map(|o| {
                        let nm = o.parts.iter().map(|p| p.name.as_str()).collect::<Vec<_>>().join(" + ");
                        format!("{nm}: {:.1}% WR · {:.1}% picked", o.wr, o.pr)
                    })
                    .collect::<Vec<_>>()
                    .join("\n");
                ui.label(RichText::new(format!("+{extra} more")).size(9.0).color(MUTED))
                    .on_hover_text(extra_tip);
            }
        });
    });
    ui.add_space(1.0);

    for o in shown {
        let stroke = if o.best {
            Stroke::new(1.2, POS)
        } else {
            Stroke::new(1.0, LINE)
        };
        let fill = if o.best {
            Color32::from_rgba_unmultiplied(0x3D, 0xDC, 0x97, 14)
        } else {
            PANEL
        };
        egui::Frame::new()
            .fill(fill)
            .stroke(stroke)
            .corner_radius(CornerRadius::same(6))
            .inner_margin(egui::Margin::symmetric(8, 7))
            .show(ui, |ui| {
                ui.set_width((col_w - 16.0 - stroke.width * 2.0).max(1.0));
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 6.0;
                    for p in &o.parts {
                        imgs.paint_icon(ui, &p.name, &p.icon, Vec2::splat(32.0), 5.0, Color32::WHITE)
                            .on_hover_text(if p.plaintext.is_empty() {
                                p.name.clone()
                            } else {
                                format!("{}\n{}", p.name, p.plaintext)
                            });
                    }
                    let stats_w = if o.best { 62.0 } else if o.has_vs { 34.0 } else { 0.0 };
                    let name_w = (ui.available_width() - stats_w - 6.0).max(1.0);
                    ui.vertical(|ui| {
                        ui.set_width(name_w);
                        ui.spacing_mut().item_spacing.y = 1.0;
                        let name = o.parts.iter().map(|p| p.name.as_str()).collect::<Vec<_>>().join(" + ");
                        left_label(ui, name_w, 17.0, strong(name).size(12.0).color(TEXT));
                        ui.horizontal(|ui| {
                            ui.label(strong(format!("{:.1}%", o.wr)).size(10.0).color(TEXT2));
                            if name_w >= 104.0 {
                                let (bar, _) = ui.allocate_exact_size(Vec2::new(30.0, 3.0), Sense::hover());
                                ui.painter().rect_filled(bar, CornerRadius::same(2), LINE);
                                let mut fill_r = bar;
                                fill_r.set_width(bar.width() * (o.pr / max_pr).clamp(0.0, 1.0) as f32);
                                ui.painter().rect_filled(fill_r, CornerRadius::same(2), MUTED);
                            }
                            ui.label(RichText::new(format!("{:.1}%", o.pr)).size(9.5).color(MUTED));
                        });
                    });
                    if stats_w > 0.0 {
                        ui.with_layout(Layout::top_down(Align::Max), |ui| {
                            ui.set_width(stats_w);
                            ui.spacing_mut().item_spacing.y = 1.0;
                            if o.best {
                                egui::Frame::new()
                                    .fill(POS)
                                    .corner_radius(CornerRadius::same(4))
                                    .inner_margin(egui::Margin::symmetric(4, 1))
                                    .show(ui, |ui| {
                                        ui.label(strong("BEST VS LANE").size(8.0).color(BG));
                                    });
                            }
                            if o.has_vs {
                                let (txt, col) = fmt_delta(o.matchup);
                                ui.label(strong(txt).size(13.0).color(col));
                            }
                        });
                    }
                });
            });
    }
}
