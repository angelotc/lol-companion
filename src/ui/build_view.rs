use egui::{Color32, CornerRadius, RichText, Sense, Ui, Vec2};

use crate::engine::views::{BuildView, DamageSplitView, OptionView, RuneNodeView, RunePageView};

use crate::images::ImageCache;
use crate::ui::theme::{AD, AP, BG, DIM, GOLD, LINE, MUTED, NEG, PANEL, POS, RAISED, TEXT, TEXT2, fmt_signed};

#[derive(Default)]
pub struct BuildUiState {
    pub page_idx: usize,
    pub last_champ: i32,
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

    egui::ScrollArea::vertical().id_salt("build_scroll").show(ui, |ui| {
        // Header card
        egui::Frame::new()
            .fill(PANEL)
            .stroke(egui::Stroke::new(1.0, LINE))
            .corner_radius(CornerRadius::same(6))
            .inner_margin(egui::Margin::same(8))
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    imgs.paint_icon(ui, &d.champ.name, &d.champ.icon, Vec2::splat(36.0), 6.0, Color32::WHITE);
                    ui.vertical(|ui| {
                        ui.label(RichText::new(&d.champ.name).size(15.0).strong().color(TEXT));
                        ui.label(
                            RichText::new(format!("{} · {:.1}% WR · {} games", d.lane, d.wr, d.games))
                                .size(11.0)
                                .color(MUTED),
                        );
                    });
                    if let Some(dmg) = &d.damage {
                        ui.add_space(16.0);
                        ui.allocate_ui(Vec2::new(260.0, 32.0), |ui| {
                            render_damage_bar(ui, dmg);
                        });
                    }
                    if !d.opponents.is_empty() {
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            for (e, lane, games, wr) in d.opponents.iter().rev() {
                                imgs.paint_icon(ui, &e.name, &e.icon, Vec2::splat(22.0), 4.0, Color32::WHITE)
                                    .on_hover_text(format!("vs {} ({lane}): {wr:.1}% WR · {games} games", e.name));
                            }
                            ui.label(RichText::new("Δ vs").size(10.5).color(MUTED));
                        });
                    }
                });
            });

        ui.add_space(8.0);
        ui.columns(2, |cols| {
            // Left: Runes + Spells + Keystones
            render_rune_card(&mut cols[0], d, st, imgs);
            cols[0].add_space(8.0);
            if !d.spells.is_empty() {
                section_card(&mut cols[0], "Summoner Spells", &d.spells, imgs);
            }
            if !d.keystones.is_empty() {
                cols[0].add_space(8.0);
                egui::Frame::new()
                    .fill(PANEL)
                    .stroke(egui::Stroke::new(1.0, LINE))
                    .corner_radius(CornerRadius::same(6))
                    .inner_margin(egui::Margin::same(8))
                    .show(&mut cols[0], |ui| {
                        ui.label(RichText::new("TOP KEYSTONES").size(10.0).strong().color(GOLD));
                        ui.add_space(4.0);
                        for k in &d.keystones {
                            ui.horizontal(|ui| {
                                imgs.paint_icon(ui, &k.name, &k.icon, Vec2::splat(22.0), 11.0, Color32::WHITE);
                                ui.label(RichText::new(&k.name).size(11.5).color(TEXT));
                                if let (Some(wr), Some(pr)) = (k.wr, k.pr) {
                                    ui.label(RichText::new(format!("{wr:.1}% · {pr:.1}%")).size(10.5).color(MUTED));
                                }
                                if k.has_vs {
                                    let col = if k.matchup >= 0.3 { POS } else if k.matchup <= -0.3 { NEG } else { DIM };
                                    ui.label(RichText::new(fmt_signed(k.matchup, 1)).size(11.0).strong().color(col));
                                }
                            });
                        }
                    });
            }

            // Right: Item sections + Skill order
            for sec in &d.items {
                section_card(&mut cols[1], &sec.title, &sec.options, imgs);
                cols[1].add_space(6.0);
            }
            if !d.skill_order.is_empty() {
                egui::Frame::new()
                    .fill(PANEL)
                    .stroke(egui::Stroke::new(1.0, LINE))
                    .corner_radius(CornerRadius::same(6))
                    .inner_margin(egui::Margin::same(8))
                    .show(&mut cols[1], |ui| {
                        ui.label(RichText::new("SKILL ORDER").size(10.0).strong().color(GOLD));
                        ui.add_space(4.0);
                        ui.horizontal_wrapped(|ui| {
                            for ch in d.skill_order.chars() {
                                let (bg, fg) = if ch == 'R' { (GOLD, BG) } else { (RAISED, TEXT2) };
                                let (rect, _) = ui.allocate_exact_size(Vec2::splat(18.0), Sense::hover());
                                ui.painter().rect_filled(rect, CornerRadius::same(3), bg);
                                ui.painter().text(
                                    rect.center(),
                                    egui::Align2::CENTER_CENTER,
                                    ch.to_string(),
                                    egui::FontId::proportional(10.5),
                                    fg,
                                );
                            }
                        });
                    });
            }
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
        let (rect, resp) = ui.allocate_exact_size(Vec2::new(ui.available_width().min(260.0), 5.0), Sense::hover());
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
        .stroke(egui::Stroke::new(1.0, LINE))
        .corner_radius(CornerRadius::same(6))
        .inner_margin(egui::Margin::same(8))
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                if d.pages.len() > 1 {
                    for (i, pg) in d.pages.iter().enumerate() {
                        if ui.selectable_label(i == st.page_idx, &pg.label).clicked() {
                            st.page_idx = i;
                        }
                    }
                } else {
                    ui.label(RichText::new(&page.label).size(11.0).strong().color(GOLD));
                }
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(
                        RichText::new(format!("{:.1}% WR · {} games", page.wr, page.games))
                            .size(10.5)
                            .color(MUTED),
                    );
                });
            });
            ui.add_space(6.0);
            ui.columns(2, |cols| {
                // Primary tree
                if let Some(s) = page.styles.first() {
                    cols[0].horizontal(|ui| {
                        imgs.paint_icon(ui, &s.name, &s.icon, Vec2::splat(16.0), 8.0, Color32::WHITE);
                        ui.label(RichText::new(&s.name).size(11.0).strong().color(TEXT));
                    });
                }
                for row in &page.tree_primary {
                    rune_tree_row(&mut cols[0], row, imgs);
                }
                // Secondary tree + shards
                if let Some(s) = page.styles.get(1) {
                    cols[1].horizontal(|ui| {
                        imgs.paint_icon(ui, &s.name, &s.icon, Vec2::splat(16.0), 8.0, Color32::WHITE);
                        ui.label(RichText::new(&s.name).size(11.0).strong().color(TEXT));
                    });
                }
                for row in &page.tree_secondary {
                    rune_tree_row(&mut cols[1], row, imgs);
                }
                cols[1].add_space(4.0);
                for row in &page.tree_shards {
                    rune_tree_row(&mut cols[1], row, imgs);
                }
            });
        });
}

fn rune_tree_row(ui: &mut Ui, row: &[RuneNodeView], imgs: &mut ImageCache) {
    ui.horizontal(|ui| {
        let mut chosen_node = None;
        for r in row {
            let tint = if r.chosen {
                Color32::WHITE
            } else {
                Color32::from_rgba_unmultiplied(255, 255, 255, 75)
            };
            if r.chosen {
                chosen_node = Some(r);
            }
            let resp = imgs.paint_icon(ui, &r.name, &r.icon, Vec2::splat(22.0), 11.0, tint);
            let mut tip = r.name.clone();
            if let (Some(wr), Some(pr), Some(g)) = (r.wr, r.pr, r.games) {
                tip.push_str(&format!("\n{wr:.1}% WR · {pr:.1}% picked · {g} games"));
            }
            if r.has_vs {
                tip.push_str(&format!("\nΔ vs lane: {}", fmt_signed(r.matchup, 1)));
            }
            resp.on_hover_text(tip);
        }
        if let Some(p) = chosen_node
            && p.has_vs
            && p.matchup.abs() >= 0.3
        {
            let col = if p.matchup > 0.0 { POS } else { NEG };
            ui.label(RichText::new(fmt_signed(p.matchup, 1)).size(10.5).strong().color(col));
        }
    });
}

fn section_card(ui: &mut Ui, title: &str, opts: &[OptionView], imgs: &mut ImageCache) {
    egui::Frame::new()
        .fill(PANEL)
        .stroke(egui::Stroke::new(1.0, LINE))
        .corner_radius(CornerRadius::same(6))
        .inner_margin(egui::Margin::same(8))
        .show(ui, |ui| {
            ui.label(RichText::new(title.to_uppercase()).size(10.0).strong().color(GOLD));
            ui.add_space(4.0);
            for o in opts {
                ui.horizontal(|ui| {
                    for (i, p) in o.parts.iter().enumerate() {
                        if i > 0 {
                            ui.label(RichText::new("›").size(10.0).color(DIM));
                        }
                        imgs.paint_icon(ui, &p.name, &p.icon, Vec2::splat(24.0), 4.0, Color32::WHITE)
                            .on_hover_text(if p.plaintext.is_empty() {
                                p.name.clone()
                            } else {
                                format!("{}\n{}", p.name, p.plaintext)
                            });
                    }
                    ui.label(RichText::new(format!("{:.1}%", o.wr)).size(11.5).strong().color(TEXT));
                    ui.label(RichText::new(format!("{:.1}%", o.pr)).size(10.5).color(MUTED));
                    if o.has_vs {
                        let col = if o.matchup >= 0.3 {
                            POS
                        } else if o.matchup <= -0.3 {
                            NEG
                        } else {
                            DIM
                        };
                        ui.label(RichText::new(fmt_signed(o.matchup, 1)).size(11.0).strong().color(col));
                    }
                    if o.best {
                        ui.label(RichText::new("BEST").size(9.0).strong().color(POS));
                    }
                });
            }
        });
}
