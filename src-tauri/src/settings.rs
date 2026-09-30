//! Validated application preferences. Missing fields support older SQLite records.
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default, deny_unknown_fields, rename_all = "camelCase")]
pub struct Settings {
    pub direction: bool,
    pub trails: bool,
    pub sound: bool,
    pub volume: u8,
    pub archive: bool,
    pub home: [f64; 2],
    pub zoom: f64,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Region {
    center: [f64; 2],
    bbox: [f64; 4],
    default_zoom: f64,
    min_zoom: f64,
    max_zoom: f64,
}
/// Reads the compile-time map bounds used to validate persisted preferences.
fn region() -> Region {
    serde_json::from_str(include_str!("../../map-region.json"))
        .expect("Embedded map-region.json must be valid")
}
impl Default for Settings {
    fn default() -> Self {
        let region = region();
        Self {
            direction: false,
            trails: true,
            sound: false,
            volume: 30,
            archive: true,
            home: region.center,
            zoom: region.default_zoom,
        }
    }
}
impl Settings {
    /// Checks volume, coordinates and zoom at the Rust trust boundary.
    pub fn validate(&self) -> Result<(), String> {
        if self.volume > 100 {
            return Err("Громкость: от 0 до 100".into());
        }
        let r = region();
        if !self.home.iter().all(|n| n.is_finite())
            || !(r.bbox[0]..=r.bbox[2]).contains(&self.home[0])
            || !(r.bbox[1]..=r.bbox[3]).contains(&self.home[1])
        {
            return Err("Домашняя позиция должна находиться в пределах локальной карты".into());
        }
        if !self.zoom.is_finite() || !(r.min_zoom..=r.max_zoom).contains(&self.zoom) {
            return Err("Масштаб вне допустимого диапазона карты".into());
        }
        Ok(())
    }
    /// Restores compatible JSON and falls back to defaults on invalid records.
    pub fn restore(text: &str) -> Self {
        match serde_json::from_str::<Self>(text) {
            Ok(value) if value.validate().is_ok() => value,
            _ => {
                tracing::warn!(
                    event = "invalid_saved_settings",
                    "Using default preferences"
                );
                Self::default()
            }
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn defaults_old_records_and_invalid_values() {
        assert_eq!(Settings::restore("{}"), Settings::default());
        assert!(!Settings::restore(r#"{"sound":true}"#).direction);
        assert!(Settings::restore(r#"{"sound":true}"#).sound);
        for input in [
            r#"{"volume":999}"#,
            r#"{"sound":"yes"}"#,
            r#"{"home":[0,0]}"#,
            r#"{"zoom":100}"#,
            "null",
            "broken",
        ] {
            assert_eq!(Settings::restore(input), Settings::default());
        }
        let bad = Settings {
            volume: 101,
            ..Settings::default()
        };
        assert!(bad.validate().is_err());
        assert!(serde_json::from_str::<Settings>(r#"{"sound":"yes"}"#).is_err());
    }
}
