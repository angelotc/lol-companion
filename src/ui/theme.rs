use egui::{Color32, CornerRadius, Stroke, Style, Visuals};

pub const BG: Color32 = Color32::from_rgb(0x0D, 0x10, 0x15);
pub const BG2: Color32 = Color32::from_rgb(0x11, 0x15, 0x1B);
pub const PANEL: Color32 = Color32::from_rgb(0x15, 0x1A, 0x21);
pub const RAISED: Color32 = Color32::from_rgb(0x1B, 0x22, 0x2B);
pub const LINE: Color32 = Color32::from_rgb(0x26, 0x2E, 0x39);
pub const LINE2: Color32 = Color32::from_rgb(0x3A, 0x43, 0x50);
pub const TEXT: Color32 = Color32::from_rgb(0xE8, 0xEC, 0xF1);
pub const TEXT2: Color32 = Color32::from_rgb(0xC9, 0xD1, 0xDB);
pub const MUTED: Color32 = Color32::from_rgb(0x9A, 0xA5, 0xB4);
pub const DIM: Color32 = Color32::from_rgb(0x7C, 0x87, 0x96);
pub const GOLD: Color32 = Color32::from_rgb(0xE6, 0xB8, 0x5C);
pub const ALLY: Color32 = Color32::from_rgb(0x5A, 0xA9, 0xFF);
pub const ALLY_TEXT: Color32 = Color32::from_rgb(0x6F, 0xB4, 0xFF);
pub const ENEMY: Color32 = Color32::from_rgb(0xFF, 0x6B, 0x6B);
pub const ENEMY_TEXT: Color32 = Color32::from_rgb(0xFF, 0x7B, 0x7B);
pub const POS: Color32 = Color32::from_rgb(0x3D, 0xDC, 0x97);
pub const NEG: Color32 = Color32::from_rgb(0xFF, 0x8A, 0x70);
pub const AD: Color32 = Color32::from_rgb(0xF2, 0xA2, 0x5C);
pub const AP: Color32 = Color32::from_rgb(0x7F, 0xA8, 0xFF);

pub fn apply_theme(ctx: &egui::Context) {
    let mut style = Style::default();
    let mut v = Visuals::dark();
    v.panel_fill = BG;
    v.window_fill = PANEL;
    v.extreme_bg_color = BG2;
    v.faint_bg_color = RAISED;
    v.widgets.noninteractive.bg_fill = PANEL;
    v.widgets.noninteractive.bg_stroke = Stroke::new(1.0, LINE);
    v.widgets.noninteractive.fg_stroke = Stroke::new(1.0, TEXT);
    v.widgets.inactive.bg_fill = RAISED;
    v.widgets.inactive.bg_stroke = Stroke::new(1.0, LINE2);
    v.widgets.inactive.fg_stroke = Stroke::new(1.0, TEXT2);
    v.widgets.hovered.bg_fill = Color32::from_rgb(0x23, 0x2B, 0x36);
    v.widgets.hovered.bg_stroke = Stroke::new(1.0, GOLD);
    v.widgets.hovered.fg_stroke = Stroke::new(1.0, TEXT);
    v.widgets.active.bg_fill = GOLD;
    v.widgets.active.fg_stroke = Stroke::new(1.0, BG);
    v.selection.bg_fill = Color32::from_rgba_unmultiplied(0xE6, 0xB8, 0x5C, 48);
    v.selection.stroke = Stroke::new(1.0, GOLD);
    v.window_corner_radius = CornerRadius::same(6);
    style.visuals = v;
    ctx.set_style(style);
}

pub fn short_lane(l: &str) -> &'static str {
    match l.to_ascii_lowercase().as_str() {
        "top" => "TOP",
        "jungle" | "jg" => "JG",
        "middle" | "mid" => "MID",
        "bottom" | "bot" | "adc" => "BOT",
        "support" | "utility" | "sup" => "SUP",
        _ => "?",
    }
}

pub fn fmt_signed(v: f64, decimals: usize) -> String {
    if !v.is_finite() {
        return "-".into();
    }
    let abs_fmt = format!("{:.decimals$}", v.abs());
    let zero = abs_fmt.parse::<f64>().unwrap_or(0.0) == 0.0;
    if zero {
        abs_fmt
    } else if v > 0.0 {
        format!("+{abs_fmt}")
    } else {
        format!("-{abs_fmt}")
    }
}

pub fn heat_bg(v: f64, max: f64) -> (Color32, Color32) {
    if v.abs() < 0.05 {
        return (Color32::TRANSPARENT, DIM);
    }
    let alpha = (0.12 + (v.abs() / max.max(0.5)) * 0.50).min(0.62);
    let a_u8 = (alpha * 255.0).round() as u8;
    let base = if v > 0.0 { POS } else { NEG };
    let bg = Color32::from_rgba_unmultiplied(base.r(), base.g(), base.b(), a_u8);
    let fg = if alpha > 0.30 { TEXT } else { base };
    (bg, fg)
}

pub fn tier_color(tier: &str) -> Color32 {
    match tier {
        "iron" => Color32::from_rgb(0x8A, 0x80, 0x77),
        "bronze" => Color32::from_rgb(0xB0, 0x77, 0x4A),
        "silver" => Color32::from_rgb(0xA9, 0xB4, 0xBD),
        "gold" => Color32::from_rgb(0xE0, 0xB8, 0x4A),
        "platinum" => Color32::from_rgb(0x4F, 0xC4, 0xB0),
        "emerald" => Color32::from_rgb(0x2F, 0xBF, 0x71),
        "diamond" => Color32::from_rgb(0x6A, 0xA6, 0xFF),
        "master" => Color32::from_rgb(0xC7, 0x7D, 0xFF),
        "grandmaster" => Color32::from_rgb(0xFF, 0x6B, 0x6B),
        "challenger" => Color32::from_rgb(0x7F, 0xE3, 0xFF),
        _ => DIM,
    }
}
