//! Visibility and zone-entry notifications.
use super::sweep;
use super::{Engine, Target};
use crate::{contains, Notice, Zone, ZoneKind, RADAR_RANGE_M};
impl Engine {
    /// Applies radar range and ignore-zone visibility rules to a live target.
    pub(super) fn visible(zones: &[Zone], t: &Target) -> bool {
        t.position.distance() <= RADAR_RANGE_M
            && !zones
                .iter()
                .any(|z| z.kind == ZoneKind::Ignore && contains(&z.points, t.position))
    }
    /// Processes sweep hits, trails, zone notifications and scheduled removal.
    pub(super) fn update_zones(&mut self) {
        let mut destroyed = Vec::new();
        for t in &mut self.targets {
            if t.identified && sweep::crosses(self.time_ms, t.position) {
                // A double click schedules the action; the contact remains on
                // screen until the antenna reaches its current true position.
                destroyed.push(t.id);
                continue;
            }
            let visible = Self::visible(&self.config.zones, t);
            if !visible {
                t.active_zones.clear();
                t.trail.clear();
                t.observed_position = None;
                continue;
            }
            // No observation before the first pass; no hidden live coordinates in the DTO.
            if t.observed_at_ms == self.time_ms || !sweep::crosses(self.time_ms, t.position) {
                continue;
            }
            t.visible_since.get_or_insert(self.time_ms);
            t.observed_position = Some(t.position);
            t.observed_at_ms = self.time_ms;
            t.trail.push(t.position);
            if t.trail.len() > 40 {
                t.trail.remove(0);
            }
            let active: Vec<_> = self
                .config
                .zones
                .iter()
                .filter(|z| z.kind == ZoneKind::Detection && contains(&z.points, t.position))
                .collect();
            for z in &active {
                if !t.active_zones.contains(&z.id) {
                    self.notices.push(Notice {
                        id: self.next_notice,
                        target_id: t.id,
                        zone_name: z.name.clone(),
                        time_ms: self.time_ms,
                        position: t.position,
                        speed_mps: t.speed,
                    });
                    self.next_notice += 1;
                }
            }
            t.active_zones = active.iter().map(|z| z.id.clone()).collect();
        }
        if !destroyed.is_empty() {
            self.targets
                .retain(|target| !destroyed.contains(&target.id));
        }
        if self.notices.len() > 200 {
            self.notices.drain(..self.notices.len() - 200);
        }
    }
}
