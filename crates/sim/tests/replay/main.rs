//! Replay regression tests (ADR 0034): each `*.cmdlog` in `tests/replays` is a saved session.
//! Replaying it must reach its `expect` line, and a save and load halfway through must not
//! change where it ends.
//!
//! After an intended gameplay change, review the differences and then refresh the expectations:
//! `BLESS_REPLAYS=1 cargo test -p sim --test replay`.

use std::fs;
use std::path::PathBuf;

use sim::CommandLog;
use sim::World;
use sim::replay::Outcome;

fn logs() -> Vec<PathBuf> {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/replays");
    let mut paths: Vec<_> = fs::read_dir(&dir)
        .expect("tests/replays exists")
        .map(|e| e.expect("readable entry").path())
        .filter(|p| p.extension().is_some_and(|e| e == "cmdlog"))
        .collect();
    paths.sort();
    assert!(!paths.is_empty(), "no command logs in {}", dir.display());
    paths
}

#[test]
fn saved_sessions_replay_to_their_expected_cities() {
    let bless = std::env::var_os("BLESS_REPLAYS").is_some();
    let mut failures = Vec::new();
    for path in logs() {
        let name = path.file_name().unwrap().to_string_lossy().into_owned();
        let text = fs::read_to_string(&path).expect("readable log");
        let mut log = CommandLog::parse(&text).unwrap_or_else(|e| panic!("{name}: {e}"));
        let reached = Outcome::of(&log.replay());

        let mut world = log.world();
        let next = log.advance(&mut world, 0, log.end / 2);
        let mut loaded = World::load(&world.save().expect("saves")).expect("loads");
        log.advance(&mut loaded, next, log.end);
        assert_eq!(
            Outcome::of(&loaded),
            reached,
            "{name}: a save and load at tick {} changed the replay",
            log.end / 2
        );

        if name == "scripted-fire.cmdlog" {
            let s = loaded.fire_stats();
            assert!(s.saved >= 1 && s.destroyed >= 1, "{name}: {s:?}");
            assert!(!s.enabled, "{name}: the log turns fires off");
        }

        if bless {
            if log.expect != Some(reached) {
                log.expect = Some(reached);
                let comments: String = text
                    .lines()
                    .take_while(|l| l.starts_with('#'))
                    .map(|l| format!("{l}\n"))
                    .collect();
                fs::write(&path, format!("{comments}{log}")).expect("writable log");
            }
        } else if log.expect != Some(reached) {
            failures.push(format!(
                "{name}: expected {:?}, reached {reached:?}",
                log.expect
            ));
        }
    }
    assert!(
        failures.is_empty(),
        "replays changed; if intended, rerun with BLESS_REPLAYS=1:\n{}",
        failures.join("\n")
    );
}
