//! Every command kind, and every plant, water piece, zone, road class and fires setting a
//! command can carry, appears in at least one saved session. A new command variant fails to
//! compile here until it gets a name in `KINDS`, and then fails the test until a log uses it.

use std::collections::BTreeSet;

use sim::Command;
use sim::CommandLog;
use sim::Zone;
use sim::power::PlantKind;
use sim::road::RoadClass;
use sim::water::WaterKind;

const KINDS: [&str; 15] = [
    "spawn", "road", "curve", "roadgrid", "class", "delete", "plant", "water", "line", "station",
    "fires", "paint", "undo", "redo", "fill",
];

/// The command's word in `KINDS`, then the option it carries, if any.
fn words(c: &Command) -> (usize, Option<String>) {
    let option = |o: &dyn std::fmt::Debug| Some(format!("{o:?}"));
    match c {
        Command::SpawnAgents { .. } => (0, None),
        Command::BuildRoad { class, .. } => (1, option(class)),
        Command::BuildCurve { class, .. } => (2, option(class)),
        Command::BuildGrid { class, .. } => (3, option(class)),
        Command::SetRoadClass { class, .. } => (4, option(class)),
        Command::DeleteRoad { .. } => (5, None),
        Command::PlacePlant { kind, .. } => (6, option(kind)),
        Command::PlaceWater { kind, .. } => (7, option(kind)),
        Command::BuildLine { .. } => (8, None),
        Command::PlaceStation { .. } => (9, None),
        Command::SetFires { on } => (10, Some(format!("fires {on}"))),
        Command::PaintZone { zone, .. } => (11, option(zone)),
        Command::Undo => (12, None),
        Command::Redo => (13, None),
        Command::FillBlock { zone, .. } => (14, option(zone)),
    }
}

/// Every option a command can carry, read from each enum's own numbering.
fn every_option() -> BTreeSet<String> {
    let all = |f: &dyn Fn(u8) -> Option<String>| (0..=u8::MAX).map_while(f).collect::<Vec<_>>();
    let mut out: BTreeSet<String> = ["fires true", "fires false"].map(String::from).into();
    out.extend(all(&|v| PlantKind::from_u8(v).map(|k| format!("{k:?}"))));
    out.extend(all(&|v| WaterKind::from_u8(v).map(|k| format!("{k:?}"))));
    out.extend(all(&|v| Zone::from_u8(v).map(|k| format!("{k:?}"))));
    out.extend(all(&|v| RoadClass::from_u8(v).map(|k| format!("{k:?}"))));
    out
}

#[test]
fn saved_sessions_cover_every_command_and_option() {
    let mut kinds = BTreeSet::new();
    let mut options = BTreeSet::new();
    for path in super::logs() {
        let text = std::fs::read_to_string(&path).expect("readable log");
        let log = CommandLog::parse(&text).expect("valid log");
        for (_, c) in &log.commands {
            let (kind, option) = words(c);
            kinds.insert(KINDS[kind]);
            options.extend(option);
        }
    }
    let missing: Vec<_> = KINDS.iter().filter(|k| !kinds.contains(*k)).collect();
    assert!(missing.is_empty(), "no saved session uses {missing:?}");
    let missing: Vec<_> = every_option().difference(&options).cloned().collect();
    assert!(missing.is_empty(), "no saved session uses {missing:?}");
}
