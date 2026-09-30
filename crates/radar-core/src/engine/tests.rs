use super::*;
use crate::contains;
fn engine() -> Engine {
    let mut e = Engine::new(Config::default(), "test".into()).unwrap();
    e.start().unwrap();
    e
}
#[test]
fn pause_freezes_everything_and_resume_advances_one_step() {
    let mut e = engine();
    for _ in 0..100 {
        e.tick();
    }
    e.pause().unwrap();
    let before = serde_json::to_string(&e.snapshot()).unwrap();
    for _ in 0..600 {
        e.tick();
    }
    assert_eq!(before, serde_json::to_string(&e.snapshot()).unwrap());
    assert!(e.identify(e.targets[0].id).is_err());
    e.resume().unwrap();
    e.tick();
    assert_eq!(e.time_ms, 5050);
}
#[test]
fn deterministic_seed_and_no_truth_in_dto() {
    let (mut a, mut b) = (engine(), engine());
    for _ in 0..700 {
        a.tick();
        b.tick();
    }
    let s = serde_json::to_string(&a.snapshot()).unwrap();
    assert_eq!(s, serde_json::to_string(&b.snapshot()).unwrap());
    assert!(!s.contains("drone"));
    assert!(!s.contains("bird"));
}
#[test]
fn generation_obeys_tz_and_capacity() {
    let mut e = engine();
    for _ in 0..2400 {
        for t in &e.targets {
            match t.kind {
                Kind::Drone => {
                    assert!((25.0..=35.0).contains(&t.speed));
                    assert!(t.life_ms > 10_000);
                }
                Kind::Bird => {
                    assert!((2.0..=10.0).contains(&t.speed));
                    assert!((5_000..=20_000).contains(&t.life_ms));
                }
            }
        }
        assert!(e.targets.len() <= 20);
        e.tick();
    }
    assert_eq!(e.status, Status::Finished);
    assert_eq!(e.time_ms, 120_000);
}

#[test]
fn birds_turn_back_at_the_radar_edge() {
    let mut e = engine();
    e.config.duration_seconds = 600;
    e.config.max_targets = 1;
    e.targets.truncate(1);
    let bird = &mut e.targets[0];
    bird.kind = Kind::Bird;
    bird.position = Point { x: 6799.0, y: 0.0 };
    bird.heading = 0.0;
    bird.life_ms = 600_000;
    let id = bird.id;
    for _ in 0..6000 {
        e.tick();
    }
    let bird = e.targets.iter().find(|target| target.id == id).unwrap();
    assert!(bird.position.distance() <= 6800.0);
}

#[test]
fn birds_expire_and_leave_room_for_new_targets() {
    let mut e = engine();
    e.targets.truncate(1);
    e.config.max_targets = 1;
    let bird = &mut e.targets[0];
    bird.kind = Kind::Bird;
    bird.life_ms = 100;
    let id = bird.id;
    e.tick();
    assert!(e.targets.iter().any(|target| target.id == id));
    e.tick();
    assert!(!e.targets.iter().any(|target| target.id == id));
    while !e.time_ms.is_multiple_of(750) {
        e.tick();
    }
    assert_eq!(e.targets.len(), 1);
    assert_ne!(e.targets[0].id, id);
}

#[test]
fn marked_target_is_removed_only_on_the_next_beam_pass() {
    let mut e = engine();
    e.targets.truncate(1);
    e.config.max_targets = 1;
    let target = &mut e.targets[0];
    target.kind = Kind::Drone;
    target.position = Point { x: 0.0, y: 5000.0 };
    target.speed = 0.0;
    target.life_ms = 80_000;
    let id = target.id;
    e.tick();
    e.identify(id).unwrap();
    assert!(e.snapshot().targets.iter().any(|target| target.id == id));
    for _ in 0..79 {
        e.tick();
    }
    assert!(e.snapshot().targets.iter().any(|target| target.id == id));
    e.tick();
    assert!(!e.targets.iter().any(|target| target.id == id));
}
#[test]
fn identifying_once_and_no_alert_spam() {
    let mut e = engine();
    for _ in 0..80 {
        e.tick();
    }
    let id = e.snapshot().targets[0].id;
    e.identify(id).unwrap();
    assert!(e.identify(id).is_err());
    assert_eq!(e.snapshot().statistics.marked, 1);
    let notices = e.notices.len();
    e.tick();
    assert_eq!(notices, e.notices.len());
}
#[test]
fn ignore_overrides_detection_and_reentry_alerts() {
    let mut e = engine();
    e.targets.truncate(1);
    e.targets[0].position = Point {
        x: 5000.0,
        y: 1000.0,
    };
    e.time_ms = 900;
    e.update_zones();
    let n = e.notices.len();
    e.update_zones();
    assert_eq!(n, e.notices.len());
    e.targets[0].position = Point { x: 0.0, y: 0.0 };
    e.update_zones();
    assert!(e.snapshot().targets.is_empty());
    assert_eq!(n, e.notices.len());
    e.targets[0].position = Point {
        x: 5000.0,
        y: 1000.0,
    };
    e.time_ms += 4000;
    e.update_zones();
    assert_eq!(n + 1, e.notices.len());
}
#[test]
fn input_validation_and_polygon_boundary() {
    let c = Config {
        max_targets: 21,
        ..Config::default()
    };
    assert!(c.validate().is_err());
    let p = vec![
        Point { x: 0.0, y: 0.0 },
        Point { x: 10.0, y: 0.0 },
        Point { x: 10.0, y: 10.0 },
        Point { x: 0.0, y: 10.0 },
    ];
    assert!(contains(&p, Point { x: 10.0, y: 5.0 }));
    assert!(!contains(&p, Point { x: 11.0, y: 5.0 }));
}
#[test]
fn drone_motion_straight_and_reaction_excludes_pause() {
    let mut e = engine();
    for _ in 0..80 {
        e.tick();
    }
    let t = e
        .targets
        .iter()
        .find(|t| {
            t.kind == Kind::Drone
                && t.observed_position.is_some()
                && Engine::visible(&e.config.zones, t)
        })
        .unwrap()
        .clone();
    for _ in 0..20 {
        e.tick();
    }
    let moved = e.targets.iter().find(|x| x.id == t.id).unwrap();
    assert!(
        ((moved.position.x - t.position.x).hypot(moved.position.y - t.position.y) - t.speed).abs()
            < 1e-6
    );
    e.pause().unwrap();
    for _ in 0..200 {
        e.tick();
    }
    e.resume().unwrap();
    e.identify(t.id).unwrap();
    assert_eq!(
        e.statistics.average_reaction_ms,
        e.time_ms - t.visible_since.unwrap()
    );
}

#[test]
fn contacts_and_trails_only_update_when_beam_passes() {
    let mut e = engine();
    e.targets.truncate(1);
    e.config.max_targets = 1;
    let t = &mut e.targets[0];
    t.kind = Kind::Drone;
    t.position = Point { x: 0.0, y: 5000.0 };
    t.speed = 30.0;
    t.heading = -std::f64::consts::FRAC_PI_2;
    t.life_ms = 80_000;
    let id = t.id;
    assert!(e.snapshot().targets.is_empty());
    assert!(e.notices.is_empty());
    assert!(e.identify(id).is_err());
    e.tick();
    let first = e.snapshot().targets[0].clone();
    assert_eq!(first.observed_at_ms, 50);
    assert_eq!(first.trail.len(), 1);
    assert_eq!(e.notices.len(), 1);
    for _ in 0..79 {
        e.tick();
    }
    let waiting = e.snapshot().targets[0].clone();
    assert_eq!(waiting.position, first.position);
    assert_eq!(waiting.trail, first.trail);
    assert_ne!(e.targets[0].position, waiting.position);
    e.tick();
    let second = e.snapshot().targets[0].clone();
    assert_ne!(second.position, first.position);
    assert_eq!(second.observed_at_ms, 4050);
    assert_eq!(second.trail, vec![first.position, second.position]);
    assert_eq!(e.notices.len(), 1);
}
