use egui::{Align, Color32, Layout, RichText, Ui};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use crate::domain::config::{SettingsValues, load_config_strict, save_settings_form};
use crate::engine::controller::Controller;

use crate::ui::theme::{GOLD, MUTED, NEG, POS, TEXT};
use crate::win32;

pub struct SettingsUiState {
    pub open: bool,
    pub path: PathBuf,
    pub min_pick_rate: String,
    pub comfort_only: bool,
    pub comfort_weight: String,
    pub lockfile: String,
    pub errors: HashMap<&'static str, String>,
    pub message: Option<(String, bool)>,
    pub version_text: String,
}

impl SettingsUiState {
    pub fn new(path: PathBuf, version_text: String) -> Self {
        let mut st = Self {
            open: false,
            path,
            min_pick_rate: "1.0".into(),
            comfort_only: false,
            comfort_weight: "1.0".into(),
            lockfile: String::new(),
            errors: HashMap::new(),
            message: None,
            version_text,
        };
        st.reload_from_disk();
        st
    }

    pub fn reload_from_disk(&mut self) {
        let vals = load_config_strict(Some(&self.path))
            .map(|c| SettingsValues::from_config(&c))
            .unwrap_or_else(|_| SettingsValues::defaults());
        self.apply_values(&vals);
        self.errors.clear();
        self.message = None;
    }

    pub fn apply_values(&mut self, v: &SettingsValues) {
        self.min_pick_rate = format!("{}", v.min_pick_rate);
        self.comfort_only = v.comfort_only;
        self.comfort_weight = format!("{}", v.comfort_weight);
        self.lockfile = v.lockfile.clone();
    }
}

pub fn render_settings(ui: &mut Ui, st: &mut SettingsUiState, ctl: &Arc<Controller>) {
    ui.label(RichText::new("Settings").size(16.0).strong().color(GOLD));
    ui.label(RichText::new(&st.version_text).size(10.5).color(MUTED));
    ui.add_space(8.0);
    ui.separator();
    ui.add_space(8.0);

    ui.label(RichText::new("Minimum pick rate (%)").size(12.0).color(TEXT));
    ui.text_edit_singleline(&mut st.min_pick_rate);
    if let Some(err) = st.errors.get("min_pick_rate") {
        ui.label(RichText::new(err).size(10.5).color(NEG));
    }
    ui.add_space(8.0);

    ui.checkbox(&mut st.comfort_only, "Only recommend champions you've played or have mastery on");
    ui.add_space(8.0);

    ui.label(RichText::new("Record and mastery weight (0 = off, up to 2)").size(12.0).color(TEXT));
    ui.text_edit_singleline(&mut st.comfort_weight);
    if let Some(err) = st.errors.get("comfort_weight") {
        ui.label(RichText::new(err).size(10.5).color(NEG));
    }
    ui.add_space(8.0);

    ui.label(RichText::new("League lockfile path").size(12.0).color(TEXT));
    ui.text_edit_singleline(&mut st.lockfile);
    if let Some(err) = st.errors.get("lockfile") {
        ui.label(RichText::new(err).size(10.5).color(NEG));
    }
    ui.add_space(12.0);

    if let Some((msg, ok)) = &st.message {
        ui.label(RichText::new(msg).size(11.0).color(if *ok { POS } else { NEG }));
        ui.add_space(6.0);
    }

    ui.horizontal(|ui| {
        if ui.button("Save").clicked() {
            let vals = SettingsValues {
                min_pick_rate: st.min_pick_rate.trim().parse::<f64>().unwrap_or(f64::NAN),
                comfort_only: st.comfort_only,
                comfort_weight: st.comfort_weight.trim().parse::<f64>().unwrap_or(f64::NAN),
                lockfile: st.lockfile.clone(),
            };
            match save_settings_form(&vals, &st.path) {
                Ok(errs) if errs.is_empty() => {
                    st.errors.clear();
                    st.message = Some(("Saved.".into(), true));
                    if let Ok(cfg) = load_config_strict(Some(&st.path)) {
                        let ctl_c = Arc::clone(ctl);
                        tokio::spawn(async move {
                            ctl_c.reload_config(cfg).await;
                        });
                    }
                }
                Ok(errs) => {
                    st.errors = errs;
                    st.message = None;
                }
                Err(e) => {
                    st.message = Some((e, false));
                }
            }
        }
        if ui.button("Reset defaults").clicked() {
            st.apply_values(&SettingsValues::defaults());
            st.errors.clear();
        }
        if ui.button("Open config.toml").clicked() {
            win32::open_file(&st.path);
        }
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            if ui.button("Close").clicked() {
                st.open = false;
            }
        });
    });
    let _ = Color32::TRANSPARENT;
}
