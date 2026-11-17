use egui::{Align, Align2, Color32, CornerRadius, FontId, Layout, Pos2, Rect, RichText, Sense, Stroke, Ui, Vec2};
use std::sync::Arc;
use tokio::sync::mpsc;

use crate::domain::scoring::LANES;
use crate::engine::controller::Controller;
use crate::engine::views::{ExploreChampView, ExploreReportView};

use crate::images::ImageCache;
use crate::ui::theme::{DIM, GOLD, LINE, LINE2, MUTED, NEG, PANEL, POS, RAISED, TEXT, fmt_signed};

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

    // Header row: title + role pills
    ui.horizontal(|ui| {
        ui.vertical(|ui| {
            ui.label(RichText::new("What can you pick blind?").size(15.0).strong().color(TEXT));
            ui.label(
                RichText::new("Right = stronger. Up = loses less when the enemy counters it.")
                    .size(10.5)
                    .color(MUTED),
            );
        });
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            for &r in LANES.iter().rev() {
                let on = st.role == r;
                if ui.selectable_label(on, role_label(r)).clicked() && !on {
                    st.role = r.to_string();
                    st.trigger_load(ctl, ui.ctx());
                }
            }
        });
    });
    ui.add_space(6.0);

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
                if ui.button("Retry").clicked() {
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
    let side_w = (avail_w * 0.32).clamp(240.0, 310.0);
    let chart_w = (avail_w - side_w - 10.0).max(320.0);
    let avail_h = ui.available_height().max(260.0);

    ui.horizontal_top(|ui| {
        // Scatter plot card
        egui::Frame::new()
            .fill(PANEL)
            .stroke(Stroke::new(1.0, LINE))
            .corner_radius(CornerRadius::same(6))
            .inner_margin(egui::Margin::same(8))
            .show(ui, |ui| {
                ui.set_min_size(Vec2::new(chart_w - 18.0, avail_h - 18.0));
                render_scatter(ui, &rep, med, &mut st.selected, imgs);
            });

        ui.add_space(8.0);

        // Right side: detail + top list
        ui.allocate_ui_with_layout(
            Vec2::new(side_w, avail_h),
            Layout::top_down(Align::Min),
            |ui| {
                if let Some(sel) = rep
                    .champs
                    .iter()
                    .find(|c| Some(c.champ.id) == st.selected)
                    .or_else(|| rep.champs.first())
                {
                    render_explore_detail(ui, sel, med, imgs);
                }
                ui.add_space(6.0);
                egui::Frame::new()
                    .fill(PANEL)
                    .stroke(Stroke::new(1.0, LINE))
                    .corner_radius(CornerRadius::same(6))
                    .inner_margin(egui::Margin::same(8))
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            ui.label(RichText::new("BEST BLIND PICKS").size(10.0).strong().color(GOLD));
                            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                ui.label(RichText::new("Str · If cnt").size(9.5).color(MUTED));
                            });
                        });
                        ui.separator();
                        egui::ScrollArea::vertical().id_salt("exp_list").show(ui, |ui| {
                            for (i, c) in rep.champs.iter().take(18).enumerate() {
                                let on = st.selected == Some(c.champ.id);
                                let resp = ui
                                    .horizontal(|ui| {
                                        ui.add_sized([18.0, 22.0], egui::Label::new(RichText::new(format!("{}", i + 1)).size(10.5).color(DIM)));
                                        imgs.paint_icon(ui, &c.champ.name, &c.champ.icon, Vec2::splat(20.0), 3.0, Color32::WHITE);
                                        ui.add_sized([92.0, 22.0], egui::Label::new(RichText::new(&c.champ.name).size(11.0).strong().color(if on { GOLD } else { TEXT })).truncate());
                                        let sc = if c.base > 0.0 { POS } else { NEG };
                                        let tc = if c.tail < 0.0 { NEG } else { POS };
                                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                            ui.label(RichText::new(fmt_signed(c.tail, 1)).size(10.5).color(tc));
                                            ui.label(RichText::new("·").color(DIM));
                                            ui.label(RichText::new(fmt_signed(c.base, 1)).size(10.5).color(sc));
                                        });
                                    })
                                    .response
                                    .interact(Sense::click());
                                if resp.clicked() {
                                    st.selected = Some(c.champ.id);
                                }
                            }
                        });
                    });
            },
        );
    });
}

fn render_scatter(
    ui: &mut Ui,
    rep: &ExploreReportView,
    med: f64,
    selected: &mut Option<i32>,
    imgs: &mut ImageCache,
) {
    let size = ui.available_size();
    let (rect, _) = ui.allocate_exact_size(size, Sense::hover());
    let plot = rect.shrink2(Vec2::new(34.0, 22.0));
    if plot.width() <= 20.0 || plot.height() <= 20.0 {
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
    p.rect_filled(
        Rect::from_min_max(Pos2::new(xz, plot.top()), Pos2::new(plot.right(), ym)),
        CornerRadius::ZERO,
        Color32::from_rgba_unmultiplied(0x3D, 0xDC, 0x97, 14),
    );

    // Dividers
    p.line_segment([Pos2::new(xz, plot.top()), Pos2::new(xz, plot.bottom())], Stroke::new(1.0, LINE2));
    p.line_segment([Pos2::new(plot.left(), ym), Pos2::new(plot.right(), ym)], Stroke::new(1.0, LINE2));
    p.rect_stroke(plot, CornerRadius::ZERO, Stroke::new(1.0, LINE), egui::StrokeKind::Inside);

    // Quadrant captions
    p.text(Pos2::new(plot.right() - 6.0, plot.top() + 6.0), Align2::RIGHT_TOP, "BLIND THESE", FontId::proportional(9.5), POS);
    p.text(Pos2::new(plot.left() + 6.0, plot.top() + 6.0), Align2::LEFT_TOP, "SAFE BUT WEAK", FontId::proportional(9.5), DIM);
    p.text(Pos2::new(plot.right() - 6.0, plot.bottom() - 6.0), Align2::RIGHT_BOTTOM, "STRONG, PICK LATE", FontId::proportional(9.5), DIM);
    p.text(Pos2::new(plot.left() + 6.0, plot.bottom() - 6.0), Align2::LEFT_BOTTOM, "AVOID BLIND", FontId::proportional(9.5), DIM);

    // Axis tick labels
    let sx = if x1 - x0 > 8.0 { 2.0 } else { 1.0 };
    let mut vx = (x0 / sx).ceil() * sx;
    while vx <= x1 + 1e-9 {
        let px = x_of(vx);
        let lbl = if vx.abs() < 1e-9 { "0".into() } else { fmt_signed(vx, 0) };
        p.text(Pos2::new(px, plot.bottom() + 4.0), Align2::CENTER_TOP, lbl, FontId::proportional(9.5), MUTED);
        vx += sx;
    }
    let sy = if y1 - y0 > 4.0 { 1.0 } else { 0.5 };
    let mut vy = (y0 / sy).ceil() * sy;
    while vy <= y1 + 1e-9 {
        let py = y_of(vy);
        p.text(Pos2::new(plot.left() - 4.0, py), Align2::RIGHT_CENTER, fmt_signed(vy, 1), FontId::proportional(9.5), MUTED);
        vy += sy;
    }

    // Draw unselected first, selected last so it stays on top
    let mut order: Vec<&ExploreChampView> = rep.champs.iter().collect();
    order.sort_by_key(|c| i32::from(Some(c.champ.id) == *selected));

    for c in order {
        let cx = x_of(c.base);
        let cy = y_of(c.tail);
        let on = Some(c.champ.id) == *selected;
        let good = c.base > 0.0 && c.tail >= med;
        let pt_sz = if on { 26.0 } else { 22.0 };
        let pt_rect = Rect::from_center_size(Pos2::new(cx, cy), Vec2::splat(pt_sz));

        let resp = ui.interact(pt_rect, ui.id().with(("exp_pt", c.champ.id)), Sense::click());
        if resp.clicked() {
            *selected = Some(c.champ.id);
        }
        if on {
            ui.painter().circle_stroke(Pos2::new(cx, cy), pt_sz * 0.56, Stroke::new(2.0, GOLD));
        } else if good {
            ui.painter().circle_stroke(Pos2::new(cx, cy), pt_sz * 0.54, Stroke::new(1.5, POS));
        }

        let mut sub = ui.new_child(egui::UiBuilder::new().max_rect(pt_rect));
        imgs.paint_icon(&mut sub, &c.champ.name, &c.champ.icon, Vec2::splat(pt_sz), pt_sz * 0.5, Color32::WHITE);
        if on || good {
            ui.painter().text(
                Pos2::new(cx, cy + pt_sz * 0.5 + 2.0),
                Align2::CENTER_TOP,
                &c.champ.name,
                FontId::proportional(if on { 10.5 } else { 9.0 }),
                if on { GOLD } else { TEXT },
            );
        }
        resp.on_hover_text(format!(
            "{} · blind score {}\nstrength {} · if countered {} · avg matchup {}",
            c.champ.name,
            fmt_signed(c.score, 2),
            fmt_signed(c.base, 2),
            fmt_signed(c.tail, 2),
            fmt_signed(c.mean, 2)
        ));
    }
}

fn render_explore_detail(ui: &mut Ui, d: &ExploreChampView, med: f64, imgs: &mut ImageCache) {
    let strong = d.base > 0.0;
    let safe = d.tail >= med;
    let (verdict, vcol) = match (strong, safe) {
        (true, true) => ("Safe to pick blind", POS),
        (true, false) => ("Strong, but wait for their pick", GOLD),
        (false, true) => ("Safe fallback, below avg strength", MUTED),
        (false, false) => ("Avoid picking blind", NEG),
    };

    egui::Frame::new()
        .fill(PANEL)
        .stroke(Stroke::new(1.0, LINE))
        .corner_radius(CornerRadius::same(6))
        .inner_margin(egui::Margin::same(8))
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                imgs.paint_icon(ui, &d.champ.name, &d.champ.icon, Vec2::splat(34.0), 6.0, Color32::WHITE);
                ui.vertical(|ui| {
                    ui.label(RichText::new(&d.champ.name).size(14.0).strong().color(TEXT));
                    ui.label(RichText::new(verdict).size(10.5).strong().color(vcol));
                });
            });
            ui.add_space(6.0);
            ui.columns(2, |cols| {
                egui::Frame::new().fill(RAISED).corner_radius(4).inner_margin(6).show(&mut cols[0], |ui| {
                    ui.label(RichText::new("STRENGTH").size(9.0).color(MUTED));
                    ui.label(RichText::new(fmt_signed(d.base, 1)).size(14.0).strong().color(if d.base > 0.0 { POS } else { NEG }));
                });
                egui::Frame::new().fill(RAISED).corner_radius(4).inner_margin(6).show(&mut cols[1], |ui| {
                    ui.label(RichText::new("IF COUNTERED").size(9.0).color(MUTED));
                    ui.label(RichText::new(fmt_signed(d.tail, 1)).size(14.0).strong().color(if d.tail < 0.0 { NEG } else { POS }));
                });
            });
            let worst: Vec<_> = d.opps.iter().filter(|o| o.games > 0 && o.d < 0.0).take(3).collect();
            if !worst.is_empty() {
                ui.add_space(4.0);
                ui.horizontal_wrapped(|ui| {
                    ui.label(RichText::new("Worst:").size(10.0).color(MUTED));
                    for w in worst {
                        imgs.paint_icon(ui, &w.champ.name, &w.champ.icon, Vec2::splat(16.0), 3.0, Color32::WHITE)
                            .on_hover_text(format!("{}: {}", w.champ.name, fmt_signed(w.d, 1)));
                        ui.label(RichText::new(&w.champ.name).size(10.0).color(TEXT));
                    }
                });
            }
        });
}
