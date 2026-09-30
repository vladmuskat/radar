//! Integration checks use only the public facade, independently of module layout.
use radar_core::{Config, Engine, Status, STEP_MS};

#[test]
fn paused_ticks_do_not_change_future_scenario() {
    for seed in [0, 2401, u32::MAX] {
        let config = Config {
            seed,
            duration_seconds: 30,
            ..Config::default()
        };
        let mut control = Engine::new(config.clone(), "contract".into()).unwrap();
        let mut paused = Engine::new(config, "contract".into()).unwrap();
        control.start().unwrap();
        paused.start().unwrap();
        for step in 0..600 {
            if step % 73 == 0 {
                paused.pause().unwrap();
                let before = serde_json::to_value(paused.snapshot()).unwrap();
                for _ in 0..100 {
                    paused.tick();
                }
                assert_eq!(before, serde_json::to_value(paused.snapshot()).unwrap());
                paused.resume().unwrap();
            }
            control.tick();
            paused.tick();
            let mut expected = serde_json::to_value(control.snapshot()).unwrap();
            let mut actual = serde_json::to_value(paused.snapshot()).unwrap();
            // Pause/resume are real commands and legitimately increment sequence.
            expected.as_object_mut().unwrap().remove("sequence");
            actual.as_object_mut().unwrap().remove("sequence");
            assert_eq!(actual, expected, "seed={seed}, step={step}");
        }
        assert_eq!(paused.status, Status::Finished);
        assert_eq!(paused.time_ms, 600 * STEP_MS);
    }
}

#[test]
fn exam_hides_scoring_until_finish_and_finish_is_idempotent() {
    let mut engine = Engine::new(
        Config {
            exam: true,
            ..Config::default()
        },
        "exam".into(),
    )
    .unwrap();
    assert!(engine.pause().is_err());
    assert!(engine.resume().is_err());
    engine.start().unwrap();
    assert!(engine.start().is_err());
    for _ in 0..80 {
        engine.tick();
    }
    let id = engine.snapshot().targets[0].id;
    engine.identify(id).unwrap();
    let during = engine.snapshot();
    assert_eq!(during.statistics.marked, 1);
    assert_eq!(during.statistics.correct + during.statistics.errors, 0);
    assert_eq!(during.statistics.missed, 0);
    engine.finish();
    let finished = engine.snapshot();
    assert_eq!(finished.statistics.correct + finished.statistics.errors, 1);
    let before = serde_json::to_value(finished).unwrap();
    engine.finish();
    engine.tick();
    assert_eq!(before, serde_json::to_value(engine.snapshot()).unwrap());
}
