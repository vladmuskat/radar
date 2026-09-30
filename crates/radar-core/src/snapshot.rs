//! Public data contract: target truth is never included.
use crate::{Config, Point};
use serde::{Deserialize, Serialize};
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub enum Status {
    Ready,
    Running,
    Paused,
    Finished,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TargetView {
    pub id: u64,
    pub position: Point,
    pub longitude: f64,
    pub latitude: f64,
    pub speed_mps: f64,
    #[serde(default)]
    pub observed_at_ms: u64,
    pub identified: bool,
    pub trail: Vec<Point>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Notice {
    pub id: u64,
    pub target_id: u64,
    pub zone_name: String,
    pub time_ms: u64,
    pub position: Point,
    pub speed_mps: f64,
}
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Statistics {
    pub marked: u32,
    pub correct: u32,
    pub errors: u32,
    pub missed: u32,
    pub average_reaction_ms: u64,
    pub reactions_ms: Vec<u64>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Snapshot {
    pub session_id: String,
    pub sequence: u64,
    pub status: Status,
    pub simulation_time_ms: u64,
    /// Clockwise radians from north, driven exclusively by simulation time.
    #[serde(default)]
    pub sweep_bearing: f64,
    pub remaining_time_ms: u64,
    pub targets: Vec<TargetView>,
    pub notifications: Vec<Notice>,
    pub statistics: Statistics,
    pub config: Config,
}
