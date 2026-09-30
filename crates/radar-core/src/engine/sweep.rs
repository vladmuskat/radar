//! Discrete antenna scan: half-open swept sectors avoid duplicate boundary hits.
use crate::{Point, STEP_MS, SWEEP_PERIOD_MS};
use std::f64::consts::TAU;

/// Returns clockwise antenna bearing in radians for simulation time.
pub(super) fn bearing(time_ms: u64) -> f64 {
    (time_ms % SWEEP_PERIOD_MS) as f64 / SWEEP_PERIOD_MS as f64 * TAU
}

/// Tests whether the current discrete sweep sector contains a target.
pub(super) fn crosses(time_ms: u64, position: Point) -> bool {
    if time_ms == 0 {
        return false;
    }
    let target = position.x.atan2(position.y).rem_euclid(TAU);
    let previous = bearing(time_ms.saturating_sub(STEP_MS));
    let offset = (target - previous).rem_euclid(TAU);
    // Trigonometric roundoff at north must not put one contact in both adjacent sectors.
    let epsilon = 1e-10;
    let offset = if TAU - offset < epsilon { 0.0 } else { offset };
    offset < STEP_MS as f64 / SWEEP_PERIOD_MS as f64 * TAU - epsilon
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sweep_rotates_clockwise_and_handles_north_wrap() {
        assert!(!crosses(0, Point { x: 0.0, y: 1000.0 }));
        assert!(crosses(50, Point { x: 0.0, y: 1000.0 }));
        assert!(!crosses(100, Point { x: 0.0, y: 1000.0 }));
        assert!(crosses(1050, Point { x: 1000.0, y: 0.0 }));
        assert!(crosses(4000, Point { x: -1.0, y: 1000.0 }));
        assert!(crosses(4050, Point { x: 0.0, y: 1000.0 }));
        assert_eq!(bearing(4000), 0.0);
    }
}
