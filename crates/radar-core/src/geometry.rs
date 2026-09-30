//! Coordinates and inclusive polygon containment.
use crate::{ORIGIN_LAT, ORIGIN_LON};
use serde::{Deserialize, Serialize};
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub struct Point {
    pub x: f64,
    pub y: f64,
}
impl Point {
    /// Returns radial distance from the radar origin in metres.
    pub(crate) fn distance(self) -> f64 {
        self.x.hypot(self.y)
    }
    /// Converts local east/north metres to longitude and latitude.
    pub fn geographic(self) -> [f64; 2] {
        [
            ORIGIN_LON + (self.x / (6_371_000.0 * ORIGIN_LAT.to_radians().cos())).to_degrees(),
            ORIGIN_LAT + (self.y / 6_371_000.0).to_degrees(),
        ]
    }
}

/// Ray casting with inclusive boundaries, shared by visibility and notifications.
pub fn contains(points: &[Point], p: Point) -> bool {
    if points.len() < 3 {
        return false;
    }
    let mut inside = false;
    for (a, b) in points
        .iter()
        .zip(points.iter().cycle().skip(1))
        .take(points.len())
    {
        let cross = (p.y - a.y) * (b.x - a.x) - (p.x - a.x) * (b.y - a.y);
        if cross.abs() < 1e-7
            && p.x >= a.x.min(b.x)
            && p.x <= a.x.max(b.x)
            && p.y >= a.y.min(b.y)
            && p.y <= a.y.max(b.y)
        {
            return true;
        }
        if (a.y > p.y) != (b.y > p.y) && p.x < (b.x - a.x) * (p.y - a.y) / (b.y - a.y) + a.x {
            inside = !inside;
        }
    }
    inside
}

/// Reject touching/crossing non-adjacent edges and adjacent backtracking.
pub(crate) fn is_simple_ring(points: &[Point]) -> bool {
    const EPS: f64 = 1e-7;
    fn cross(a: Point, b: Point, c: Point) -> f64 {
        (b.x - a.x) * (c.y - a.y) - (b.y - a.y) * (c.x - a.x)
    }
    fn on(a: Point, b: Point, p: Point) -> bool {
        cross(a, b, p).abs() <= EPS
            && p.x >= a.x.min(b.x) - EPS
            && p.x <= a.x.max(b.x) + EPS
            && p.y >= a.y.min(b.y) - EPS
            && p.y <= a.y.max(b.y) + EPS
    }
    let n = points.len();
    if n < 3 {
        return false;
    }
    for i in 0..n {
        let (a, b, c) = (points[i], points[(i + 1) % n], points[(i + 2) % n]);
        if (a.x - b.x).hypot(a.y - b.y) <= EPS {
            return false;
        }
        if cross(a, b, c).abs() <= EPS
            && (b.x - a.x) * (c.x - b.x) + (b.y - a.y) * (c.y - b.y) < 0.0
        {
            return false;
        }
        for j in (i + 1)..n {
            if j == i + 1 || (i == 0 && j == n - 1) {
                continue;
            }
            let (c, d) = (points[j], points[(j + 1) % n]);
            if on(a, b, c)
                || on(a, b, d)
                || on(c, d, a)
                || on(c, d, b)
                || (cross(a, b, c) * cross(a, b, d) < 0.0 && cross(c, d, a) * cross(c, d, b) < 0.0)
            {
                return false;
            }
        }
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rejects_crossings_touches_and_overlaps() {
        let ring = |pairs: &[(f64, f64)]| {
            pairs
                .iter()
                .map(|&(x, y)| Point { x, y })
                .collect::<Vec<_>>()
        };
        assert!(!is_simple_ring(&ring(&[
            (0., 0.),
            (4., 4.),
            (0., 4.),
            (3., 0.)
        ])));
        assert!(!is_simple_ring(&ring(&[
            (0., 0.),
            (4., 0.),
            (2., 0.),
            (2., 4.)
        ])));
        assert!(!is_simple_ring(&ring(&[
            (0., 0.),
            (4., 0.),
            (4., 4.),
            (0., 0.)
        ])));
        assert!(is_simple_ring(&ring(&[
            (0., 0.),
            (4., 0.),
            (2., 2.),
            (4., 4.),
            (0., 4.)
        ])));
        assert!(crate::Config::default().validate().is_ok());
    }
}
