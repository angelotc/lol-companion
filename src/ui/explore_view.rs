use egui::{Align, Align2, Color32, CornerRadius, FontId, Layout, Pos2, Rect, RichText, Sense, Stroke, Ui, Vec2};
use std::sync::Arc;
use tokio::sync::mpsc;

use crate::domain::scoring::LANES;
use crate::engine::controller::Controller;
use crate::engine::views::{ExploreChampView, ExploreReportView};

use crate::images::ImageCache;
use crate::ui::theme::{ACCENT, BG, BG2, DIM, LINE, LINE2, MUTED, NEG, PANEL, POS, RAISED, TEXT, TEXT2, fmt_signed, strong, strong_font};

pub struct ExploreUiState {
    pub role: String,
    pub tier: String,
    pub loading: bool,
    pub error: Option<String>,
    pub report: Option<ExploreReportView>,
    pub selected: Option<i32>,
    tx: mpsc::UnboundedSender<Result<ExploreReportView, String>>,
    rx: mpsc::UnboundedReceiver<Result<ExploreReportView, String>>,
    started: bool,
}

impl Default for ExploreUiState {
    fn default() -> Self {
        let (tx, rx) = mpsc::unbounded_channel();
        Self {
            role: "support".into(),
            tier: "master_plus".into(),
            loading: false,
            error: None,
            report: None,
            selected: None,
            tx,
            rx,
            started: false,
        }
    }
}

impl ExploreUiState {
    fn trigger_load(&mut self, ctl: &Arc<Controller>, ctx: &egui::Context) {
        self.loading = true;
        self.error = None;
        let ctl_c = Arc::clone(ctl);
        let role = self.role.clone();
        let tier = self.tier.clone();
        let tx = self.tx.clone();
        let ctx_c = ctx.clone();
        tokio::spawn(async move {
            let res = ctl_c.explore_report(&role, &tier).await;
            let _ = tx.send(res);
            ctx_c.request_repaint();
        });
    }

    pub fn poll(&mut self, ctl: &Arc<Controller>, current_tier: &str, ctx: &egui::Context) {
        if !self.started {
            self.started = true;
            if let Some(r) = ctl.main_role()
                && LANES.contains(&r.as_str())
            {
                self.role = r;
            }
            self.tier = current_tier.to_string();
            self.trigger_load(ctl, ctx);
        } else if self.tier != current_tier {
            self.tier = current_tier.to_string();
            self.trigger_load(ctl, ctx);
        }

        while let Ok(res) = self.rx.try_recv() {
            self.loading = false;
            match res {
                Ok(rep) => {
                    if self.selected.is_none_or(|s| !rep.champs.iter().any(|c| c.champ.id == s)) {
                        self.selected = rep.champs.first().map(|c| c.champ.id);
                    }
                    self.report = Some(rep);
                    self.error = None;
                }
                Err(e) => {
                    self.error = Some(e);
                }
            }
        }
    }
}

fn role_label(r: &str) -> &'static str {
    match r {
        "top" => "Top",
        "jungle" => "Jungle",
        "middle" => "Mid",
        "bottom" => "Bot",
        "support" => "Support",
        _ => "?",
    }
}

pub fn render_explore(
    ui: &mut Ui,
    ctl: &Arc<Controller>,
    current_tier: &str,
    st: &mut ExploreUiState,
    imgs: &mut ImageCache,
) {
    st.poll(ctl, current_tier, ui.ctx());

    // Header row: inline title + guide on left, tight segmented role pill on right
    ui.horizontal(|ui| {
        ui.label(strong("What can you pick blind?").size(19.0).color(TEXT));
        ui.add_space(6.0);
        ui.label(
            RichText::new("Right = stronger. Up = loses less when the enemy counters it.")
                .size(11.5)
                .color(MUTED),
        );
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            egui::Frame::new()
                .fill(BG2)
                .stroke(Stroke::new(1.0, LINE2))
                .corner_radius(CornerRadius::same(16))
                .inner_margin(egui::Margin::symmetric(4, 3))
                .show(ui, |ui| {
                    // right_to_left inherits cleanly without stretching across empty header space
                    for &r in LANES.iter().rev() {
                        let on = st.role == r;
                        let btn = egui::Button::new(
                            RichText::new(role_label(r))
                                .size(11.5)
                                .strong()
                                .color(if on { BG } else { MUTED }),
                        )
                        .fill(if on { ACCENT } else { Color32::TRANSPARENT })
                        .stroke(Stroke::NONE)
                        .min_size(Vec2::new(44.0, 22.0))
                        .corner_radius(CornerRadius::same(13));
                        if ui.add(btn).on_hover_cursor(egui::CursorIcon::PointingHand).clicked() && !on {
                            st.role = r.to_string();
                            st.trigger_load(ctl, ui.ctx());
                        }
                    }
                });
        });
    });
    ui.add_space(8.0);

    if st.loading {
        ui.centered_and_justified(|ui| {
            ui.label(
                RichText::new(format!(
                    "Loading {} stats... (first time for a role takes a few seconds)",
                    role_label(&st.role)
                ))
                .color(MUTED),
            );
        });
        return;
    }

    if let Some(err) = st.error.clone() {
        ui.centered_and_justified(|ui| {
            ui.vertical_centered(|ui| {
                ui.label(RichText::new(err).color(NEG));
                if ui.button("Retry").on_hover_cursor(egui::CursorIcon::PointingHand).clicked() {
                    st.trigger_load(ctl, ui.ctx());
                }
            });
        });
        return;
    }

    let Some(rep) = st.report.clone() else {
        return;
    };
    if rep.champs.is_empty() {
        ui.centered_and_justified(|ui| {
            ui.label(RichText::new("No champions with enough games here.").color(MUTED));
        });
        return;
    }

    let mut tails: Vec<f64> = rep.champs.iter().map(|c| c.tail).collect();
    tails.sort_by(|a, b| a.total_cmp(b));
    let med = tails[tails.len() / 2];

    let avail_w = ui.available_width();
    let side_w = (avail_w * 0.28).clamp(288.0, 340.0);
    let chart_w = (avail_w - side_w - 16.0).max(100.0);
    let avail_h = ui.available_height().max(260.0);
    let (area, _) = ui.allocate_exact_size(Vec2::new(avail_w, avail_h), Sense::hover());
    let chart = Rect::from_min_size(area.min, Vec2::new(chart_w, avail_h));
    ui.painter().rect_filled(chart, CornerRadius::same(8), PANEL);
    ui.painter().rect_stroke(chart, CornerRadius::same(8), Stroke::new(1.0, LINE), egui::StrokeKind::Inside);
    let mut chart_ui = ui.new_child(egui::UiBuilder::new().id_salt("explore_chart").max_rect(chart.shrink(10.0)));
    chart_ui.set_clip_rect(chart.shrink(1.0).intersect(ui.clip_rect()));
    render_scatter(&mut chart_ui, chart.shrink(10.0).size(), &rep, med, &mut st.selected, imgs);

    let sidebar = Rect::from_min_max(Pos2::new(chart.right() + 16.0, area.top()), area.max);
    let mut side_ui = ui.new_child(egui::UiBuilder::new().id_salt("explore_sidebar").max_rect(sidebar));
    side_ui.set_clip_rect(sidebar.intersect(ui.clip_rect()));
    side_ui.set_width(side_w);
    if let Some(sel) = rep.champs.iter().find(|c| Some(c.champ.id) == st.selected).or_else(|| rep.champs.first()) {
        render_explore_detail(&mut side_ui, sel, med, imgs);
    }
    side_ui.add_space(8.0);
    render_explore_list(&mut side_ui, &rep, &mut st.selected, imgs);
}

fn render_explore_list(ui: &mut Ui, rep: &ExploreReportView, selected: &mut Option<i32>, imgs: &mut ImageCache) {
    let (card, _) = ui.allocate_exact_size(ui.available_size(), Sense::hover());
    ui.painter().rect_filled(card, CornerRadius::same(8), PANEL);
    ui.painter().rect_stroke(card, CornerRadius::same(8), Stroke::new(1.0, LINE), egui::StrokeKind::Inside);
    let inner = card.shrink(10.0);
    let p = ui.painter();
    p.text(inner.min, Align2::LEFT_TOP, "BEST BLIND PICKS", strong_font(10.0), MUTED);
    p.line_segment([Pos2::new(inner.left(), inner.top() + 34.0), Pos2::new(inner.right(), inner.top() + 34.0)], Stroke::new(1.0, LINE));
    let rows = Rect::from_min_max(Pos2::new(inner.left(), inner.top() + 39.0), inner.max);
    if rows.height() <= 0.0 { return; }
    let mut list_ui = ui.new_child(egui::UiBuilder::new().id_salt("explore_list").max_rect(rows));
    list_ui.set_clip_rect(rows.intersect(ui.clip_rect()));
    list_ui.spacing_mut().item_spacing.y = 0.0;
    let list = egui::ScrollArea::vertical().auto_shrink([false, false]).show_rows(&mut list_ui, 36.0, rep.champs.len(), |ui, range| {
        for i in range {
            let c = &rep.champs[i];
            let (row, resp) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 36.0), Sense::click());
            let on = *selected == Some(c.champ.id);
            let resp = resp.on_hover_cursor(egui::CursorIcon::PointingHand);
            if resp.clicked() { *selected = Some(c.champ.id); }
            if on || resp.hovered() {
                ui.painter().rect_filled(row.shrink(1.0), CornerRadius::same(4), if on { Color32::from_rgba_unmultiplied(ACCENT.r(), ACCENT.g(), ACCENT.b(), 22) } else { RAISED });
            }
            if on {
                ui.painter().rect_stroke(row.shrink(1.0), CornerRadius::same(4), Stroke::new(1.0, ACCENT.gamma_multiply(0.55)), egui::StrokeKind::Inside);
            }
            let p = ui.painter();
            p.text(Pos2::new(row.left() + 8.0, row.center().y), Align2::CENTER_CENTER, (i + 1).to_string(), FontId::proportional(11.0), DIM);
            let icon = Rect::from_min_size(Pos2::new(row.left() + 22.0, row.top() + 4.0), Vec2::splat(28.0));
            let mut icon_ui = ui.new_child(egui::UiBuilder::new().max_rect(icon));
            imgs.paint_icon(&mut icon_ui, &c.champ.name, &c.champ.icon, icon.size(), 4.0, Color32::WHITE);
            let name_rect = Rect::from_min_max(Pos2::new(row.left() + 60.0, row.top()), Pos2::new(row.right() - 122.0, row.bottom()));
            ui.painter_at(name_rect).text(name_rect.left_center(), Align2::LEFT_CENTER, &c.champ.name, strong_font(13.0), if on { ACCENT } else { TEXT });
            ui.painter().text(Pos2::new(row.right() - 74.0, row.center().y), Align2::RIGHT_CENTER, fmt_signed(c.base, 1), strong_font(12.5), if c.base > 0.0 { POS } else { NEG });
            ui.painter().text(Pos2::new(row.right() - 6.0, row.center().y), Align2::RIGHT_CENTER, fmt_signed(c.tail, 1), strong_font(12.5), if c.tail < 0.0 { NEG } else { POS });
            resp.on_hover_text(format!("{} · blind score {}", c.champ.name, fmt_signed(c.score, 2)));
        }
    });
    let right = list.inner_rect.right();
    ui.painter().text(Pos2::new(right - 93.0, inner.top() + 23.0), Align2::CENTER_CENTER, "Strength", FontId::proportional(9.5), MUTED);
    ui.painter().text(Pos2::new(right - 31.0, inner.top() + 23.0), Align2::CENTER_CENTER, "If countered", FontId::proportional(9.5), MUTED);
}

fn render_scatter(
    ui: &mut Ui,
    size: Vec2,
    rep: &ExploreReportView,
    med: f64,
    selected: &mut Option<i32>,
    imgs: &mut ImageCache,
) {
    let (rect, _) = ui.allocate_exact_size(size, Sense::hover());
    let plot = Rect::from_min_max(
        Pos2::new(rect.left() + 48.0, rect.top() + 8.0),
        Pos2::new(rect.right() - 12.0, rect.bottom() - 24.0),
    );
    if plot.width() <= 40.0 || plot.height() <= 40.0 {
        return;
    }

    let x0 = rep
        .champs
        .iter()
        .map(|c| c.base)
        .fold(f64::INFINITY, f64::min)
        .floor()
        - 0.5;
    let x1 = rep
        .champs
        .iter()
        .map(|c| c.base)
        .fold(f64::NEG_INFINITY, f64::max)
        .ceil()
        + 0.5;
    let y0 = (rep
        .champs
        .iter()
        .map(|c| c.tail)
        .fold(f64::INFINITY, f64::min)
        * 2.0)
        .floor()
        / 2.0
        - 0.25;
    let y1 = ((rep
        .champs
        .iter()
        .map(|c| c.tail)
        .fold(f64::NEG_INFINITY, f64::max)
        * 2.0)
        .ceil()
        / 2.0
        + 0.25)
        .min(0.25);

    let x_of = |v: f64| plot.left() + ((v - x0) / (x1 - x0).max(0.1)) as f32 * plot.width();
    let y_of = |v: f64| plot.top() + ((y1 - v) / (y1 - y0).max(0.1)) as f32 * plot.height();

    let p = ui.painter();
    let xz = x_of(0.0).clamp(plot.left(), plot.right());
    let ym = y_of(med).clamp(plot.top(), plot.bottom());

    // Top-right "BLIND THESE" quadrant highlight
    if xz < plot.right() && ym > plot.top() {
        p.rect_filled(
            Rect::from_min_max(Pos2::new(xz, plot.top()), Pos2::new(plot.right(), ym)),
            CornerRadius::ZERO,
            Color32::from_rgba_unmultiplied(0x3D, 0xDC, 0x97, 15),
        );
    }

    // Horizontal grid lines + y-axis ticks
    let sy = if y1 - y0 > 4.0 { 1.0 } else { 0.5 };
    let mut vy = (y0 / sy).ceil() * sy;
    while vy <= y1 + 1e-9 {
        let py = y_of(vy);
        p.line_segment(
            [Pos2::new(plot.left(), py), Pos2::new(plot.right(), py)],
            Stroke::new(1.0, LINE),
        );
        p.text(
            Pos2::new(plot.left() - 8.0, py),
            Align2::RIGHT_CENTER,
            fmt_signed(vy, 1),
            FontId::proportional(10.0),
            MUTED,
        );
        vy += sy;
    }

    // Dividers (x = 0 if inside plot, y = median)
    if x0 < 0.0 && x1 > 0.0 {
        p.line_segment([Pos2::new(xz, plot.top()), Pos2::new(xz, plot.bottom())], Stroke::new(1.0, LINE2));
    }
    p.line_segment([Pos2::new(plot.left(), ym), Pos2::new(plot.right(), ym)], Stroke::new(1.0, LINE2));

    // Left / bottom axes
    p.line_segment([Pos2::new(plot.left(), plot.top()), Pos2::new(plot.left(), plot.bottom())], Stroke::new(1.0, LINE2));
    p.line_segment([Pos2::new(plot.left(), plot.bottom()), Pos2::new(plot.right(), plot.bottom())], Stroke::new(1.0, LINE2));

    // Quadrant captions
    p.text(Pos2::new(plot.right() - 8.0, plot.top() + 8.0), Align2::RIGHT_TOP, "BLIND THESE", FontId::proportional(10.0), POS);
    p.text(Pos2::new(plot.left() + 8.0, plot.top() + 8.0), Align2::LEFT_TOP, "SAFE BUT WEAK", FontId::proportional(10.0), DIM);
    p.text(Pos2::new(plot.right() - 8.0, plot.bottom() - 8.0), Align2::RIGHT_BOTTOM, "STRONG, PICK LATE", FontId::proportional(10.0), DIM);
    p.text(Pos2::new(plot.left() + 8.0, plot.bottom() - 8.0), Align2::LEFT_BOTTOM, "AVOID BLIND", FontId::proportional(10.0), DIM);

    // X-axis tick labels (drop ticks near the right axis title)
    p.text(
        Pos2::new(plot.right(), plot.bottom() + 6.0),
        Align2::RIGHT_TOP,
        "strength ->",
        FontId::proportional(10.0),
        DIM,
    );
    let sx = if x1 - x0 > 8.0 { 2.0 } else { 1.0 };
    let mut vx = (x0 / sx).ceil() * sx;
    while vx <= x1 + 1e-9 {
        let px = x_of(vx);
        if px < plot.right() - 68.0 {
            let is_zero = vx.abs() < 1e-9;
            let lbl = if is_zero { "0 · average".into() } else { fmt_signed(vx, 0) };
            p.text(
                Pos2::new(px, plot.bottom() + 6.0),
                Align2::CENTER_TOP,
                lbl,
                FontId::proportional(10.0),
                if is_zero { TEXT2 } else { MUTED },
            );
        }
        vx += sx;
    }

    // Rotated vertical Y-axis title along left margin
    let galley = p.layout_no_wrap(
        "if countered (higher = safer) ->".into(),
        FontId::proportional(10.0),
        DIM,
    );
    let text_w = galley.rect.width();
    let y_title_pos = Pos2::new(rect.left() + 2.0, plot.bottom() - (plot.height() - text_w) * 0.5);
    p.add(egui::epaint::TextShape {
        pos: y_title_pos,
        galley,
        underline: Stroke::NONE,
        fallback_color: DIM,
        override_text_color: None,
        opacity_factor: 1.0,
        angle: -std::f32::consts::FRAC_PI_2,
    });

    // Draw unselected first, selected last
    let mut order: Vec<&ExploreChampView> = rep.champs.iter().collect();
    order.sort_by_key(|c| i32::from(Some(c.champ.id) == *selected));

    let mut pt_boxes: Vec<(i32, Rect)> = Vec::with_capacity(order.len());
    for c in &order {
        let on = Some(c.champ.id) == *selected;
        let pt_sz = if on { 42.0 } else { 34.0 };
        let cx = x_of(c.base);
        let cy = y_of(c.tail);
        pt_boxes.push((c.champ.id, Rect::from_center_size(Pos2::new(cx, cy), Vec2::splat(pt_sz))));
    }

    let mut hovered = None;
    for (idx, c) in order.iter().enumerate() {
        let (_, pt_rect) = pt_boxes[idx];
        let on = Some(c.champ.id) == *selected;
        let good = c.base > 0.0 && c.tail >= med;
        let pt_sz = pt_rect.width();

        let resp = ui.interact(pt_rect, ui.id().with(("exp_pt", c.champ.id)), Sense::click())
            .on_hover_cursor(egui::CursorIcon::PointingHand);
        if resp.hovered() {
            hovered = Some(c.champ.id);
        }
        if resp.clicked() {
            *selected = Some(c.champ.id);
        }

        let mut sub = ui.new_child(egui::UiBuilder::new().max_rect(pt_rect));
        imgs.paint_icon(&mut sub, &c.champ.name, &c.champ.icon, Vec2::splat(pt_sz), pt_sz * 0.5, Color32::WHITE);

        if on {
            ui.painter().circle_stroke(pt_rect.center(), pt_sz * 0.52, Stroke::new(2.5, ACCENT));
        } else if resp.hovered() {
            ui.painter().circle_stroke(pt_rect.center(), pt_sz * 0.53, Stroke::new(2.0, TEXT));
        } else if good {
            ui.painter().circle_stroke(pt_rect.center(), pt_sz * 0.51, Stroke::new(1.8, POS));
        } else {
            ui.painter().circle_stroke(pt_rect.center(), pt_sz * 0.50, Stroke::new(1.0, LINE2));
        }

        let w3: String = c
            .opps
            .iter()
            .filter(|o| o.games > 0 && o.d < 0.0)
            .take(3)
            .map(|o| format!("{} {}", o.champ.name, fmt_signed(o.d, 1)))
            .collect::<Vec<_>>()
            .join(", ");
        let mut tip = format!(
            "{} · blind score {}\nstrength {} · if countered {} · avg matchup {}",
            c.champ.name,
            fmt_signed(c.score, 2),
            fmt_signed(c.base, 2),
            fmt_signed(c.tail, 2),
            fmt_signed(c.mean, 2)
        );
        if !w3.is_empty() {
            tip.push_str(&format!("\nworst: {w3}"));
        }
        resp.on_hover_text(tip);
    }

    // Greedy non-overlapping name labels for "blind these" and selected (selected first)
    let mut want: Vec<(usize, &ExploreChampView)> = order
        .iter()
        .enumerate()
        .filter(|(_, c)| Some(c.champ.id) == *selected || Some(c.champ.id) == hovered || (c.base > 0.0 && c.tail >= med))
        .map(|(i, &c)| (i, c))
        .collect();
    want.sort_by(|a, b| {
        let ao = Some(a.1.champ.id) == *selected;
        let bo = Some(b.1.champ.id) == *selected;
        bo.cmp(&ao)
            .then_with(|| (Some(b.1.champ.id) == hovered).cmp(&(Some(a.1.champ.id) == hovered)))
            .then_with(|| b.1.score.total_cmp(&a.1.score))
    });

    let mut placed: Vec<Rect> = Vec::new();
    for (idx, c) in want {
        let on = Some(c.champ.id) == *selected;
        let emphasized = on || Some(c.champ.id) == hovered;
        let (_, pt_rect) = pt_boxes[idx];
        let font = if emphasized { strong_font(11.0) } else { FontId::proportional(10.0) };
        let galley = ui.painter().layout_no_wrap(c.champ.name.clone(), font, if on { ACCENT } else { TEXT });
        let lw = galley.rect.width() + 4.0;
        let lh = galley.rect.height() + 2.0;

        let below = Rect::from_center_size(Pos2::new(pt_rect.center().x, pt_rect.bottom() + lh * 0.5 + 2.0), Vec2::new(lw, lh));
        let above = Rect::from_center_size(Pos2::new(pt_rect.center().x, pt_rect.top() - lh * 0.5 - 2.0), Vec2::new(lw, lh));
        let right = Rect::from_center_size(Pos2::new(pt_rect.right() + lw * 0.5 + 4.0, pt_rect.center().y), Vec2::new(lw, lh));
        let left = Rect::from_center_size(Pos2::new(pt_rect.left() - lw * 0.5 - 4.0, pt_rect.center().y), Vec2::new(lw, lh));
        let clashes = |r: Rect| {
            !plot.contains_rect(r)
                || placed.iter().any(|p| p.intersects(r))
                || pt_boxes.iter().any(|(id, pb)| *id != c.champ.id && pb.shrink(2.0).intersects(r))
        };

        let chosen = [below, above, right, left,
            above.translate(Vec2::new(lw * 0.5, 0.0)),
            above.translate(Vec2::new(-lw * 0.5, 0.0)),
            below.translate(Vec2::new(lw * 0.5, 0.0)),
            below.translate(Vec2::new(-lw * 0.5, 0.0)),
        ].into_iter().find(|r| !clashes(*r));

        if let Some(lr) = chosen {
            if emphasized {
                ui.painter().rect_filled(lr.expand(1.0), CornerRadius::same(3), BG);
            }
            ui.painter().galley(Pos2::new(lr.left() + 2.0, lr.top() + 1.0), galley, if on { ACCENT } else { TEXT });
            placed.push(lr);
        }
    }
}

fn render_explore_detail(ui: &mut Ui, d: &ExploreChampView, med: f64, imgs: &mut ImageCache) {
    let above_average = d.base > 0.0;
    let safe = d.tail >= med;
    let (verdict, vcol) = match (above_average, safe) {
        (true, true) => ("Safe to pick blind", POS),
        (true, false) => ("Strong, but wait for their pick", ACCENT),
        (false, true) => ("Safe fallback, below avg strength", MUTED),
        (false, false) => ("Avoid picking blind", NEG),
    };

    egui::Frame::new()
        .fill(PANEL)
        .stroke(Stroke::new(1.0, ACCENT))
        .corner_radius(CornerRadius::same(8))
        .inner_margin(egui::Margin::same(10))
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                imgs.paint_icon(ui, &d.champ.name, &d.champ.icon, Vec2::splat(46.0), 6.0, Color32::WHITE);
                ui.add_space(6.0);
                ui.vertical(|ui| {
                    ui.label(strong(&d.champ.name).size(21.0).color(TEXT));
                    ui.label(RichText::new(verdict).size(11.0).strong().color(vcol));
                });
            });
            ui.add_space(8.0);
            ui.columns(2, |cols| {
                egui::Frame::new().fill(RAISED).corner_radius(6).inner_margin(8).show(&mut cols[0], |ui| {
                    ui.label(RichText::new("STRENGTH").size(9.0).color(MUTED));
                    ui.label(strong(fmt_signed(d.base, 1)).size(21.0).color(if d.base > 0.0 { POS } else { NEG }));
                });
                egui::Frame::new().fill(RAISED).corner_radius(6).inner_margin(8).show(&mut cols[1], |ui| {
                    ui.label(RichText::new("IF COUNTERED").size(9.0).color(MUTED));
                    ui.label(strong(fmt_signed(d.tail, 1)).size(21.0).color(if d.tail < 0.0 { NEG } else { POS }));
                });
            });
            let worst: Vec<_> = d.opps.iter().filter(|o| o.games > 0 && o.d < 0.0).take(3).collect();
            if !worst.is_empty() {
                ui.add_space(8.0);
                ui.horizontal_wrapped(|ui| {
                    ui.label(RichText::new("Worst counters").size(10.5).color(MUTED));
                    ui.add_space(4.0);
                    for w in worst {
                        imgs.paint_icon(ui, &w.champ.name, &w.champ.icon, Vec2::splat(20.0), 4.0, Color32::WHITE)
                            .on_hover_text(format!("{}: {}", w.champ.name, fmt_signed(w.d, 1)));
                        ui.label(RichText::new(&w.champ.name).size(11.0).color(TEXT2));
                        ui.add_space(4.0);
                    }
                });
            }
        });
}
