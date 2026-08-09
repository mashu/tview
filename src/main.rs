#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;
mod data;
mod export;
mod series;
mod theme;
mod ui;

use eframe::egui;
use std::path::PathBuf;

fn main() -> eframe::Result<()> {
    // Files passed on the command line, e.g.
    //   tview run_a.csv run_b.tsv
    //   cargo run --release -- run_a.csv run_b.tsv
    // Non-existent paths are dropped here; load errors (if any) surface in the UI status line.
    let initial: Vec<PathBuf> = std::env::args_os()
        .skip(1)
        .map(PathBuf::from)
        .filter(|p| p.is_file())
        .collect();

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1360.0, 860.0])
            .with_min_inner_size([800.0, 520.0])
            .with_title("tview"),
        ..Default::default()
    };
    eframe::run_native(
        "tview",
        options,
        Box::new(move |cc| {
            theme::apply(&cc.egui_ctx);
            Box::new(app::TsvPlotApp::new(initial))
        }),
    )
}
