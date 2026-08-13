pub const CANVAS_ZOOM_MIN: f64 = 0.05;

pub enum ZoomDir {
    In,
    Out,
}

/// Salvaged verbatim from src/graf/viewport.rs:23-28.
pub fn clamp_world(v: f64) -> f64 {
    const WORLD_COORD_LIMIT: f64 = 1.0e18;
    if !v.is_finite() {
        return 0.0;
    }
    v.clamp(-WORLD_COORD_LIMIT, WORLD_COORD_LIMIT)
}

/// Body salvaged from Viewport::pan_by (src/graf/viewport.rs:177-186).
/// Returns None on non-finite → caller leaves center unchanged.
pub fn pan_centered(cx: f64, cy: f64, dx: f64, dy: f64) -> Option<(f64, f64)> {
    if !dx.is_finite() || !dy.is_finite() {
        return None;
    }
    let (nx, ny) = (cx + dx, cy + dy);
    if nx.is_finite() && ny.is_finite() {
        Some((nx, ny))
    } else {
        None
    }
}

/// Fuses Viewport::zoom_in/zoom_out (src/graf/viewport.rs:138-159).
/// In: zoom*factor. Out: zoom/factor floored at `min`. Rejects non-finite.
pub fn zoom_step(zoom: f64, factor: f64, dir: ZoomDir, min: f64) -> Option<f64> {
    if !factor.is_finite() || factor <= 0.0 || !zoom.is_finite() || zoom <= 0.0 {
        return None;
    }
    let candidate = match dir {
        ZoomDir::In => zoom * factor,
        ZoomDir::Out => zoom / factor,
    };
    if !candidate.is_finite() || candidate <= 0.0 || !(100.0 / candidate).is_finite() {
        return None;
    }
    let floored = if matches!(dir, ZoomDir::Out) && min.is_finite() && min > 0.0 {
        candidate.max(min)
    } else {
        candidate
    };
    Some(floored)
}

/// 60° cone forward search salvaged from Viewport::nearest_in_direction
/// (src/graf/viewport.rs:204-251). `cands` = candidate positions in view
/// iteration order (caller excludes the current node BEFORE building the
/// slice so ties resolve identically). Returns index into `cands`.
pub fn nearest_in_dir(
    cands: &[(f64, f64)],
    origin: (f64, f64),
    dir: (f64, f64),
    cone: f64,
) -> Option<usize> {
    let dir_len = (dir.0 * dir.0 + dir.1 * dir.1).sqrt();
    if dir_len == 0.0 {
        return None;
    }
    let (ndx, ndy) = (dir.0 / dir_len, dir.1 / dir_len);
    const ANGLE_WEIGHT: f64 = 80.0;
    let mut best: Option<(usize, f64)> = None;
    for (i, &(cx, cy)) in cands.iter().enumerate() {
        let (dx, dy) = (cx - origin.0, cy - origin.1);
        let dist = (dx * dx + dy * dy).sqrt();
        if dist < 1e-6 {
            continue;
        }
        let dot = (dx * ndx + dy * ndy) / dist;
        if dot < 0.0 {
            continue;
        }
        let angle = dot.acos();
        if angle > cone {
            continue;
        }
        let score = ANGLE_WEIGHT * angle + dist;
        match best {
            Some((_, bs)) if score >= bs => {}
            _ => best = Some((i, score)),
        }
    }
    best.map(|(i, _)| i)
}

/// Closest-by-Euclidean to a target point (graf no-selection fallback,
/// salvaged from Viewport::nearest_to_center src/graf/viewport.rs:188-202).
pub fn nearest_to_point(cands: &[(f64, f64)], target: (f64, f64)) -> Option<usize> {
    let mut best: Option<(usize, f64)> = None;
    for (i, &(cx, cy)) in cands.iter().enumerate() {
        let d = ((cx - target.0).powi(2) + (cy - target.1).powi(2)).sqrt();
        match best {
            Some((_, bd)) if d >= bd => {}
            _ => best = Some((i, d)),
        }
    }
    best.map(|(i, _)| i)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::f64::consts::FRAC_PI_3;

    #[test]
    fn clamp_world_bounds() {
        assert_eq!(clamp_world(f64::NAN), 0.0);
        assert_eq!(clamp_world(f64::INFINITY), 0.0);
        assert_eq!(clamp_world(f64::NEG_INFINITY), 0.0);
        assert_eq!(clamp_world(1.0e19), 1.0e18);
        assert_eq!(clamp_world(-1.0e19), -1.0e18);
        assert_eq!(clamp_world(0.0), 0.0);
        assert_eq!(clamp_world(42.0), 42.0);
    }

    #[test]
    fn zoom_step_floors_out() {
        assert_eq!(zoom_step(1.0, 2.0, ZoomDir::In, 0.0), Some(2.0));
        // Out floors at min.
        assert_eq!(zoom_step(0.01, 2.0, ZoomDir::Out, 0.05), Some(0.05));
        assert_eq!(zoom_step(0.2, 2.0, ZoomDir::Out, 0.05), Some(0.1));
        // In ignores non-finite factor.
        assert_eq!(zoom_step(1.0, f64::NAN, ZoomDir::In, 0.0), None);
        // Out rejects factor <= 0.
        assert_eq!(zoom_step(1.0, 0.0, ZoomDir::Out, 0.05), None);
        assert_eq!(zoom_step(1.0, -1.0, ZoomDir::Out, 0.05), None);
        // Non-finite zoom rejected.
        assert_eq!(zoom_step(f64::NAN, 2.0, ZoomDir::In, 0.0), None);
    }

    #[test]
    fn pan_centered_rejects_nan() {
        assert_eq!(pan_centered(1.0, 2.0, f64::NAN, 0.0), None);
        assert_eq!(pan_centered(1.0, 2.0, 3.0, 4.0), Some((4.0, 6.0)));
    }

    #[test]
    fn nearest_in_dir_picks_forward() {
        // origin (0,0), dir (1,0): node at (1,0) forward, (0,1) orthogonal-out,
        // (-1,0) behind.
        let cands = [(1.0, 0.0), (0.0, 1.0), (-1.0, 0.0)];
        assert_eq!(
            nearest_in_dir(&cands, (0.0, 0.0), (1.0, 0.0), FRAC_PI_3),
            Some(0)
        );
        // Behind node only → None.
        assert_eq!(
            nearest_in_dir(&[(-1.0, 0.0)], (0.0, 0.0), (1.0, 0.0), FRAC_PI_3),
            None
        );
        // Outside-cone: node at 90° (dot≈0 within cone? 90° > 60°) → None.
        assert_eq!(
            nearest_in_dir(&[(0.0, 1.0)], (0.0, 0.0), (1.0, 0.0), FRAC_PI_3),
            None
        );
        // Empty → None.
        assert_eq!(nearest_in_dir(&[], (0.0, 0.0), (1.0, 0.0), FRAC_PI_3), None);
        // Tie → first index wins (both at distance 1 on-axis).
        assert_eq!(
            nearest_in_dir(&[(1.0, 0.0), (1.0, 0.0)], (0.0, 0.0), (1.0, 0.0), FRAC_PI_3),
            Some(0)
        );
    }

    #[test]
    fn nearest_to_point_closest_wins() {
        let cands = [(10.0, 10.0), (1.0, 1.0), (5.0, 5.0)];
        assert_eq!(nearest_to_point(&cands, (0.0, 0.0)), Some(1));
        assert_eq!(nearest_to_point(&[], (0.0, 0.0)), None);
    }
}
