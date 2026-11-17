use egui::{Color32, CornerRadius, Stroke, Style, Visuals};

pub const BG: Color32 = Color32::from_rgb(0x18, 0x14, 0x20);
pub const BG2: Color32 = Color32::from_rgb(0x21, 0x1A, 0x2E);
pub const PANEL: Color32 = Color32::from_rgb(0x27, 0x20, 0x32);
pub const RAISED: Color32 = Color32::from_rgb(0x30, 0x27, 0x3D);
pub const LINE: Color32 = Color32::from_rgb(0x3E, 0x33, 0x4D);
pub const LINE2: Color32 = Color32::from_rgb(0x55, 0x47, 0x64);
pub const TEXT: Color32 = Color32::from_rgb(0xF3, 0xEF, 0xF9);
pub const TEXT2: Color32 = Color32::from_rgb(0xD7, 0xD0, 0xE4);
pub const MUTED: Color32 = Color32::from_rgb(0xAB, 0xA0, 0xBC);
pub const DIM: Color32 = Color32::from_rgb(0x91, 0x84, 0x9F);
pub const ACCENT: Color32 = Color32::from_rgb(0xC4, 0xB5, 0xFD);
pub const CORAL: Color32 = Color32::from_rgb(0xFF, 0x8A, 0x72);
pub const ALLY: Color32 = Color32::from_rgb(0x5A, 0xA9, 0xFF);
pub const ALLY_TEXT: Color32 = Color32::from_rgb(0x6F, 0xB4, 0xFF);
pub const ENEMY: Color32 = Color32::from_rgb(0xFF, 0x6B, 0x6B);
pub const ENEMY_TEXT: Color32 = Color32::from_rgb(0xFF, 0x7B, 0x7B);
pub const POS: Color32 = Color32::from_rgb(0x3D, 0xDC, 0x97);
pub const NEG: Color32 = Color32::from_rgb(0xFF, 0x8A, 0x70);
pub const AD: Color32 = Color32::from_rgb(0xF2, 0xA2, 0x5C);
pub const AP: Color32 = Color32::from_rgb(0x7F, 0xA8, 0xFF);

const STRONG_FAMILY: &str = "ui-semibold";

pub fn strong(text: impl Into<String>) -> egui::RichText {
    egui::RichText::new(text)
        .family(egui::FontFamily::Name(STRONG_FAMILY.into()))
        .strong()
}

pub fn strong_font(size: f32) -> egui::FontId {
    egui::FontId::new(size, egui::FontFamily::Name(STRONG_FAMILY.into()))
}

fn apply_fonts(ctx: &egui::Context) {
    let mut fonts = egui::FontDefinitions::default();
    #[cfg(target_os = "windows")]
    {
        let font_dir = std::path::PathBuf::from(
            std::env::var_os("WINDIR").unwrap_or_else(|| r"C:\Windows".into()),
        ).join("Fonts");
        for (name, file) in [("ui-regular", "segoeui.ttf"), (STRONG_FAMILY, "seguisb.ttf")] {
            if let Ok(bytes) = std::fs::read(font_dir.join(file)) {
                fonts.font_data.insert(name.into(), egui::FontData::from_owned(bytes).into());
            }
        }
        if fonts.font_data.contains_key("ui-regular") {
            fonts.families.entry(egui::FontFamily::Proportional).or_default().insert(0, "ui-regular".into());
        }
    }
    let mut emphasis = fonts.families[&egui::FontFamily::Proportional].clone();
    if fonts.font_data.contains_key(STRONG_FAMILY) {
        emphasis.insert(0, STRONG_FAMILY.into());
    }
    fonts.families.insert(egui::FontFamily::Name(STRONG_FAMILY.into()), emphasis);
    ctx.set_fonts(fonts);
}

pub fn apply_theme(ctx: &egui::Context) {
    apply_fonts(ctx);
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
    v.widgets.hovered.bg_fill = Color32::from_rgb(0x3A, 0x2E, 0x4B);
    v.widgets.hovered.bg_stroke = Stroke::new(1.0, ACCENT);
    v.widgets.hovered.fg_stroke = Stroke::new(1.0, TEXT);
    v.widgets.active.bg_fill = ACCENT;
    v.widgets.active.fg_stroke = Stroke::new(1.0, BG);
    v.selection.bg_fill = Color32::from_rgba_unmultiplied(ACCENT.r(), ACCENT.g(), ACCENT.b(), 48);
    v.selection.stroke = Stroke::new(1.0, ACCENT);
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

pub fn left_label(ui: &mut egui::Ui, w: f32, h: f32, text: egui::RichText) {
    ui.allocate_ui_with_layout(
        egui::Vec2::new(w, h),
        egui::Layout::left_to_right(egui::Align::Center),
        |ui| {
            ui.add(egui::Label::new(text).truncate());
        },
    );
}
