use egui::{Color32, ColorImage, CornerRadius, TextureHandle, TextureOptions, Ui, Vec2};
use reqwest::Client;
use sha1::{Digest, Sha1};
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use tokio::sync::mpsc;

use crate::domain::config::cache_dir;

struct LoadedTex {
    url: String,
    img: Option<ColorImage>,
}

/// Async GPU texture cache for Data Dragon champion/item/rune PNGs.
/// Decodes PNGs on background threads, caches bytes on disk, and uploads to GPU once!
pub struct ImageCache {
    textures: HashMap<String, Option<TextureHandle>>,
    requested: HashSet<String>,
    tx: mpsc::UnboundedSender<LoadedTex>,
    rx: mpsc::UnboundedReceiver<LoadedTex>,
    http: Client,
    disk_dir: PathBuf,
    sem: Arc<tokio::sync::Semaphore>,
    repaint: Arc<Mutex<Option<egui::Context>>>,
}

impl ImageCache {
    pub fn new() -> Self {
        let (tx, rx) = mpsc::unbounded_channel();
        let disk_dir = cache_dir().join("images");
        let _ = fs::create_dir_all(&disk_dir);
        Self {
            textures: HashMap::new(),
            requested: HashSet::new(),
            tx,
            rx,
            http: Client::builder()
                .timeout(std::time::Duration::from_secs(15))
                .build()
                .expect("image reqwest"),
            disk_dir,
            sem: Arc::new(tokio::sync::Semaphore::new(8)),
            repaint: Arc::new(Mutex::new(None)),
        }
    }

    pub fn bind_ctx(&self, ctx: &egui::Context) {
        let mut g = self.repaint.lock().unwrap();
        if g.is_none() {
            *g = Some(ctx.clone());
        }
    }

    pub fn repaint_all(ctx: &egui::Context) {
        ctx.request_repaint_of(egui::ViewportId::ROOT);
        ctx.request_repaint_of(egui::ViewportId::from_hash_of("ingame_overlay"));
    }

    pub fn poll(&mut self, ctx: &egui::Context) {
        self.bind_ctx(ctx);
        while let Ok(item) = self.rx.try_recv() {
            let handle = item.img.map(|im| {
                ctx.load_texture(&item.url, im, TextureOptions::LINEAR)
            });
            self.textures.insert(item.url, handle);
        }
    }

    fn disk_path(&self, url: &str) -> PathBuf {
        let mut h = Sha1::new();
        h.update(url.as_bytes());
        let d = h.finalize();
        let hex: String = d.iter().take(10).map(|b| format!("{b:02x}")).collect();
        self.disk_dir.join(format!("{hex}.img"))
    }

    pub fn get(&mut self, url: &str) -> Option<&TextureHandle> {
        if url.is_empty() {
            return None;
        }
        if let Some(opt) = self.textures.get(url) {
            return opt.as_ref();
        }
        if !self.requested.contains(url) {
            self.requested.insert(url.to_string());
            let url_s = url.to_string();
            let path = self.disk_path(url);
            let http = self.http.clone();
            let tx = self.tx.clone();
            let sem = Arc::clone(&self.sem);
            let ctx_hold = Arc::clone(&self.repaint);
            tokio::spawn(async move {
                let _permit = sem.acquire().await;
                let bytes = if path.exists() {
                    fs::read(&path).ok()
                } else {
                    None
                };
                let bytes = match bytes {
                    Some(b) => Some(b),
                    None => {
                        if let Ok(r) = http.get(&url_s).send().await
                            && r.status().is_success()
                            && let Ok(b) = r.bytes().await
                        {
                            let vec = b.to_vec();
                            let _ = fs::write(&path, &vec);
                            Some(vec)
                        } else {
                            None
                        }
                    }
                };
                let img = bytes.and_then(|b| {
                    let dyn_img = image::load_from_memory(&b).ok()?;
                    let rgba = dyn_img.to_rgba8();
                    let size = [rgba.width() as usize, rgba.height() as usize];
                    Some(ColorImage::from_rgba_unmultiplied(size, rgba.as_raw()))
                });
                let _ = tx.send(LoadedTex { url: url_s, img });
                if let Some(ctx) = ctx_hold.lock().unwrap().as_ref() {
                    Self::repaint_all(ctx);
                }
            });
        }
        None
    }

    pub fn paint_icon(
        &mut self,
        ui: &mut Ui,
        name: &str,
        url: &str,
        size: Vec2,
        rounding: f32,
        tint: Color32,
    ) -> egui::Response {
        let (rect, resp) = ui.allocate_exact_size(size, egui::Sense::hover());
        if ui.is_rect_visible(rect) {
            let p = ui.painter();
            let cr = CornerRadius::same(rounding.round().clamp(0.0, 255.0) as u8);
            p.rect_filled(rect, cr, Color32::from_rgb(0x1B, 0x22, 0x2B));
            if let Some(tex) = self.get(url) {
                p.image(
                    tex.id(),
                    rect,
                    egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
                    tint,
                );
            } else {
                let abbr: String = name
                    .chars()
                    .filter(|c| c.is_ascii_alphanumeric())
                    .take(2)
                    .collect();
                p.text(
                    rect.center(),
                    egui::Align2::CENTER_CENTER,
                    if abbr.is_empty() { "?" } else { &abbr },
                    egui::FontId::proportional((size.y * 0.42).max(9.0)),
                    Color32::from_rgb(0xE6, 0xB8, 0x5C),
                );
            }
        }
        resp
    }
}
