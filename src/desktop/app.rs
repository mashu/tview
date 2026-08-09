use crate::backend::{Backend, Command, Event, FileView, PlotOptions, SeriesDto};
use crate::desktop::theme::{self, ACCENT, BG_DEEP, BORDER};
use crate::desktop::ui::{self, DisplaySettings};
use crate::series::Series;
use eframe::egui;
use egui::{Context, Frame, Margin, Stroke};
use std::path::PathBuf;

/// egui shell: never loads files or exports on the UI thread.
pub struct DesktopApp {
    backend: Backend,
    /// Local mirrors for widgets; flushed to backend on change.
    options: PlotOptions,
    live_reload: bool,
    status: String,
    busy: bool,
    files: Vec<FileView>,
    series: Vec<Series>,
    x_opts: Vec<String>,
    last_generation: u64,
}

impl DesktopApp {
    pub fn new(backend: Backend, initial: Vec<PathBuf>) -> Self {
        if !initial.is_empty() {
            backend.send(Command::LoadPaths(initial));
        }
        let snap = backend.snapshot();
        Self {
            backend,
            options: snap.options,
            live_reload: snap.live_reload,
            status: snap.status,
            busy: snap.busy,
            files: snap.files,
            series: snap.series.iter().map(SeriesDto::to_series).collect(),
            x_opts: snap.x_axis_options,
            last_generation: snap.generation,
        }
    }

    fn sync_from_backend(&mut self) {
        for ev in self.backend.poll_events() {
            match ev {
                Event::Status(s) => self.status = s,
                Event::ExportSaved(p) => self.status = format!("Saved {}", p.display()),
                Event::Failed(e) => self.status = e,
                Event::PngReady { .. } => {}
            }
        }

        let snap = self.backend.snapshot();
        if snap.generation == self.last_generation {
            self.busy = snap.busy;
            return;
        }
        self.last_generation = snap.generation;
        self.busy = snap.busy;
        self.status = snap.status;
        self.live_reload = snap.live_reload;
        // Keep local widget values unless backend changed x_col via auto-pick on load.
        self.options = snap.options;
        self.files = snap.files;
        self.x_opts = snap.x_axis_options;
        self.series = snap.series.iter().map(SeriesDto::to_series).collect();
    }

    fn pick_files(&self) {
        if let Some(paths) = rfd::FileDialog::new()
            .add_filter("tabular", &["csv", "tsv", "txt", "tab"])
            .pick_files()
        {
            self.backend.send(Command::LoadPaths(paths));
        }
    }

    fn export_plot(&self) {
        if self.series.is_empty() {
            // Status will update after a no-op export fails on backend; surface early.
            return;
        }
        if let Some(path) = rfd::FileDialog::new()
            .add_filter("PNG", &["png"])
            .set_file_name("plot.png")
            .save_file()
        {
            self.backend.send(Command::ExportPng {
                path,
                size: (1600, 1000),
            });
        }
    }
}

impl eframe::App for DesktopApp {
    fn update(&mut self, ctx: &Context, _frame: &mut eframe::Frame) {
        self.sync_from_backend();
        // Keep animating while backend is busy or live-watching.
        if self.busy || (self.live_reload && !self.files.is_empty()) {
            ctx.request_repaint_after(std::time::Duration::from_millis(100));
        }

        let hovering = ctx.input(|i| !i.raw.hovered_files.is_empty());
        if hovering {
            let painter = ctx.layer_painter(egui::LayerId::new(
                egui::Order::Foreground,
                egui::Id::new("drop_overlay"),
            ));
            let screen = ctx.screen_rect();
            painter.rect_filled(
                screen,
                0.0,
                egui::Color32::from_rgba_unmultiplied(12, 14, 18, 180),
            );
            painter.rect_stroke(screen.shrink(18.0), 12.0, Stroke::new(2.0_f32, ACCENT));
            painter.text(
                screen.center(),
                egui::Align2::CENTER_CENTER,
                "Drop CSV / TSV to add",
                egui::FontId::proportional(28.0),
                ACCENT,
            );
        }

        let dropped: Vec<PathBuf> = ctx.input(|i| {
            i.raw
                .dropped_files
                .iter()
                .filter_map(|f| f.path.clone())
                .collect()
        });
        if !dropped.is_empty() {
            self.backend.send(Command::LoadPaths(dropped));
        }

        let mut do_add = false;
        let mut do_export = false;

        egui::TopBottomPanel::top("toolbar")
            .frame(
                Frame::none()
                    .fill(theme::BG_PANEL)
                    .stroke(Stroke::new(1.0_f32, BORDER))
                    .inner_margin(Margin::symmetric(16.0, 10.0)),
            )
            .show(ctx, |ui| {
                let ev = ui::draw_toolbar(ui, self.busy);
                do_add |= ev.add_files;
                do_export |= ev.export;
            });

        egui::TopBottomPanel::bottom("status")
            .exact_height(28.0)
            .frame(
                Frame::none()
                    .fill(theme::BG_PANEL)
                    .stroke(Stroke::new(1.0_f32, BORDER))
                    .inner_margin(Margin::symmetric(14.0, 4.0)),
            )
            .show(ctx, |ui| {
                ui::draw_status_bar(
                    ui,
                    &self.status,
                    self.series.len(),
                    self.files.len(),
                    self.live_reload,
                );
            });

        egui::SidePanel::left("controls")
            .resizable(true)
            .default_width(300.0)
            .min_width(240.0)
            .frame(
                Frame::none()
                    .fill(theme::BG_PANEL)
                    .stroke(Stroke::new(1.0_f32, BORDER))
                    .inner_margin(Margin::symmetric(14.0, 12.0)),
            )
            .show(ctx, |ui| {
                let ev = ui::draw_controls(
                    ui,
                    &self.files,
                    DisplaySettings {
                        options: &mut self.options,
                        live_reload: &mut self.live_reload,
                    },
                    &self.x_opts,
                );
                do_add |= ev.add_files;
                do_export |= ev.export;
                if ev.options_changed {
                    self.backend.send(Command::SetOptions(self.options.clone()));
                }
                if ev.live_changed {
                    self.backend.send(Command::SetLiveReload(self.live_reload));
                }
                if let Some(i) = ev.remove {
                    self.backend.send(Command::RemoveFile(i));
                }
                if let Some((i, visible)) = ev.set_visible {
                    self.backend.send(Command::SetVisible { index: i, visible });
                }
                if let Some((file, column, selected)) = ev.set_selected {
                    self.backend.send(Command::SetColumnSelected {
                        file,
                        column,
                        selected,
                    });
                }
            });

        egui::CentralPanel::default()
            .frame(Frame::none().fill(BG_DEEP).inner_margin(Margin::same(12.0)))
            .show(ctx, |ui| {
                if self.series.is_empty() {
                    if ui::draw_empty_state(ui) {
                        do_add = true;
                    }
                } else {
                    let ylab = if self.options.log_y {
                        "value (log10)"
                    } else {
                        "value"
                    };
                    ui::draw_plot(
                        ui,
                        &self.series,
                        &self.options.x_col,
                        ylab,
                        self.options.line_w,
                    );
                }
            });

        if do_add {
            self.pick_files();
        }
        if do_export {
            if self.series.is_empty() {
                self.status = "Nothing selected to export.".into();
            } else {
                self.export_plot();
            }
        }
    }

    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        // Best-effort stop; process exit will join remaining work.
        self.backend.send(Command::Shutdown);
    }
}
