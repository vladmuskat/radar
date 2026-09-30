//! Deterministic domain without UI, storage or wall-clock dependencies.
//! Call Engine::tick at fixed 50 ms steps; pause freezes simulation time.

mod config;
mod engine;
mod geometry;
mod snapshot;
mod zones;

pub use config::Config;
pub use engine::Engine;
pub use geometry::{contains, Point};
pub use snapshot::{Notice, Snapshot, Statistics, Status, TargetView};
pub use zones::{default_zones, Zone, ZoneKind};

pub const STEP_MS: u64 = 50;
/// Prototype antenna rotation period; measured in simulation time.
pub const SWEEP_PERIOD_MS: u64 = 4_000;
pub const RADAR_RANGE_M: f64 = 7_000.0;
pub const ORIGIN_LAT: f64 = 59.88;
pub const ORIGIN_LON: f64 = 30.26;
