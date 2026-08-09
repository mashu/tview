use crate::data::{DataFile, RefreshOutcome};
use crate::series::{ROW_INDEX, build_series, x_axis_options};
use crate::theme::{self, ACCENT, BG_DEEP, BORDER};
use crate::ui;
use eframe::egui;
use egui::{Context, Frame, Margin, Stroke};
use std::path::PathBuf;
use std::time::{Duration, Instant};

const LIVE_POLL_INTERVAL: Duration = Duration::from_millis(500);

pub struct TsvPlotApp {
    files: Vec<DataFile>,
    x_col: String,
    log_y: bool,
    smoothing: f32,
    line_w: f32,
    live_reload: bool,
    last_poll: Instant,
    status: String,
}

impl Default for TsvPlotApp {
    fn default() -> Self {
        Self {
            files: Vec::new(),
            x_col: ROW_INDEX.to_string(),
            log_y: false,
            smoothing: 0.0,
            line_w: 2.0,
            live_reload: true,
            last_poll: Instant::now()
                .checked_sub(LIVE_POLL_INTERVAL)
                .unwrap_or_else(Instant::now),
            status: String::new(),
        }
    }
}

impl TsvPlotApp {
    /// Construct the app, optionally preloading files given on the command line.
    pub fn new(initial: Vec<PathBuf>) -> Self {
        let mut app = Self::default();
        if !initial.is_empty() {
            app.load_paths(initial);
        }
        app
    }

    fn load_paths(&mut self, paths: Vec<PathBuf>) {
        let mut loaded = 0usize;
        for p in paths {
            match DataFile::load(&p) {
                Ok(f) => {
                    self.files.push(f);
                    self.default_select_last();
                    loaded += 1;
                    self.status = format!("Loaded {}", p.display());
                }
                Err(e) => self.status = format!("Error loading {}: {e}", p.display()),
            }
        }
        if loaded > 1 {
            self.status = format!("Loaded {loaded} files");
        }

        let opts = x_axis_options(&self.files);
        if !opts.iter().any(|o| o == &self.x_col) {
            self.x_col = ROW_INDEX.to_string();
        }
        if self.x_col == ROW_INDEX
            && let Some(x) = opts
                .iter()
                .find(|o| matches!(o.as_str(), "step" | "epoch" | "iter" | "iteration"))
        {
            self.x_col = x.clone();
        }
    }

    fn default_select_last(&mut self) {
        let Some(f) = self.files.last_mut() else {
            return;
        };
        if f.selected.iter().any(|&b| b) {
            return;
        }
        let pick = f
            .columns
            .iter()
            .position(|c| c.numeric && c.name.eq_ignore_ascii_case("loss"))
            .or_else(|| {
                f.columns.iter().position(|c| {
                    c.numeric && !matches!(c.name.as_str(), "step" | "epoch" | "iter" | "iteration")
                })
            });
        if let Some(i) = pick {
            f.selected[i] = true;
        }
    }

    fn pick_files(&mut self) {
        if let Some(paths) = rfd::FileDialog::new()
            .add_filter("tabular", &["csv", "tsv", "txt", "tab"])
            .pick_files()
        {
            self.load_paths(paths);
        }
    }

    fn export_plot(&mut self, series: &[crate::series::Series]) {
        if series.is_empty() {
            self.status = "Nothing selected to export.".into();
            return;
        }
        if let Some(path) = rfd::FileDialog::new()
            .add_filter("PNG", &["png"])
            .set_file_name("plot.png")
            .save_file()
        {
            let lw = (self.line_w.round() as u32).max(1);
            match crate::export::export_png(
                &path,
                series,
                &self.x_col,
                self.log_y,
                (1600, 1000),
                lw,
            ) {
                Ok(()) => self.status = format!("Saved {}", path.display()),
                Err(e) => self.status = format!("Export failed: {e}"),
            }
        }
    }

    /// Poll open files for appends / rewrites and keep the UI waking while live.
    fn poll_live_files(&mut self, ctx: &Context) {
        if !self.live_reload || self.files.is_empty() {
            return;
        }

        let now = Instant::now();
        let due = now.duration_since(self.last_poll) >= LIVE_POLL_INTERVAL;
        if due {
            self.last_poll = now;
            let mut updated = 0usize;
            let mut rows_after = 0usize;
            for f in &mut self.files {
                match f.refresh_from_disk() {
                    Ok(RefreshOutcome::Reloaded) => {
                        updated += 1;
                        rows_after += f.nrows;
                    }
                    Ok(RefreshOutcome::Unchanged) => {}
                    Err(e) => {
                        self.status = format!("Watch error: {e}");
                    }
                }
            }
            if updated > 0 {
                self.status = if updated == 1 {
                    format!("Live update · {rows_after} rows")
                } else {
                    format!("Live update · {updated} files refreshed")
                };
            }
        }

        let remaining = LIVE_POLL_INTERVAL.saturating_sub(now.duration_since(self.last_poll));
        ctx.request_repaint_after(remaining.max(Duration::from_millis(50)));
    }
}

impl eframe::App for TsvPlotApp {
    fn update(&mut self, ctx: &Context, _frame: &mut eframe::Frame) {
        self.poll_live_files(ctx);

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
            painter.rect_stroke(screen.shrink(18.0), 12.0, Stroke::new(2.0, ACCENT));
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
            self.load_paths(dropped);
        }

        let x_opts = x_axis_options(&self.files);
        let series = build_series(&self.files, &self.x_col, self.log_y, self.smoothing as f64);
        let mut do_add = false;
        let mut do_export = false;
        let mut do_remove = None;

        egui::TopBottomPanel::top("toolbar")
            .frame(
                Frame::none()
                    .fill(theme::BG_PANEL)
                    .stroke(Stroke::new(1.0, BORDER))
                    .inner_margin(Margin::symmetric(16.0, 10.0)),
            )
            .show(ctx, |ui| {
                let ev = ui::draw_toolbar(ui);
                do_add |= ev.add_files;
                do_export |= ev.export;
            });

        egui::TopBottomPanel::bottom("status")
            .exact_height(28.0)
            .frame(
                Frame::none()
                    .fill(theme::BG_PANEL)
                    .stroke(Stroke::new(1.0, BORDER))
                    .inner_margin(Margin::symmetric(14.0, 4.0)),
            )
            .show(ctx, |ui| {
                ui::draw_status_bar(
                    ui,
                    &self.status,
                    series.len(),
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
                    .stroke(Stroke::new(1.0, BORDER))
                    .inner_margin(Margin::symmetric(14.0, 12.0)),
            )
            .show(ctx, |ui| {
                let ev = ui::draw_controls(
                    ui,
                    &mut self.files,
                    ui::DisplaySettings {
                        x_col: &mut self.x_col,
                        log_y: &mut self.log_y,
                        smoothing: &mut self.smoothing,
                        line_w: &mut self.line_w,
                        live_reload: &mut self.live_reload,
                    },
                    &x_opts,
                );
                do_add |= ev.add_files;
                do_export |= ev.export;
                do_remove = ev.remove;
            });

        egui::CentralPanel::default()
            .frame(Frame::none().fill(BG_DEEP).inner_margin(Margin::same(12.0)))
            .show(ctx, |ui| {
                if series.is_empty() {
                    if ui::draw_empty_state(ui) {
                        do_add = true;
                    }
                } else {
                    let ylab = if self.log_y { "value (log10)" } else { "value" };
                    ui::draw_plot(ui, &series, &self.x_col, ylab, self.line_w);
                }
            });

        if let Some(i) = do_remove
            && i < self.files.len()
        {
            let name = self.files[i].name.clone();
            self.files.remove(i);
            self.status = format!("Removed {name}");
        }
        if do_add {
            self.pick_files();
        }
        if do_export {
            self.export_plot(&series);
        }
    }
}
