//! Local slope / trend estimation over a trailing window of points.

/// Classification of local trend relative to a plateau band.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Trend {
    Decreasing,
    Plateau,
    Increasing,
}

impl Trend {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Decreasing => "decreasing",
            Self::Plateau => "plateau",
            Self::Increasing => "increasing",
        }
    }

    pub fn arrow(self) -> &'static str {
        match self {
            Self::Decreasing => "↓",
            Self::Plateau => "→",
            Self::Increasing => "↑",
        }
    }
}

/// OLS fit on `xs[start..=end]` / `ys[start..=end]`.
#[derive(Debug, Clone, Copy)]
pub struct SlopeFit {
    pub slope: f64,
    pub intercept: f64,
    pub x0: f64,
    pub y0: f64,
    pub x_first: f64,
    pub x_last: f64,
    pub n: usize,
    /// Relative change over the window: `(slope * x_span) / |mean_y|`.
    pub rel: f64,
    pub trend: Trend,
}

/// Binary-search nearest index for sorted `xs`.
pub fn nearest_index(xs: &[f64], x: f64) -> Option<usize> {
    if xs.is_empty() {
        return None;
    }
    let mut lo = 0usize;
    let mut hi = xs.len() - 1;
    while lo < hi {
        let mid = (lo + hi) / 2;
        if xs[mid] < x {
            lo = mid + 1;
        } else {
            hi = mid;
        }
    }
    if lo > 0 && (xs[lo - 1] - x).abs() <= (xs[lo] - x).abs() {
        Some(lo - 1)
    } else {
        Some(lo)
    }
}

/// Least-squares slope on the trailing window ending at `end_idx`.
///
/// `plateau_frac` is the relative-Δ threshold (e.g. `0.02` = 2%). Below that
/// band in absolute value → [`Trend::Plateau`].
pub fn local_slope(
    xs: &[f64],
    ys: &[f64],
    end_idx: usize,
    window: usize,
    plateau_frac: f64,
) -> Option<SlopeFit> {
    if xs.len() != ys.len() || xs.is_empty() {
        return None;
    }
    let end = end_idx.min(xs.len() - 1);
    let win = window.max(2);
    let start = end.saturating_sub(win - 1);

    let mut n = 0usize;
    let mut sum_x = 0.0;
    let mut sum_y = 0.0;
    let mut sum_xx = 0.0;
    let mut sum_xy = 0.0;
    let mut x_first = f64::NAN;
    let mut x_last = f64::NAN;

    for i in start..=end {
        let x = xs[i];
        let y = ys[i];
        if !(x.is_finite() && y.is_finite()) {
            continue;
        }
        if !x_first.is_finite() {
            x_first = x;
        }
        x_last = x;
        n += 1;
        sum_x += x;
        sum_y += y;
        sum_xx += x * x;
        sum_xy += x * y;
    }
    if n < 2 {
        return None;
    }
    let denom = (n as f64) * sum_xx - sum_x * sum_x;
    if denom.abs() < 1e-18 {
        return None;
    }
    let slope = ((n as f64) * sum_xy - sum_x * sum_y) / denom;
    let mean_y = sum_y / n as f64;
    let intercept = mean_y - slope * (sum_x / n as f64);
    let x_span = x_last - x_first;
    let rel = (slope * x_span) / mean_y.abs().max(1e-12);
    let band = plateau_frac.max(0.0);
    let trend = if rel < -band {
        Trend::Decreasing
    } else if rel > band {
        Trend::Increasing
    } else {
        Trend::Plateau
    };
    let x0 = xs[end];
    if !x0.is_finite() {
        return None;
    }
    let y0 = intercept + slope * x0;
    Some(SlopeFit {
        slope,
        intercept,
        x0,
        y0,
        x_first,
        x_last,
        n,
        rel,
        trend,
    })
}

impl SlopeFit {
    /// Half-width for a tangent segment in x units.
    pub fn tangent_half_span(&self, series_x_span: f64) -> f64 {
        let half = (self.x_last - self.x_first).abs() * 0.6;
        if half > 0.0 {
            half
        } else {
            series_x_span.abs().max(1.0) * 0.03
        }
    }

    pub fn tangent_endpoints(&self, series_x_span: f64) -> [[f64; 2]; 2] {
        let half = self.tangent_half_span(series_x_span);
        let x1 = self.x0 - half;
        let x2 = self.x0 + half;
        [
            [x1, self.intercept + self.slope * x1],
            [x2, self.intercept + self.slope * x2],
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classifies_decreasing_flat_increasing() {
        let xs: Vec<f64> = (1..=10).map(|i| i as f64).collect();
        let dec: Vec<f64> = (1..=10).rev().map(|i| i as f64).collect();
        let flat = vec![5.0, 5.01, 4.99, 5.0, 5.02, 4.98, 5.0, 5.01, 5.0, 5.0];
        let inc: Vec<f64> = (1..=10).map(|i| i as f64).collect();

        let d = local_slope(&xs, &dec, 9, 5, 0.02).unwrap();
        assert_eq!(d.trend, Trend::Decreasing);
        assert!((d.slope + 1.0).abs() < 1e-9);

        let f = local_slope(&xs, &flat, 9, 5, 0.02).unwrap();
        assert_eq!(f.trend, Trend::Plateau);

        let i = local_slope(&xs, &inc, 9, 5, 0.02).unwrap();
        assert_eq!(i.trend, Trend::Increasing);
        assert!((i.slope - 1.0).abs() < 1e-9);
    }

    #[test]
    fn nearest_index_picks_closest() {
        let xs = [1.0, 2.0, 3.0, 4.0];
        assert_eq!(nearest_index(&xs, 2.4), Some(1));
        assert_eq!(nearest_index(&xs, 2.6), Some(2));
        assert_eq!(nearest_index(&xs, 0.0), Some(0));
        assert_eq!(nearest_index(&xs, 99.0), Some(3));
    }
}
