use crate::backend::{ColumnView, FileView, PlotOptions};
use crate::desktop::theme::{
    self, ACCENT, BG_DEEP, BG_ELEVATED, BORDER, TEXT, TEXT_DIM, TEXT_MUTED,
};
use crate::series::{Series, palette};
use egui::{Color32, ComboBox, Frame, Margin, RichText, Sense, Stroke, Ui};
use egui_plot::{Legend, Line, Plot, PlotPoints};

pub fn color32(c: (u8, u8, u8)) -> Color32 {
    Color32::from_rgb(c.0, c.1, c.2)
}

pub fn color32_a(c: (u8, u8, u8), a: u8) -> Color32 {
    Color32::from_rgba_unmultiplied(c.0, c.1, c.2, a)
}

#[derive(Default)]
pub struct ControlEvents {
    pub add_files: bool,
    pub export: bool,
    pub remove: Option<usize>,
    pub set_visible: Option<(usize, bool)>,
    pub set_selected: Option<(usize, usize, bool)>,
    pub options_changed: bool,
    pub live_changed: bool,
}

/// Mutable display options edited in the sidebar (copied to backend on change).
pub struct DisplaySettings<'a> {
    pub options: &'a mut PlotOptions,
    pub live_reload: &'a mut bool,
}

pub fn draw_toolbar(ui: &mut Ui, busy: bool) -> ControlEvents {
    let mut ev = ControlEvents::default();
    ui.horizontal(|ui| {
        ui.add_space(2.0);
        ui.vertical(|ui| {
            ui.add_space(2.0);
            ui.horizontal(|ui| {
                ui.label(RichText::new("tview").size(20.0).strong().color(ACCENT));
                ui.add_space(6.0);
                ui.label(RichText::new("Plot & compare tabular metrics").color(TEXT_MUTED));
                if busy {
                    ui.add_space(8.0);
                    ui.spinner();
                    ui.label(RichText::new("working…").small().color(TEXT_DIM));
                }
            });
        });
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if theme::secondary_button(ui, "Export PNG").clicked() {
                ev.export = true;
            }
            ui.add_space(6.0);
            if theme::primary_button(ui, "+  Add files").clicked() {
                ev.add_files = true;
            }
        });
    });
    ev
}

pub fn draw_plot(ui: &mut Ui, series: &[Series], x_label: &str, y_label: &str, width: f32) {
    // Pan only with an explicit modifier — plain drag was too easy to knock
    // the curve out of view and disable auto-bounds.
    let pan = ui.input(|i| i.modifiers.shift_only());
    Frame::none()
        .fill(BG_DEEP)
        .stroke(Stroke::new(1.0_f32, BORDER))
        .rounding(10.0)
        .inner_margin(Margin::same(8.0))
        .show(ui, |ui| {
            let response = Plot::new("main_plot")
                .legend(
                    Legend::default()
                        .background_alpha(0.85)
                        .text_style(egui::TextStyle::Small),
                )
                .x_axis_label(x_label)
                .y_axis_label(y_label)
                .auto_bounds(egui::Vec2b::TRUE)
                .allow_drag(pan)
                .allow_zoom(true)
                .allow_scroll(true)
                .allow_boxed_zoom(true)
                .allow_double_click_reset(true)
                .show_axes([true, true])
                .show_grid([true, true])
                .height(ui.available_height())
                .show(ui, |pu| {
                    for s in series {
                        if let Some(raw) = &s.raw_ys {
                            let pts: PlotPoints =
                                s.xs.iter()
                                    .zip(raw)
                                    .filter(|(_, y)| y.is_finite())
                                    .map(|(x, y)| [*x, *y])
                                    .collect();
                            pu.line(
                                Line::new(pts)
                                    .color(color32_a(s.color, 55))
                                    .width(width * 0.55),
                            );
                        }
                        let pts: PlotPoints =
                            s.xs.iter()
                                .zip(&s.ys)
                                .filter(|(_, y)| y.is_finite())
                                .map(|(x, y)| [*x, *y])
                                .collect();
                        pu.line(
                            Line::new(pts)
                                .color(color32(s.color))
                                .width(width)
                                .name(&s.name),
                        );
                    }
                })
                .response;
            response.on_hover_text("Scroll: zoom · Shift+drag: pan · Double-click: fit to data");
        });
}

pub fn draw_empty_state(ui: &mut Ui) -> bool {
    let mut add = false;
    ui.vertical_centered(|ui| {
        ui.add_space(ui.available_height() * 0.22);
        ui.label(
            RichText::new("Drop files to get started")
                .size(26.0)
                .strong()
                .color(TEXT),
        );
        ui.add_space(8.0);
        ui.label(
            RichText::new(
                "Open CSV or TSV metric logs, tick the columns you care about,\nand compare runs side by side.",
            )
            .color(TEXT_MUTED),
        );
        ui.add_space(20.0);
        if theme::primary_button(ui, "+  Add CSV / TSV files").clicked() {
            add = true;
        }
        ui.add_space(12.0);
        ui.label(
            RichText::new("Tip: drag & drop files anywhere on the window")
                .small()
                .color(TEXT_DIM),
        );
    });
    add
}

pub fn draw_controls(
    ui: &mut Ui,
    files: &[FileView],
    settings: DisplaySettings<'_>,
    x_opts: &[String],
) -> ControlEvents {
    let mut ev = ControlEvents::default();
    let DisplaySettings {
        options,
        live_reload,
    } = settings;

    theme::section_label(ui, "Display");

    Frame::none()
        .fill(BG_ELEVATED)
        .stroke(Stroke::new(1.0_f32, BORDER))
        .rounding(8.0)
        .inner_margin(Margin::symmetric(12.0, 10.0))
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label(RichText::new("X axis").color(TEXT_MUTED));
                let before = options.x_col.clone();
                ComboBox::from_id_source("x_axis")
                    .selected_text(options.x_col.clone())
                    .width(ui.available_width().max(120.0))
                    .show_ui(ui, |ui| {
                        for o in x_opts {
                            ui.selectable_value(&mut options.x_col, o.clone(), o);
                        }
                    });
                if options.x_col != before {
                    ev.options_changed = true;
                }
            });
            ui.add_space(4.0);
            if ui.checkbox(&mut options.log_y, "Log-scale Y").changed() {
                ev.options_changed = true;
            }
            let mut live = *live_reload;
            if ui
                .checkbox(&mut live, "Live reload (watch files)")
                .on_hover_text("Backend polls open files every half second and re-plots on append.")
                .changed()
            {
                *live_reload = live;
                ev.live_changed = true;
            }
            let mut smoothing = options.smoothing as f32;
            if ui
                .add(
                    egui::Slider::new(&mut smoothing, 0.0..=0.95)
                        .text("Smoothing")
                        .show_value(true),
                )
                .changed()
            {
                options.smoothing = f64::from(smoothing);
                ev.options_changed = true;
            }
            if ui
                .add(
                    egui::Slider::new(&mut options.line_w, 0.5..=4.0)
                        .text("Line width")
                        .show_value(true),
                )
                .changed()
            {
                ev.options_changed = true;
            }
        });

    ui.add_space(8.0);
    theme::section_label(ui, &format!("Files ({})", files.len()));

    if files.is_empty() {
        Frame::none()
            .fill(BG_ELEVATED)
            .stroke(Stroke::new(1.0_f32, BORDER))
            .rounding(8.0)
            .inner_margin(Margin::same(14.0))
            .show(ui, |ui| {
                ui.label(RichText::new("No files loaded yet.").color(TEXT_MUTED));
                ui.label(
                    RichText::new("Use Add files or drag a CSV/TSV here.")
                        .small()
                        .color(TEXT_DIM),
                );
            });
        return ev;
    }

    egui::ScrollArea::vertical()
        .auto_shrink([false, false])
        .show(ui, |ui| {
            let mut color_idx = 0usize;
            for (fi, file) in files.iter().enumerate() {
                let file_visible = file.visible;

                Frame::none()
                    .fill(if file_visible {
                        BG_ELEVATED
                    } else {
                        Color32::from_rgb(28, 30, 36)
                    })
                    .stroke(Stroke::new(1.0_f32, BORDER))
                    .rounding(8.0)
                    .inner_margin(Margin::symmetric(10.0, 8.0))
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            let mut visible = file.visible;
                            if ui.checkbox(&mut visible, "").changed() {
                                ev.set_visible = Some((fi, visible));
                            }
                            let rest = ui.available_width();
                            ui.allocate_ui_with_layout(
                                egui::vec2(rest, ui.spacing().interact_size.y),
                                egui::Layout::right_to_left(egui::Align::Center),
                                |ui| {
                                    if theme::danger_button(ui, "Remove").clicked() {
                                        ev.remove = Some(fi);
                                    }
                                    ui.add_space(8.0);
                                    ui.label(
                                        RichText::new(format!("{} rows", file.nrows))
                                            .small()
                                            .color(TEXT_DIM),
                                    );
                                    ui.with_layout(
                                        egui::Layout::left_to_right(egui::Align::Center),
                                        |ui| {
                                            ui.label(
                                                RichText::new(&file.name).strong().color(TEXT),
                                            )
                                            .on_hover_text(&file.path);
                                        },
                                    );
                                },
                            );
                        });

                        ui.add_space(4.0);
                        for (ci, col) in file.columns.iter().enumerate() {
                            if !col.numeric {
                                continue;
                            }
                            draw_column_row(
                                ui,
                                col,
                                &options.x_col,
                                file_visible,
                                &mut color_idx,
                                |selected| {
                                    ev.set_selected = Some((fi, ci, selected));
                                },
                            );
                        }
                    });
                ui.add_space(6.0);
            }
        });

    ev
}

fn draw_column_row(
    ui: &mut Ui,
    col: &ColumnView,
    x_col: &str,
    file_visible: bool,
    color_idx: &mut usize,
    on_toggle: impl FnOnce(bool),
) {
    let is_x = col.name == x_col;
    let fill = if col.selected && file_visible && !is_x {
        let c = color32(palette(*color_idx));
        *color_idx += 1;
        c
    } else if col.selected {
        TEXT_MUTED
    } else {
        TEXT_DIM
    };

    ui.horizontal(|ui| {
        let (rect, _) = ui.allocate_exact_size(egui::vec2(8.0, 8.0), Sense::hover());
        ui.painter().circle_filled(rect.center(), 4.0, fill);
        let mut selected = col.selected;
        if ui.checkbox(&mut selected, &col.name).changed() {
            on_toggle(selected);
        }
    });
}

pub fn draw_status_bar(ui: &mut Ui, status: &str, n_series: usize, n_files: usize, live: bool) {
    ui.horizontal(|ui| {
        ui.label(
            RichText::new(format!("{n_files} files"))
                .small()
                .color(TEXT_DIM),
        );
        ui.label(RichText::new("·").small().color(TEXT_DIM));
        ui.label(
            RichText::new(format!("{n_series} series"))
                .small()
                .color(TEXT_DIM),
        );
        if live && n_files > 0 {
            ui.label(RichText::new("·").small().color(TEXT_DIM));
            ui.label(RichText::new("live").small().color(ACCENT));
        }
        if !status.is_empty() {
            ui.separator();
            ui.label(RichText::new(status).small().color(TEXT_MUTED));
        }
    });
}
