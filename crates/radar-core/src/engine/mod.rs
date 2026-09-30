//! Session lifecycle, fixed-step movement and scoring.
mod detection;
mod generation;
mod random;
mod sweep;
#[cfg(test)]
mod tests;
use crate::{
    Config, Notice, Point, Snapshot, Statistics, Status, TargetView, RADAR_RANGE_M, STEP_MS,
};
use random::Random;
#[derive(Debug, Clone, Copy, PartialEq)]
enum Kind {
    Drone,
    Bird,
}
#[derive(Debug, Clone)]
struct Target {
    id: u64,
    kind: Kind,
    position: Point,
    speed: f64,
    heading: f64,
    turn: f64,
    born_ms: u64,
    life_ms: u64,
    identified: bool,
    visible_since: Option<u64>,
    observed_position: Option<Point>,
    observed_at_ms: u64,
    trail: Vec<Point>,
    active_zones: Vec<String>,
}

pub struct Engine {
    pub config: Config,
    pub status: Status,
    pub time_ms: u64,
    session_id: String,
    sequence: u64,
    rng: Random,
    targets: Vec<Target>,
    notices: Vec<Notice>,
    statistics: Statistics,
    next_id: u64,
    next_notice: u64,
}
impl Engine {
    /// Creates a validated ready session with deterministic random state.
    pub fn new(config: Config, session_id: String) -> Result<Self, String> {
        config.validate()?;
        Ok(Self {
            rng: Random(config.seed as u64 + 1),
            config,
            status: Status::Ready,
            time_ms: 0,
            session_id,
            sequence: 0,
            targets: vec![],
            notices: vec![],
            statistics: Statistics::default(),
            next_id: 1,
            next_notice: 1,
        })
    }
    /// Starts a ready session and creates its initial target population.
    pub fn start(&mut self) -> Result<(), String> {
        if self.status != Status::Ready {
            return Err("Сеанс уже запущен".into());
        }
        self.status = Status::Running;
        while self.targets.len() < self.config.max_targets {
            self.spawn();
        }
        self.update_zones();
        self.sequence += 1;
        Ok(())
    }
    /// Freezes simulation time; repeated pause commands are idempotent.
    pub fn pause(&mut self) -> Result<(), String> {
        if self.status == Status::Paused {
            return Ok(());
        }
        if self.status != Status::Running {
            return Err("Тренировка не запущена".into());
        }
        self.status = Status::Paused;
        self.sequence += 1;
        Ok(())
    }
    /// Continues a previously paused session.
    pub fn resume(&mut self) -> Result<(), String> {
        if self.status == Status::Running {
            return Ok(());
        }
        if self.status != Status::Paused {
            return Err("Тренировка не на паузе".into());
        }
        self.status = Status::Running;
        self.sequence += 1;
        Ok(())
    }
    /// Advances movement, detection and lifetime by one fixed 50 ms step.
    pub fn tick(&mut self) {
        if self.status != Status::Running {
            return;
        }
        self.time_ms += STEP_MS;
        for t in &mut self.targets {
            if t.kind == Kind::Bird {
                t.heading +=
                    (t.turn + 0.3 * ((self.time_ms - t.born_ms) as f64 / 1200.0).sin()) * 0.05;
                let next = Point {
                    x: t.position.x + t.speed * t.heading.cos() * 0.05,
                    y: t.position.y + t.speed * t.heading.sin() * 0.05,
                };
                if next.distance() > 6800.0 {
                    // Smoothly turn back towards the radar instead of allowing a
                    // bird to leave the display and be replaced by a new one.
                    t.heading = (-t.position.y).atan2(-t.position.x) + t.turn * 0.35;
                }
            }
            t.position.x += t.speed * t.heading.cos() * 0.05;
            t.position.y += t.speed * t.heading.sin() * 0.05;
        }
        self.update_zones();
        self.targets.retain(|t| {
            let keep = t.identified
                || (self.time_ms - t.born_ms < t.life_ms && t.position.distance() <= RADAR_RANGE_M);
            if !keep && t.kind == Kind::Drone && !t.identified && t.visible_since.is_some() {
                self.statistics.missed += 1;
            }
            keep
        });
        if self.time_ms >= self.config.duration_seconds as u64 * 1000 {
            self.finish();
        } else if self.time_ms.is_multiple_of(750) {
            while self.targets.len() < self.config.max_targets {
                self.spawn();
            }
            self.update_zones();
        }
        self.sequence += 1;
    }
    /// Scores an operator decision and schedules removal on the next sweep.
    pub fn identify(&mut self, id: u64) -> Result<(), String> {
        if self.status != Status::Running {
            return Err("Распознавание доступно только во время тренировки".into());
        }
        let t = self
            .targets
            .iter_mut()
            .find(|t| t.id == id)
            .ok_or("Цель уже покинула область наблюдения")?;
        if t.identified {
            return Err("Цель уже отмечена".into());
        }
        if t.observed_position.is_none() || !Self::visible(&self.config.zones, t) {
            return Err("Цель ещё не обнаружена или скрыта зоной игнорирования".into());
        }
        t.identified = true;
        self.statistics.marked += 1;
        if t.kind == Kind::Drone {
            self.statistics.correct += 1;
        } else {
            self.statistics.errors += 1;
        }
        self.statistics
            .reactions_ms
            .push(self.time_ms - t.visible_since.unwrap_or(self.time_ms));
        self.statistics.average_reaction_ms =
            self.statistics.reactions_ms.iter().sum::<u64>() / self.statistics.marked as u64;
        self.sequence += 1;
        Ok(())
    }
    /// Finalizes scoring once and moves the session to its terminal state.
    pub fn finish(&mut self) {
        if self.status == Status::Finished {
            return;
        }
        for t in &self.targets {
            if t.kind == Kind::Drone && !t.identified && t.visible_since.is_some() {
                self.statistics.missed += 1;
            }
        }
        self.status = Status::Finished;
        self.sequence += 1;
    }
    /// Returns the public observed state without target truth or live position.
    pub fn snapshot(&self) -> Snapshot {
        let mut statistics = self.statistics.clone();
        if self.config.exam && self.status != Status::Finished {
            statistics.correct = 0;
            statistics.errors = 0;
            statistics.missed = 0;
        }
        Snapshot {
            session_id: self.session_id.clone(),
            sequence: self.sequence,
            status: self.status,
            simulation_time_ms: self.time_ms,
            sweep_bearing: sweep::bearing(self.time_ms),
            remaining_time_ms: (self.config.duration_seconds as u64 * 1000)
                .saturating_sub(self.time_ms),
            targets: self
                .targets
                .iter()
                .filter(|t| Self::visible(&self.config.zones, t))
                .filter_map(|t| {
                    let position = t.observed_position?;
                    let [longitude, latitude] = position.geographic();
                    Some(TargetView {
                        id: t.id,
                        position,
                        longitude,
                        latitude,
                        speed_mps: t.speed,
                        observed_at_ms: t.observed_at_ms,
                        identified: t.identified,
                        trail: t.trail.clone(),
                    })
                })
                .collect(),
            notifications: self.notices.clone(),
            statistics,
            config: self.config.clone(),
        }
    }
}
