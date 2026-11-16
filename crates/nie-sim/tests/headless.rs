use nie_model::Rules;
use nie_sim::{MatchConfig, run_headless};

fn config(seed: u64) -> MatchConfig {
    MatchConfig::generated(Rules {
        seed,
        ..Rules::default()
    })
}

#[test]
fn generated_config_is_valid() {
    config(7).validate().unwrap();
}

/// The vertical slice: a 5v5 match, 3600 ticks (one minute at 60 Hz), headless. The score and
/// the world checksum are pinned; a change means the simulation is no longer the same game.
#[test]
fn five_v_five_full_match_is_pinned() {
    let report = run_headless(config(7), None);
    assert!(report.finished);
    assert_eq!(report.ticks, 3600);
    assert_eq!(report.score_line(), "2-4");
    assert_eq!(report.checksum, 11_564_882_983_167_120_720);
}

#[test]
fn replays_are_identical() {
    assert_eq!(
        run_headless(config(3), Some(1200)),
        run_headless(config(3), Some(1200))
    );
}

#[test]
fn seeds_give_different_matches() {
    assert_ne!(
        run_headless(config(3), Some(1200)).checksum,
        run_headless(config(4), Some(1200)).checksum
    );
}

#[test]
fn cap_stops_the_clock() {
    let report = run_headless(config(7), Some(100));
    assert_eq!(report.ticks, 100);
    assert!(!report.finished);
}
