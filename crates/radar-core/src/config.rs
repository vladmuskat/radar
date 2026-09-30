//! Scenario parameters and validation.
use crate::{default_zones, Zone};
use serde::{Deserialize, Serialize};
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Config {
    pub duration_seconds: u32,
    pub max_targets: usize,
    pub seed: u32,
    pub exam: bool,
    pub zones: Vec<Zone>,
}
impl Default for Config {
    fn default() -> Self {
        Self {
            duration_seconds: 120,
            max_targets: 12,
            seed: 2401,
            exam: false,
            zones: default_zones(),
        }
    }
}
impl Config {
    /// Rejects scenario parameters and zone geometry outside supported limits.
    pub fn validate(&self) -> Result<(), String> {
        if !(30..=3600).contains(&self.duration_seconds) {
            return Err("Длительность: от 30 до 3600 секунд".into());
        }
        if !(1..=20).contains(&self.max_targets) {
            return Err("Количество целей: от 1 до 20".into());
        }
        if self.zones.len() > 20 {
            return Err("Допускается до 20 зон".into());
        }
        let mut ids = std::collections::HashSet::new();
        for z in &self.zones {
            if z.name.trim().is_empty()
                || z.name.chars().count() > 60
                || !ids.insert(&z.id)
                || z.id.is_empty()
            {
                return Err("У зон должны быть уникальные ID и названия до 60 символов".into());
            }
            if !(3..=100).contains(&z.points.len())
                || z.points
                    .iter()
                    .any(|p| !p.x.is_finite() || !p.y.is_finite() || p.distance() > 15000.0)
            {
                return Err("Зона: от 3 до 100 вершин в пределах 15 км".into());
            }
            if !crate::geometry::is_simple_ring(&z.points) {
                return Err("Граница зоны не должна пересекаться, касаться себя или содержать повторные вершины".into());
            }
            let area: f64 = z
                .points
                .iter()
                .zip(z.points.iter().cycle().skip(1))
                .take(z.points.len())
                .map(|(a, b)| a.x * b.y - b.x * a.y)
                .sum();
            if area.abs() < 100.0 {
                return Err("Площадь зоны слишком мала".into());
            }
        }
        Ok(())
    }
}
