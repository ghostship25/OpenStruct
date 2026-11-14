//! Plan-outline generators for area objects.
//!
//! Circles, ellipses and arcs are tessellated here to provide topology joints.
//! Created slabs also retain the exact `AreaCurve`, so drawing, area and
//! curve-aware splitting do not depend on the tessellation density.

use std::f64::consts::TAU;

/// Rectangle through two opposite corners.
pub fn rectangle(a: (f64, f64), b: (f64, f64)) -> Vec<(f64, f64)> {
    vec![(a.0, a.1), (b.0, a.1), (b.0, b.1), (a.0, b.1)]
}

/// Closed ellipse; a circle is `rx == ry`.
pub fn ellipse(center: (f64, f64), rx: f64, ry: f64, segments: usize) -> Vec<(f64, f64)> {
    let count = segments.clamp(8, 256);
    let (rx, ry) = (rx.abs(), ry.abs());
    (0..count)
        .map(|index| {
            let angle = TAU * index as f64 / count as f64;
            (center.0 + rx * angle.cos(), center.1 + ry * angle.sin())
        })
        .collect()
}

/// Open arc from `start` to `end` radians, both endpoints included.
pub fn arc(
    center: (f64, f64),
    rx: f64,
    ry: f64,
    start: f64,
    end: f64,
    segments: usize,
) -> Vec<(f64, f64)> {
    let count = segments.clamp(2, 256);
    let (rx, ry) = (rx.abs(), ry.abs());
    (0..=count)
        .map(|index| {
            let angle = start + (end - start) * index as f64 / count as f64;
            (center.0 + rx * angle.cos(), center.1 + ry * angle.sin())
        })
        .collect()
}

/// Shoelace area of a closed 2D polygon.
pub fn signed_area(points: &[(f64, f64)]) -> f64 {
    if points.len() < 3 {
        return 0.0;
    }
    let mut sum = 0.0;
    for index in 0..points.len() {
        let a = points[index];
        let b = points[(index + 1) % points.len()];
        sum += a.0 * b.1 - b.0 * a.1;
    }
    0.5 * sum
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rectangle_outline_and_area() {
        let outline = rectangle((0.0, 0.0), (2.0, 3.0));
        assert_eq!(outline.len(), 4);
        assert!((signed_area(&outline).abs() - 6.0).abs() < 1e-9);
    }

    #[test]
    fn circle_tessellation_approximates_area() {
        let outline = ellipse((1.0, 2.0), 2.0, 2.0, 64);
        assert_eq!(outline.len(), 64);
        let expected = std::f64::consts::PI * 4.0;
        assert!((signed_area(&outline).abs() - expected).abs() / expected < 0.01);
    }

    #[test]
    fn arc_includes_both_endpoints() {
        let outline = arc((0.0, 0.0), 2.0, 2.0, 0.0, std::f64::consts::FRAC_PI_2, 8);
        assert_eq!(outline.len(), 9);
        assert!((outline[0].0 - 2.0).abs() < 1e-9);
        assert!((outline[8].1 - 2.0).abs() < 1e-9);
    }
}
