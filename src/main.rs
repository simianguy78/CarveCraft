mod app;
mod cam;
mod export;
mod feeds;
mod geom;
mod icons;
mod model;
mod panels;
mod panels_right;
mod render3d;
mod start;
mod theme;
mod tooldb;
mod toolpath;
mod views;

use std::path::PathBuf;

fn main() -> eframe::Result<()> {
    let args: Vec<String> = std::env::args().collect();
    let mut screenshot: Option<PathBuf> = None;
    let mut open: Option<PathBuf> = None;
    let mut i = 1;
    while i < args.len() {
        if args[i] == "--screenshot" && i + 1 < args.len() {
            screenshot = Some(PathBuf::from(&args[i + 1]));
            i += 2;
        } else {
            open = Some(PathBuf::from(&args[i]));
            i += 1;
        }
    }
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("CarveCraft Studio")
            .with_inner_size([1600.0, 960.0])
            .with_min_inner_size([1100.0, 700.0]),
        ..Default::default()
    };
    eframe::run_native(
        "CarveCraft Studio",
        options,
        Box::new(move |cc| Ok(Box::new(app::App::new(cc, screenshot, open)))),
    )
}
