use egui::{Align, Align2, Color32, CornerRadius, FontId, Layout, Pos2, Rangef, Rect, RichText, Sense, Stroke, StrokeKind, Ui, Vec2};

use crate::engine::views::{BlindView, ChampRef, ContribView, PicksView, RecView};

use crate::images::ImageCache;
use crate::ui::theme::{ACCENT, ALLY, DIM, ENEMY, LINE, LINE2, MUTED, NEG, PANEL, POS, RAISED, TEXT, TEXT2, fmt_signed, heat_bg, short_lane, strong, strong_font};

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

// Every header, cell, and highlight derives its horizontal bounds from these columns.
pub(super) struct TableColumns {
    spans: Vec<Rangef>,
    width: f32,
}

impl TableColumns {
    const PADDING: f32 = 8.0;
    const CELL_GAP: f32 = 6.0;
    const GROUP_GAP: f32 = 10.0;

    pub(super) fn matchups(viewport_width: f32, value_count: usize, groups: &[usize]) -> Self {
        let mut widths = vec![22.0, 147.0, 72.0];
        widths.extend(std::iter::repeat_n(41.0, value_count));
        let extra = Self::remaining_width(viewport_width, &widths, groups.len());
        let value_extra = (extra / value_count as f32).floor().min(23.0);
        for width in &mut widths[3..] {
            *width += value_extra;
        }
        widths[1] += (extra - value_extra * value_count as f32).max(0.0);
        Self::new(&widths, groups)
    }

    fn blind(viewport_width: f32) -> Self {
        let mut widths = [22.0, 168.0, 72.0, 74.0, 74.0, 240.0, 82.0, 99.0, 91.0];
        let extra = Self::remaining_width(viewport_width, &widths, 0);
        let name_extra = (extra * 0.28).round();
        let counter_extra = (extra * 0.12).round();
        widths[1] += name_extra;
        widths[5] += extra - name_extra - counter_extra;
        widths[7] += counter_extra;
        Self::new(&widths, &[])
    }

    fn pick_detail(viewport_width: f32) -> Self {
        let mut widths = [52.0, 57.0, 46.0, 43.0];
        widths[0] += Self::remaining_width(viewport_width, &widths, 0);
        Self::new(&widths, &[])
    }

    fn remaining_width(viewport_width: f32, widths: &[f32], groups: usize) -> f32 {
        // Use whole points so accumulated fractional widths don't trigger needless scrollbars.
        let gaps = (widths.len() - 1) as f32 * Self::CELL_GAP + groups as f32 * Self::GROUP_GAP;
        (viewport_width.floor() - widths.iter().sum::<f32>() - gaps - 2.0 * Self::PADDING).max(0.0)
    }

    fn new(widths: &[f32], groups: &[usize]) -> Self {
        let mut x = Self::PADDING;
        let spans = widths
            .iter()
            .enumerate()
            .map(|(idx, width)| {
                if idx > 0 {
                    x += Self::CELL_GAP;
                }
                if groups.contains(&idx) {
                    x += Self::GROUP_GAP;
                }
                let span = Rangef::new(x, x + width);
                x = span.max;
                span
            })
            .collect();
        Self { spans, width: x + Self::PADDING }
    }

    pub(super) fn row(&self, ui: &mut Ui, height: f32, sense: Sense) -> (Rect, egui::Response) {
        ui.allocate_exact_size(Vec2::new(self.width, height), sense)
    }

    pub(super) fn cell(&self, row: Rect, index: usize) -> Rect {
        let span = self.spans[index];
        Rect::from_min_max(
            Pos2::new(row.left() + span.min, row.top()),
            Pos2::new(row.left() + span.max, row.bottom()),
        )
    }

    pub(super) fn champion_name(&self, row: Rect) -> Rect {
        let mut cell = self.cell(row, 1);
        cell.min.x += 45.0;
        cell
    }

    pub(super) fn champion_heading(&self, ui: &Ui, row: Rect, label: &str) {
        table_text(ui, self.champion_name(row), Align::Min, label, FontId::proportional(11.0), MUTED);
    }

    pub(super) fn heading(&self, ui: &Ui, row: Rect, col: usize, label: &str, align: Align) {
        table_text(ui, self.cell(row, col), align, label, FontId::proportional(11.0), MUTED);
    }

    pub(super) fn group(
        &self,
        ui: &Ui,
        row: Rect,
        cols: std::ops::Range<usize>,
        label: &str,
        color: Color32,
    ) {
        if cols.is_empty() {
            return;
        }
        let first = self.cell(row, cols.start);
        let last = self.cell(row, cols.end - 1);
        let group = first.union(last);
        // A short group can have a single portrait, so allow its label to wrap above it.
        let p = ui.painter().with_clip_rect(group);
        let text = p.layout(label.to_owned(), strong_font(10.0), color, group.width());
        let x = if text.rows.len() == 1 && cols.len() == 1 {
            group.center().x - text.size().x * 0.5
        } else {
            group.left()
        };
        p.galley(Pos2::new(x, group.bottom() - 2.0 - text.size().y), text, color);
        let y = group.bottom() - 0.5;
        p.line_segment([Pos2::new(group.left(), y), Pos2::new(group.right(), y)], Stroke::new(1.0, color));
    }
}

pub(super) fn table_panel_widths(ui: &Ui) -> (f32, f32, f32) {
    let available = ui.available_width();
    let side = (available * 0.30).clamp(260.0, 380.0);
    let gap = 16.0;
    let main = (available - side - gap - 2.0 * ui.spacing().item_spacing.x).max(1.0);
    (main, side, gap)
}

pub(super) fn table_with_note(
    ui: &mut Ui,
    id: &str,
    note: &str,
    render: impl FnOnce(&mut Ui),
) {
    let note = ui.painter().layout(note.to_owned(), FontId::proportional(11.0), MUTED, ui.available_width());
    let height = (ui.available_height() - note.size().y - ui.spacing().item_spacing.y).max(0.0);
    egui::ScrollArea::both()
        .id_salt(id)
        .max_height(height)
        .min_scrolled_height(0.0)
        .min_scrolled_width(0.0)
        .auto_shrink([false, false])
        .show(ui, |ui| {
            ui.spacing_mut().item_spacing.y = 3.0;
            render(ui);
        });
    ui.label(note);
}

pub(super) fn table_divider(ui: &Ui, row: Rect) {
    ui.painter().line_segment([row.left_bottom(), row.right_bottom()], Stroke::new(1.0, LINE));
}

pub(super) fn table_text(ui: &Ui, cell: Rect, align: Align, text: &str, font: FontId, color: Color32) {
    let (pos, anchor) = match align {
        Align::Min => (cell.left_center(), Align2::LEFT_CENTER),
        Align::Center => (cell.center(), Align2::CENTER_CENTER),
        Align::Max => (cell.right_center(), Align2::RIGHT_CENTER),
    };
    ui.painter().with_clip_rect(cell).text(pos, anchor, text, font, color);
}

pub(super) fn table_icon(
    ui: &mut Ui,
    imgs: &mut ImageCache,
    champ: &ChampRef,
    rect: Rect,
    tint: Color32,
) -> egui::Response {
    let mut child = ui.new_child(
        egui::UiBuilder::new().max_rect(rect).layout(Layout::top_down(Align::Min)),
    );
    imgs.paint_icon(&mut child, &champ.name, &champ.icon, rect.size(), 5.0, tint)
}

pub(super) fn paint_rank(ui: &Ui, cell: Rect, index: usize, color: Color32) {
    table_text(ui, cell, Align::Min, &(index + 1).to_string(), strong_font(12.0), color);
}

pub(super) fn paint_table_champion(ui: &mut Ui, imgs: &mut ImageCache, cell: Rect, champ: &ChampRef) {
    let icon = Rect::from_center_size(cell.left_center() + Vec2::new(17.5, 0.0), Vec2::splat(35.0));
    table_icon(ui, imgs, champ, icon, Color32::WHITE);
}

pub(super) fn paint_row(ui: &Ui, row: Rect, selected: Option<Color32>, hovered: bool, locked: bool) {
    let fill = if selected.is_some() || hovered {
        RAISED
    } else if locked {
        POS.gamma_multiply(0.10)
    } else {
        Color32::TRANSPARENT
    };
    let stroke = selected.map_or(Stroke::NONE, |color| Stroke::new(1.0, color));
    ui.painter().rect(row, CornerRadius::same(6), fill, stroke, StrokeKind::Inside);
}

pub(super) fn paint_score(ui: &Ui, cell: Rect, text: &str, color: Color32, fraction: f32) {
    table_text(ui, cell.translate(Vec2::new(0.0, -4.0)), Align::Max, text, strong_font(18.0), color);
    let bar = Rect::from_min_size(
        cell.left_center() + Vec2::new(0.0, 10.0),
        Vec2::new(cell.width(), 3.0),
    );
    paint_bar(ui, bar, fraction, color);
}

pub(super) fn paint_bar(ui: &Ui, bar: Rect, fraction: f32, color: Color32) {
    ui.painter().rect_filled(bar, CornerRadius::same(2), LINE);
    let mut fill = bar;
    fill.set_width(bar.width() * fraction.clamp(0.0, 1.0));
    ui.painter().rect_filled(fill, CornerRadius::same(2), color);
}

pub(super) fn paint_value(ui: &Ui, cell: Rect, value: &str, bg: Color32, fg: Color32) {
    let pill = Rect::from_center_size(cell.center(), Vec2::new(cell.width(), 29.0));
    ui.painter().rect_filled(pill, CornerRadius::same(5), bg);
    table_text(ui, pill, Align::Center, value, strong_font(12.0), fg);
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

    let (main_w, side_w, gap) = table_panel_widths(ui);
    let height = ui.available_height();

    ui.horizontal_top(|ui| {
        ui.allocate_ui_with_layout(Vec2::new(main_w, height), Layout::top_down(Align::Min), |ui| {
            if let Some(locked) = &d.locked {
                ui.horizontal(|ui| {
                    imgs.paint_icon(ui, &locked.name, &locked.icon, Vec2::splat(18.0), 4.0, Color32::WHITE);
                    ui.label(strong(format!("Locked in: {}", locked.name)).size(12.0).color(ACCENT));
                    ui.label(RichText::new("picks as they were at lock-in").size(11.0).color(MUTED));
                });
                ui.add_space(4.0);
            }

            table_with_note(
                ui,
                "picks_tbl",
                "Each cell is the win-rate points that term adds to the score: strength, each matchup and duo, and your own games and mastery. Click a pick for the raw numbers.",
                |ui| {
                    const FIRST_MATCHUP: usize = 4;
                    let own_idx = FIRST_MATCHUP + tcols.len();
                    let mut groups = vec![own_idx];
                    if n_vs > 0 && n_with > 0 {
                        groups.push(FIRST_MATCHUP + n_vs);
                    }
                    let columns = TableColumns::matchups(ui.available_width(), tcols.len() + 2, &groups);
                    let group_height = if n_vs == 1 || n_with == 1 { 28.0 } else { 19.0 };
                    let (group_row, _) = columns.row(ui, group_height, Sense::hover());
                    columns.group(ui, group_row, FIRST_MATCHUP..FIRST_MATCHUP + n_vs, "AGAINST", ENEMY);
                    columns.group(ui, group_row, FIRST_MATCHUP + n_vs..own_idx, "WITH", ALLY);
                    columns.group(ui, group_row, own_idx..own_idx + 1, "YOU", ACCENT);

                    let (header, header_resp) = columns.row(ui, 30.0, Sense::hover());
                    columns.champion_heading(ui, header, "Pick");
                    columns.heading(ui, header, 2, "Score", Align::Max);
                    columns.heading(ui, header, 3, "Base", Align::Center);
                    for (idx, tc) in tcols.iter().enumerate() {
                        let col = FIRST_MATCHUP + idx;
                        let cell = columns.cell(header, col);
                        let tint = if tc.tentative {
                            Color32::from_rgba_unmultiplied(255, 255, 255, 130)
                        } else {
                            Color32::WHITE
                        };
                        table_icon(ui, imgs, &tc.champ, Rect::from_center_size(cell.center(), Vec2::splat(26.0)), tint);
                        ui.interact(cell, header_resp.id.with(col), Sense::hover()).on_hover_text(format!(
                            "{} {} ({}){}",
                            tc.kind,
                            tc.champ.name,
                            short_lane(&tc.lane),
                            if tc.tentative { " · hovering" } else { "" }
                        ));
                    }
                    table_divider(ui, header);

                    for (i, r) in d.recs.iter().enumerate() {
                        let selected = state.selected_champ == Some(r.champ.id);
                        let is_locked = d.locked.as_ref().is_some_and(|l| l.id == r.champ.id);
                        let (row, resp) = columns.row(ui, 48.0, Sense::click());
                        let resp = resp.on_hover_cursor(egui::CursorIcon::PointingHand);
                        if resp.clicked() {
                            state.selected_champ = Some(r.champ.id);
                        }
                        paint_row(ui, row, selected.then_some(ACCENT), resp.hovered(), is_locked);
                        paint_rank(ui, columns.cell(row, 0), i, if selected { ACCENT } else { DIM });
                        paint_table_champion(ui, imgs, columns.cell(row, 1), &r.champ);
                        table_text(ui, columns.champion_name(row), Align::Min, &r.champ.name, strong_font(14.0), TEXT);
                        let color = if r.score >= 0.0 { POS } else { NEG };
                        paint_score(
                            ui,
                            columns.cell(row, 2),
                            &fmt_signed(r.score, 2),
                            color,
                            (r.score.abs() / top_score) as f32,
                        );
                        paint_heat(
                            ui,
                            columns.cell(row, 3),
                            resp.id.with(3),
                            Some(r.base),
                            max_cell,
                            &format!("{}: base {}", r.champ.name, fmt_signed(r.base, 2)),
                        );

                        for (idx, tc) in tcols.iter().enumerate() {
                            let col = FIRST_MATCHUP + idx;
                            let contribution = r.contributions.iter().find(|c| c.kind == tc.kind && c.champ.id == tc.champ.id);
                            let tip = contribution.map(|c| contrib_tip(&r.champ.name, c)).unwrap_or_default();
                            paint_heat(
                                ui,
                                columns.cell(row, col),
                                resp.id.with(col),
                                contribution.map(|c| c.weighted),
                                max_cell,
                                &tip,
                            );
                        }

                        let own: Vec<&ContribView> = r.contributions.iter().filter(|c| c.kind == "you" || c.kind == "mastery").collect();
                        let own_val = (!own.is_empty()).then(|| own.iter().map(|c| c.weighted).sum());
                        let own_tip = own.iter().map(|c| contrib_tip(&r.champ.name, c)).collect::<Vec<_>>().join("\n");
                        paint_heat(ui, columns.cell(row, own_idx), resp.id.with(own_idx), own_val, max_cell, &own_tip);
                    }
                },
            );
        });

        ui.add_space(gap);
        if let Some((idx, r)) = d.recs.iter().enumerate().find(|(_, r)| Some(r.champ.id) == state.selected_champ) {
            ui.allocate_ui_with_layout(Vec2::new(side_w, height), Layout::top_down(Align::Min), |ui| {
                render_pick_detail(ui, r, idx, d.locked.as_ref(), imgs);
            });
        }
    });
}

fn paint_heat(ui: &Ui, cell: Rect, id: egui::Id, val: Option<f64>, max: f64, tip: &str) {
    if let Some(v) = val {
        let (bg, fg) = heat_bg(v, max);
        paint_value(ui, cell, &fmt_signed(v, 1), bg, fg);
        if !tip.is_empty() {
            ui.interact(cell, id, Sense::hover()).on_hover_text(tip);
        }
    } else {
        table_text(ui, cell, Align::Center, "·", FontId::proportional(12.0), DIM);
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

    let panel_size = ui.available_size();
    egui::Frame::new()
        .fill(PANEL)
        .stroke(Stroke::new(1.0, LINE))
        .corner_radius(CornerRadius::same(8))
        .inner_margin(egui::Margin::same(12))
        .show(ui, |ui| {
            ui.set_min_size(panel_size - Vec2::splat(26.0));
            detail_heading(ui, imgs, &r.champ, &kicker, ACCENT, &fmt_signed(r.score, 2), sc_col, "win-rate points");
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
                tile(&mut cols[0], "BEST INTO", best, imgs);
                tile(&mut cols[1], "WATCH OUT FOR", worst, imgs);
            });
            ui.add_space(8.0);

            let columns = TableColumns::pick_detail(ui.available_width());
            let (header, _) = columns.row(ui, 20.0, Sense::hover());
            columns.heading(ui, header, 0, "Why this score", Align::Min);
            columns.heading(ui, header, 1, "Δ win %", Align::Max);
            columns.heading(ui, header, 3, "Adds", Align::Max);
            table_divider(ui, header);

            egui::ScrollArea::vertical().id_salt("pick_det").show(ui, |ui| {
                ui.spacing_mut().item_spacing.y = 2.0;
                let columns = TableColumns::pick_detail(ui.available_width());
                detail_row(ui, &columns, None, "Base strength", &fmt_signed(r.base, 1), r.base, max_add, imgs);
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
                    detail_row(ui, &columns, Some(&c.champ), &label, &raw, c.weighted, max_add, imgs);
                }
            });
        });
}

pub(super) fn detail_heading(
    ui: &mut Ui,
    imgs: &mut ImageCache,
    champ: &ChampRef,
    kicker: &str,
    kicker_color: Color32,
    value: &str,
    value_color: Color32,
    unit: &str,
) {
    let (row, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 58.0), Sense::hover());
    let portrait = Rect::from_min_size(row.left_top(), Vec2::splat(58.0));
    table_icon(ui, imgs, champ, portrait, Color32::WHITE);
    ui.painter().rect_stroke(portrait, CornerRadius::same(6), Stroke::new(1.0, kicker_color), StrokeKind::Inside);

    let metric_width = ui.painter().layout_no_wrap(value.to_owned(), strong_font(29.0), value_color).size().x.max(80.0);
    let name = Rect::from_min_max(
        Pos2::new(portrait.right() + 12.0, row.top()),
        Pos2::new(row.right() - metric_width - 7.0, row.bottom()),
    );
    let metric = Rect::from_min_max(Pos2::new(row.right() - metric_width, row.top()), row.right_bottom());
    table_text(ui, name.with_max_y(row.top() + 28.0), Align::Min, kicker, strong_font(11.0), kicker_color);
    table_text(ui, name.with_min_y(row.top() + 17.0), Align::Min, &champ.name, strong_font(22.0), TEXT);
    table_text(ui, metric.with_max_y(row.top() + 41.0), Align::Max, value, strong_font(29.0), value_color);
    table_text(ui, metric.with_min_y(row.top() + 33.0), Align::Max, unit, FontId::proportional(11.0), MUTED);
}

fn tile(ui: &mut Ui, cap: &str, c: Option<&ContribView>, imgs: &mut ImageCache) {
    egui::Frame::new()
        .fill(RAISED)
        .corner_radius(CornerRadius::same(6))
        .inner_margin(egui::Margin::same(8))
        .show(ui, |ui| {
            let (row, resp) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 35.0), Sense::hover());
            table_text(ui, row.with_max_y(row.top() + 12.0), Align::Min, cap, strong_font(10.0), MUTED);
            let value_row = row.with_min_y(row.top() + 14.0);
            if let Some(c) = c {
                let portrait = Rect::from_center_size(value_row.left_center() + Vec2::new(8.0, 0.0), Vec2::splat(16.0));
                table_icon(ui, imgs, &c.champ, portrait, Color32::WHITE);
                let prefix = if c.kind == "vs" { "vs" } else { "w/" };
                table_text(
                    ui,
                    value_row.with_min_x(portrait.right() + 5.0).with_max_x(row.right() - 37.0),
                    Align::Min,
                    &format!("{prefix} {}", c.champ.name),
                    strong_font(11.0),
                    TEXT,
                );
                table_text(
                    ui,
                    value_row.with_min_x(row.right() - 37.0),
                    Align::Max,
                    &fmt_signed(c.weighted, 1),
                    strong_font(12.0),
                    if c.weighted >= 0.0 { POS } else { NEG },
                );
                resp.on_hover_text(format!("{prefix} {} {}", c.champ.name, fmt_signed(c.weighted, 1)));
            } else {
                table_text(ui, value_row, Align::Min, "None", FontId::proportional(11.0), DIM);
            }
        });
}

fn detail_row(
    ui: &mut Ui,
    columns: &TableColumns,
    champ: Option<&ChampRef>,
    label: &str,
    raw: &str,
    adds: f64,
    max_add: f64,
    imgs: &mut ImageCache,
) {
    let (row, response) = columns.row(ui, 23.0, Sense::hover());
    let mut name = columns.cell(row, 0);
    if let Some(c) = champ {
        let portrait = Rect::from_center_size(name.left_center() + Vec2::new(9.0, 0.0), Vec2::splat(18.0));
        table_icon(ui, imgs, c, portrait, Color32::WHITE);
        name.min.x = portrait.right() + 6.0;
    }
    table_text(ui, name, Align::Min, label, strong_font(12.0), TEXT2);
    table_text(ui, columns.cell(row, 1), Align::Max, raw, FontId::proportional(11.5), MUTED);
    let color = if adds.abs() < 0.05 { DIM } else if adds > 0.0 { POS } else { NEG };
    table_text(ui, columns.cell(row, 3), Align::Max, &fmt_signed(adds, 1), strong_font(12.0), color);

    let bar_cell = columns.cell(row, 2);
    let fraction = (adds.abs() / max_add).clamp(0.0, 1.0) as f32;
    if fraction > 0.02 {
        let bar = Rect::from_min_size(
            bar_cell.left_center() - Vec2::new(0.0, 4.0),
            Vec2::new((bar_cell.width() * fraction).max(2.0), 8.0),
        );
        ui.painter().rect_filled(bar, CornerRadius::same(2), color);
    }
    response.on_hover_text(label);
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

    table_with_note(
        ui,
        "blind_tbl",
        "Enemy laner not picked yet. Score = strength + team fit + blind risk. \"If countered\" is the average of the worst matchups that make up 15% of enemy picks.",
        |ui| {
            const TRACK: usize = 5;
            let columns = TableColumns::blind(ui.available_width());
            let (header, _) = columns.row(ui, 38.0, Sense::hover());
            columns.champion_heading(ui, header, "Pick");
            columns.heading(ui, header, 2, "Score", Align::Max);
            columns.heading(ui, header, 3, "Strength", Align::Center);
            columns.heading(ui, header, 4, "Team fit", Align::Center);
            columns.heading(ui, header, 6, "Blind risk", Align::Center);
            columns.heading(ui, header, 7, "Worst counters", Align::Min);
            columns.heading(ui, header, 8, "Matchup data", Align::Max);
            paint_blind_axis(ui, columns.cell(header, TRACK), lo, hi);
            table_divider(ui, header);

            for (i, r) in list.iter().enumerate() {
                let strength = r.score - r.value - r.synergy;
                let top = i == 0;
                let (row, response) = columns.row(ui, 48.0, Sense::hover());
                paint_row(ui, row, top.then_some(ACCENT), false, false);
                paint_rank(ui, columns.cell(row, 0), i, if top { ACCENT } else { DIM });
                paint_table_champion(ui, imgs, columns.cell(row, 1), &r.champ);
                table_text(ui, columns.champion_name(row), Align::Min, &r.champ.name, strong_font(14.0), TEXT);
                let color = if r.score >= 0.0 { POS } else { NEG };
                paint_score(
                    ui,
                    columns.cell(row, 2),
                    &fmt_signed(r.score, 2),
                    color,
                    (r.score.abs() / top_score) as f32,
                );
                paint_heat(ui, columns.cell(row, 3), response.id.with(3), Some(strength), max_str, "Overall strength in this role");
                paint_heat(ui, columns.cell(row, 4), response.id.with(4), Some(r.synergy), max_fit, "Fit with your team's picks");
                let track = columns.cell(row, TRACK);
                paint_blind_track(ui, track, lo, hi, r);
                ui.interact(track, response.id.with(TRACK), Sense::hover()).on_hover_text(format!(
                    "{}: {} avg matchup, {} if countered",
                    r.champ.name,
                    fmt_signed(r.mean, 1),
                    fmt_signed(r.tail, 1),
                ));
                paint_heat(ui, columns.cell(row, 6), response.id.with(6), Some(r.value), max_risk, "Blind risk contribution");

                let counters = columns.cell(row, 7);
                for (idx, c) in r.counters.iter().take(3).enumerate() {
                    let center = counters.left_center() + Vec2::new(13.0 + idx as f32 * 32.0, 0.0);
                    table_icon(ui, imgs, &c.champ, Rect::from_center_size(center, Vec2::splat(26.0)), Color32::WHITE)
                        .on_hover_text(format!("{} {} ({} games)", c.champ.name, fmt_signed(c.delta, 1), c.games));
                }

                let coverage = columns.cell(row, 8);
                let color = if r.coverage < 50 { ACCENT } else { MUTED };
                let bar = Rect::from_min_size(
                    Pos2::new(coverage.right() - 79.0, coverage.center().y - 2.0),
                    Vec2::new(38.0, 4.0),
                );
                paint_bar(ui, bar, r.coverage as f32 / 100.0, color);
                table_text(ui, coverage, Align::Max, &format!("{}%", r.coverage), FontId::proportional(12.0), color);
            }
        },
    );
}

fn blind_track_x(track: Rect, lo: f64, hi: f64, value: f64) -> f32 {
    const END_PADDING: f32 = 39.0;
    track.left() + END_PADDING + ((value - lo) / (hi - lo)) as f32 * (track.width() - 2.0 * END_PADDING)
}

fn paint_blind_axis(ui: &Ui, track: Rect, lo: f64, hi: f64) {
    let p = ui.painter().with_clip_rect(track);
    let left = blind_track_x(track, lo, hi, lo);
    let right = blind_track_x(track, lo, hi, hi);
    let legend_y = track.top() + 10.0;
    p.circle_filled(Pos2::new(left, legend_y), 3.5, NEG);
    p.text(Pos2::new(left + 8.0, legend_y), Align2::LEFT_CENTER, "if countered", FontId::proportional(11.0), MUTED);
    p.circle_filled(Pos2::new(right, legend_y), 3.5, TEXT);
    p.text(Pos2::new(right - 8.0, legend_y), Align2::RIGHT_CENTER, "average matchup", FontId::proportional(11.0), MUTED);

    let step = ((hi - lo) / (right - left) as f64 * 26.0).ceil().max(1.0) as i32;
    let low_tick = (lo / f64::from(step)).ceil() as i32;
    let high_tick = (hi / f64::from(step)).floor() as i32;
    for idx in low_tick..=high_tick {
        let tick = idx * step;
        let x = blind_track_x(track, lo, hi, f64::from(tick));
        let color = if tick == 0 { TEXT2 } else { MUTED };
        p.text(Pos2::new(x, track.bottom() - 12.0), Align2::CENTER_CENTER, tick.to_string(), FontId::proportional(11.0), color);
        p.line_segment(
            [Pos2::new(x, track.bottom() - 4.0), Pos2::new(x, track.bottom() - 1.0)],
            Stroke::new(1.0, if tick == 0 { MUTED } else { LINE2 }),
        );
    }
}

fn paint_blind_track(ui: &Ui, track: Rect, lo: f64, hi: f64, r: &BlindView) {
    let x_of = |value| blind_track_x(track, lo, hi, value);
    let p = ui.painter().with_clip_rect(track);
    let cy = track.center().y;
    let at_center = |x| Pos2::new(x, cy);
    p.line_segment([at_center(x_of(lo)), at_center(x_of(hi))], Stroke::new(1.0, LINE2));
    let zero = x_of(0.0);
    p.line_segment(
        [Pos2::new(zero, cy - 11.0), Pos2::new(zero, cy + 11.0)],
        Stroke::new(1.0, MUTED),
    );
    let tail = x_of(r.tail);
    let mean = x_of(r.mean);
    p.line_segment(
        [at_center(tail), at_center(mean)],
        Stroke::new(5.0, Color32::from_rgb(0x6E, 0x3D, 0x38)),
    );
    p.circle_filled(at_center(tail), 5.0, NEG);
    p.circle_filled(at_center(mean), 5.0, TEXT);
    p.text(at_center(tail - 8.0), Align2::RIGHT_CENTER, fmt_signed(r.tail, 1), strong_font(12.0), NEG);
    p.text(at_center(mean + 8.0), Align2::LEFT_CENTER, fmt_signed(r.mean, 1), FontId::proportional(12.0), TEXT2);
}
