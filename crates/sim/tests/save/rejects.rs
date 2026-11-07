//! Damaged, truncated and incompatible saves fail instead of loading a wrong city.
use sim::{Config, SaveError, World};

fn bytes() -> Vec<u8> {
    let mut w = World::generate(Config::default(), 4, 4);
    w.apply(sim::Command::SpawnAgents { count: 40 });
    for _ in 0..200 {
        w.tick();
    }
    w.save().unwrap()
}

#[test]
fn empty_and_foreign_bytes_are_not_saves() {
    assert!(matches!(World::load(&[]), Err(SaveError::NotASave)));
    assert!(matches!(
        World::load(b"{\"city\": true}"),
        Err(SaveError::NotASave)
    ));
    let mut save = bytes();
    save[0] ^= 0xff;
    assert!(matches!(World::load(&save), Err(SaveError::NotASave)));
}

#[test]
fn every_truncation_fails() {
    let save = bytes();
    let step = (save.len() / 400).max(1);
    for len in (0..save.len()).step_by(step).chain([save.len() - 1]) {
        let err = World::load(&save[..len]).err();
        assert!(
            err.is_some(),
            "loaded a save cut to {len} of {}",
            save.len()
        );
    }
}

#[test]
fn trailing_bytes_fail() {
    let mut save = bytes();
    save.push(0);
    assert!(World::load(&save).is_err());
}

#[test]
fn flipped_bytes_are_detected() {
    let save = bytes();
    let step = (save.len() / 300).max(1);
    for at in (0..save.len()).step_by(step) {
        let mut bad = save.clone();
        bad[at] ^= 0x10;
        assert!(World::load(&bad).is_err(), "byte {at} flipped yet loaded");
    }
}

#[test]
fn other_schema_versions_are_unsupported() {
    let save = bytes();
    // The schema version follows the 8-byte format tag, little-endian. Versions 1–3
    // predate the combined money, pollution and power state, version 4 water (ADR 0052) and
    // version 5 fire (ADR 0053), version 6 children (ADR 0054), version 7 the edit history
    // (ADR 0051).
    for version in [0u32, 1, 2, 3, 4, 5, 6, 7, 9, 99, u32::MAX] {
        let mut other = save.clone();
        other[8..12].copy_from_slice(&version.to_le_bytes());
        assert!(
            matches!(World::load(&other), Err(SaveError::UnsupportedVersion(v)) if v == version),
            "version {version}"
        );
    }
}

#[test]
fn errors_explain_themselves() {
    let Err(err) = World::load(&[]) else {
        panic!("loaded nothing");
    };
    assert!(!err.to_string().is_empty());
    let _: &dyn std::error::Error = &err;
}
