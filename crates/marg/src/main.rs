//! Phase 0 spike: render page 1 of a PDF on a separate thread and show it.

use std::path::PathBuf;
use std::sync::mpsc;
use std::time::{Duration, Instant};

use eframe::egui;
use marg_pdf::RenderedPage;

const GIT_HASH: &str = env!("MARG_GIT_HASH");

/// Render scale in pixels per PDF point for the spike (fixed, ~144 dpi).
const SPIKE_SCALE: f32 = 2.0;

struct Rendered {
    page: Result<RenderedPage, String>,
    took: Duration,
}

struct SpikeApp {
    file_name: String,
    rx: mpsc::Receiver<Rendered>,
    texture: Option<egui::TextureHandle>,
    status: String,
}

impl SpikeApp {
    fn new(path: PathBuf, ctx: &egui::Context) -> Self {
        let (tx, rx) = mpsc::channel();
        let ctx = ctx.clone();
        let file_name = path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        std::thread::spawn(move || {
            let start = Instant::now();
            let page = marg_pdf::render_page(&path, 0, SPIKE_SCALE);
            let _ = tx.send(Rendered {
                page,
                took: start.elapsed(),
            });
            ctx.request_repaint();
        });
        Self {
            file_name,
            rx,
            texture: None,
            status: "rendering…".into(),
        }
    }
}

impl eframe::App for SpikeApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        if let Ok(r) = self.rx.try_recv() {
            match r.page {
                Ok(p) => {
                    let image = egui::ColorImage::from_rgb([p.width, p.height], &p.rgb);
                    self.texture = Some(ui.ctx().load_texture("page-1", image, Default::default()));
                    self.status = format!("1/{}  render {} ms", p.page_count, r.took.as_millis());
                }
                Err(e) => {
                    log::error!("{e}");
                    self.status = format!("error: {e}");
                }
            }
        }
        if ui.input(|i| i.key_pressed(egui::Key::Q)) {
            ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close);
        }

        egui::Panel::bottom("status")
            .exact_size(22.0)
            .frame(egui::Frame::new().fill(egui::Color32::from_rgb(0x1E, 0x1E, 0x1E)))
            .show(ui, |ui| {
                ui.horizontal_centered(|ui| {
                    ui.monospace(format!(
                        "NORMAL  {}  {}  marg {} ({GIT_HASH})",
                        self.file_name,
                        self.status,
                        marg_core::VERSION
                    ));
                });
            });
        egui::CentralPanel::default()
            .frame(egui::Frame::new().fill(egui::Color32::from_rgb(0x2B, 0x2B, 0x2B)))
            .show(ui, |ui| {
                if let Some(tex) = &self.texture {
                    ui.centered_and_justified(|ui| {
                        ui.add(egui::Image::new(tex).shrink_to_fit());
                    });
                }
            });
    }
}

fn main() -> eframe::Result {
    let Some(path) = std::env::args_os().nth(1).map(PathBuf::from) else {
        eprintln!("usage: marg <file.pdf>");
        std::process::exit(2);
    };
    eframe::run_native(
        "marg",
        eframe::NativeOptions::default(),
        Box::new(|cc| Ok(Box::new(SpikeApp::new(path, &cc.egui_ctx)))),
    )
}
