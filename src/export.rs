use crate::series::Series;
use plotters::prelude::*;
use std::path::Path;

const BG: RGBColor = RGBColor(18, 20, 24);
const FG: RGBColor = RGBColor(232, 236, 242);
const GRID_BOLD: RGBColor = RGBColor(48, 54, 68);
const GRID_LIGHT: RGBColor = RGBColor(34, 38, 48);

pub fn export_png(
    path: &Path,
    series: &[Series],
    x_label: &str,
    log_y: bool,
    size: (u32, u32),
    line_w: u32,
) -> Result<(), String> {
    if series.is_empty() {
        return Err("Nothing selected to plot.".into());
    }

    let (mut xmin, mut xmax) = (f64::INFINITY, f64::NEG_INFINITY);
    let (mut ymin, mut ymax) = (f64::INFINITY, f64::NEG_INFINITY);
    for s in series {
        for (x, y) in s.xs.iter().zip(&s.ys) {
            if x.is_finite() && y.is_finite() {
                xmin = xmin.min(*x);
                xmax = xmax.max(*x);
                ymin = ymin.min(*y);
                ymax = ymax.max(*y);
            }
        }
    }
    if !xmin.is_finite() || !ymin.is_finite() {
        return Err("Selected series contain no finite points.".into());
    }
    if (xmax - xmin).abs() < f64::EPSILON {
        xmin -= 0.5;
        xmax += 0.5;
    }
    if (ymax - ymin).abs() < f64::EPSILON {
        ymin -= 0.5;
        ymax += 0.5;
    }
    let pad = (ymax - ymin) * 0.05;
    ymin -= pad;
    ymax += pad;

    let root = BitMapBackend::new(path, size).into_drawing_area();
    root.fill(&BG).map_err(|e| e.to_string())?;

    let y_desc = if log_y { "value (log10)" } else { "value" };
    let mut chart = ChartBuilder::on(&root)
        .margin(18)
        .x_label_area_size(52)
        .y_label_area_size(72)
        .build_cartesian_2d(xmin..xmax, ymin..ymax)
        .map_err(|e| e.to_string())?;

    chart
        .configure_mesh()
        .axis_style(FG)
        .bold_line_style(GRID_BOLD)
        .light_line_style(GRID_LIGHT)
        .label_style(("sans-serif", 15).into_font().color(&FG))
        .x_desc(x_label)
        .y_desc(y_desc)
        .draw()
        .map_err(|e| e.to_string())?;

    for s in series {
        let color = RGBColor(s.color.0, s.color.1, s.color.2);
        if let Some(raw) = &s.raw_ys {
            let faint = color.mix(0.25);
            let pts: Vec<(f64, f64)> =
                s.xs.iter()
                    .zip(raw)
                    .filter(|(_, y)| y.is_finite())
                    .map(|(x, y)| (*x, *y))
                    .collect();
            chart
                .draw_series(LineSeries::new(pts, faint.stroke_width(line_w.max(1))))
                .map_err(|e| e.to_string())?;
        }
        let pts: Vec<(f64, f64)> =
            s.xs.iter()
                .zip(&s.ys)
                .filter(|(_, y)| y.is_finite())
                .map(|(x, y)| (*x, *y))
                .collect();
        chart
            .draw_series(LineSeries::new(pts, color.stroke_width(line_w.max(1) + 1)))
            .map_err(|e| e.to_string())?
            .label(s.name.clone())
            .legend(move |(x, y)| {
                PathElement::new(vec![(x, y), (x + 20, y)], color.stroke_width(3))
            });
    }

    chart
        .configure_series_labels()
        .background_style(BG.mix(0.85))
        .border_style(RGBColor(48, 54, 68))
        .label_font(("sans-serif", 14).into_font().color(&FG))
        .position(SeriesLabelPosition::UpperRight)
        .draw()
        .map_err(|e| e.to_string())?;

    root.present().map_err(|e| e.to_string())?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::series::Series;

    #[test]
    fn export_empty_errors() {
        let err = export_png(Path::new("x.png"), &[], "x", false, (100, 100), 2).unwrap_err();
        assert!(err.contains("Nothing"));
    }

    #[test]
    fn export_writes_png() {
        let series = [Series {
            name: "loss · run".into(),
            color: (94, 186, 167),
            xs: vec![1.0, 2.0, 3.0],
            ys: vec![1.0, 0.5, 0.25],
            raw_ys: None,
        }];
        let dir = std::env::temp_dir().join(format!("tview-export-test-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        let path = dir.join("out.png");
        export_png(&path, &series, "step", false, (320, 240), 2).unwrap();
        let meta = std::fs::metadata(&path).unwrap();
        assert!(meta.len() > 100);
        let _ = std::fs::remove_file(&path);
        let _ = std::fs::remove_dir(&dir);
    }
}
