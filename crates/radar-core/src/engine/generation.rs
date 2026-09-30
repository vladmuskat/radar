//! Seeded generation; RNG call order preserves repeatability.
use super::{Engine, Kind, Target};
use crate::Point;
use std::f64::consts::TAU;
impl Engine {
    /// Adds one seeded drone or bird with parameters allowed by the scenario.
    pub(super) fn spawn(&mut self) {
        let kind = if self.rng.unit() < 0.5 {
            Kind::Drone
        } else {
            Kind::Bird
        };
        let angle = self.rng.between(0.0, TAU);
        let radius = if kind == Kind::Drone {
            self.rng.between(3100.0, 6900.0)
        } else {
            self.rng.between(300.0, 6800.0)
        };
        let speed = if kind == Kind::Drone {
            self.rng.between(25.0, 35.0)
        } else {
            self.rng.between(2.0, 10.0)
        };
        let heading = if kind == Kind::Drone {
            angle + TAU / 2.0
        } else {
            self.rng.between(0.0, TAU)
        };
        let life_ms = if kind == Kind::Drone {
            self.rng.between(30_000.0, 80_000.0) as u64
        } else {
            self.rng.between(5_000.0, 20_000.0) as u64
        };
        let position = Point {
            x: radius * angle.cos(),
            y: radius * angle.sin(),
        };
        self.targets.push(Target {
            id: self.next_id,
            kind,
            position,
            speed,
            heading,
            turn: self.rng.between(-0.65, 0.65),
            born_ms: self.time_ms,
            life_ms,
            identified: false,
            visible_since: None,
            observed_position: None,
            observed_at_ms: 0,
            trail: vec![],
            active_zones: vec![],
        });
        self.next_id += 1;
    }
}
