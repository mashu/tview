use crate::data::DataFile;

pub const ROW_INDEX: &str = "(row index)";

/// A palette that reads well on the modern dark theme.
pub const PALETTE: [(u8, u8, u8); 10] = [
    (94, 186, 167),  // teal accent
    (120, 170, 255), // sky
    (240, 160, 96),  // apricot
    (200, 140, 255), // lilac
    (120, 210, 140), // mint
    (255, 130, 150), // rose
    (255, 210, 100), // gold
    (100, 210, 220), // cyan
    (180, 200, 255), // periwinkle
    (255, 150, 110), // coral
];

pub fn palette(i: usize) -> (u8, u8, u8) {
    PALETTE[i % PALETTE.len()]
}

pub struct Series {
    pub name: String,
    pub color: (u8, u8, u8),
    pub xs: Vec<f64>,
    pub ys: Vec<f64>,
    /// Unsmoothed values (post-transform), drawn faintly when smoothing is on.
    pub raw_ys: Option<Vec<f64>>,
}

/// Build every selected (file, column) into a plottable series.
/// `smoothing` in [0, 1): 0 = off, higher = smoother EMA.
pub fn build_series(files: &[DataFile], x_col: &str, log_y: bool, smoothing: f64) -> Vec<Series> {
    let transform = |v: f64| -> f64 {
        if log_y {
            if v > 0.0 { v.log10() } else { f64::NAN }
        } else {
            v
        }
    };
    let mut out = Vec::new();
    let mut color_idx = 0usize;
    for f in files {
        if !f.visible {
            continue;
        }
        let x_idx = if x_col == ROW_INDEX {
            None
        } else {
            f.column_index(x_col)
        };
        for (ci, col) in f.columns.iter().enumerate() {
            if !f.selected[ci] || !col.numeric {
                continue;
            }
            if Some(ci) == x_idx {
                continue; // don't plot X against itself
            }
            let mut xs = Vec::new();
            let mut ys_in = Vec::new();
            for r in 0..f.nrows {
                let y = col.values[r];
                let x = match x_idx {
                    Some(xi) => f.columns[xi].values[r],
                    None => r as f64,
                };
                if x.is_finite() && y.is_finite() {
                    xs.push(x);
                    ys_in.push(y);
                }
            }
            if xs.is_empty() {
                continue;
            }
            let color = palette(color_idx);
            color_idx += 1;
            let (ys, raw_ys) = if smoothing > 0.0 {
                let sm = ema(&ys_in, 1.0 - smoothing);
                (
                    sm.iter().map(|v| transform(*v)).collect(),
                    Some(ys_in.iter().map(|v| transform(*v)).collect()),
                )
            } else {
                (ys_in.iter().map(|v| transform(*v)).collect(), None)
            };
            out.push(Series {
                name: format!("{} · {}", col.name, f.name),
                color,
                xs,
                ys,
                raw_ys,
            });
        }
    }
    out
}

/// Exponential moving average. `alpha` is the weight of the new sample.
pub fn ema(v: &[f64], alpha: f64) -> Vec<f64> {
    let mut out = Vec::with_capacity(v.len());
    let mut s = 0.0;
    for (i, &x) in v.iter().enumerate() {
        s = if i == 0 {
            x
        } else {
            alpha * x + (1.0 - alpha) * s
        };
        out.push(s);
    }
    out
}

/// Union of column names across all files, preserving first-seen order.
pub fn x_axis_options(files: &[DataFile]) -> Vec<String> {
    let mut opts = vec![ROW_INDEX.to_string()];
    for f in files {
        for c in &f.columns {
            if c.numeric && !opts.iter().any(|o| o == &c.name) {
                opts.push(c.name.clone());
            }
        }
    }
    opts
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::DataFile;
    use std::path::Path;

    fn sample_file() -> DataFile {
        let csv = "step,loss,acc\n1,1.0,0.1\n2,0.5,0.4\n3,0.25,0.7\n";
        let mut f = DataFile::parse(Path::new("run_a.csv"), csv).unwrap();
        // select loss
        f.selected[1] = true;
        f
    }

    #[test]
    fn x_axis_options_includes_row_index_first() {
        let f = sample_file();
        let opts = x_axis_options(&[f]);
        assert_eq!(opts[0], ROW_INDEX);
        assert!(opts.iter().any(|o| o == "step"));
        assert!(opts.iter().any(|o| o == "loss"));
    }

    #[test]
    fn build_series_basic() {
        let f = sample_file();
        let series = build_series(&[f], "step", false, 0.0);
        assert_eq!(series.len(), 1);
        assert_eq!(series[0].xs, vec![1.0, 2.0, 3.0]);
        assert_eq!(series[0].ys, vec![1.0, 0.5, 0.25]);
        assert!(series[0].raw_ys.is_none());
        assert!(series[0].name.contains("loss"));
    }

    #[test]
    fn build_series_skips_invisible_and_x_column() {
        let mut f = sample_file();
        f.selected[0] = true; // step selected too — skipped when used as X
        let series = build_series(&[f], "step", false, 0.0);
        assert_eq!(series.len(), 1);

        let mut hidden = sample_file();
        hidden.visible = false;
        assert!(build_series(&[hidden], "step", false, 0.0).is_empty());
    }

    #[test]
    fn build_series_log_y_and_smoothing() {
        let f = sample_file();
        let series = build_series(&[f], ROW_INDEX, true, 0.5);
        assert_eq!(series.len(), 1);
        assert!(series[0].raw_ys.is_some());
        assert!(series[0].ys[0].is_finite());
        // log10(1) == 0 for first raw value after EMA start
        assert!((series[0].raw_ys.as_ref().unwrap()[0] - 0.0).abs() < 1e-9);
    }

    #[test]
    fn ema_identity_when_alpha_one() {
        let v = vec![1.0, 2.0, 3.0];
        assert_eq!(ema(&v, 1.0), v);
    }

    #[test]
    fn palette_wraps() {
        assert_eq!(palette(0), palette(PALETTE.len()));
    }
}
