//! Phase 0 spike window, now fed by the document thread: shows page 1.

use std::path::PathBuf;

use eframe::egui;
use marg_core::config::Config;
use marg_pdf::{DocThread, RenderKey, Request, Response, Scale};

mod paths;
mod startup;

const GIT_HASH: &str = env!("MARG_GIT_HASH");

/// Render scale in pixels per PDF point for the spike (fixed, ~144 dpi).
const SPIKE_SCALE: f32 = 2.0;

struct SpikeApp {
    config: Config,
    file_name: String,
    doc: DocThread,
    texture: Option<egui::TextureHandle>,
    status: String,
}

impl SpikeApp {
    fn new(path: PathBuf, config: Config, message: Option<String>, ctx: &egui::Context) -> Self {
        let ctx = ctx.clone();
        let doc = DocThread::spawn(move || ctx.request_repaint());
        let file_name = path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        doc.send(Request::Open(path));
        Self {
            config,
            file_name,
            doc,
            texture: None,
            status: message.unwrap_or_else(|| "opening…".into()),
        }
    }
}

impl eframe::App for SpikeApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        while let Some(resp) = self.doc.try_recv() {
            match resp {
                Response::Opened {
                    result: Ok(info), ..
                } => {
                    self.status = format!("1/{}", info.page_sizes.len());
                    self.doc.send(Request::Render(vec![RenderKey {
                        page: 0,
                        scale: Scale::from_f32(SPIKE_SCALE),
                        tile: None,
                    }]));
                }
                Response::Rendered {
                    result: Ok(p),
                    took,
                    ..
                } => {
                    let size = [p.width as usize, p.height as usize];
                    let image = egui::ColorImage::from_rgba_premultiplied(size, &p.rgba);
                    self.texture = Some(ui.ctx().load_texture("page-1", image, Default::default()));
                    self.status += &format!("  render {} ms", took.as_millis());
                }
                Response::Opened { result: Err(e), .. }
                | Response::Rendered { result: Err(e), .. } => {
                    log::error!("{e}");
                    self.status = format!("error: {e}");
                }
            }
        }
        if ui.input(|i| i.key_pressed(egui::Key::Q)) {
            ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close);
        }

        egui::Panel::bottom("status")
            .exact_size(self.config.ui.status_bar_px)
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
    let paths = paths::Paths::resolve();
    startup::init_logging(paths.as_ref());
    let (config, config_error) = startup::load_config(paths.as_ref());
    if let Some(e) = &config_error {
        log::error!("{e}");
    }
    let Some(path) = std::env::args_os().nth(1).map(PathBuf::from) else {
        eprintln!("usage: marg <file.pdf>");
        std::process::exit(2);
    };
    eframe::run_native(
        "marg",
        eframe::NativeOptions::default(),
        Box::new(|cc| {
            Ok(Box::new(SpikeApp::new(
                path,
                config,
                config_error,
                &cc.egui_ctx,
            )))
        }),
    )
}
