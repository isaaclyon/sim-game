use super::*;

/// A short session through every command kind, recorded the way the game records it.
fn session() -> (CommandLog, World) {
    let mut log = CommandLog::new(5, 4, 4);
    let mut world = log.world();
    let steps: [(u32, Command); 12] = [
        (0, Command::SpawnAgents { count: 20 }),
        (
            30,
            Command::FillBlock {
                x: 50.0,
                y: 50.0,
                zone: Zone::ResidentialLow,
            },
        ),
        (
            0,
            Command::PaintZone {
                ax: 130.0,
                ay: 10.0,
                bx: 230.1,
                by: 110.7,
                zone: Zone::CommercialLow,
            },
        ),
        (
            200,
            Command::BuildRoad {
                ax: 0.1,
                ay: 60.0,
                bx: 120.0,
                by: 60.000_004,
                class: RoadClass::Main,
            },
        ),
        (
            50,
            Command::BuildCurve {
                ax: 240.0,
                ay: 240.0,
                cx: 330.0,
                cy: 300.3,
                bx: 360.0,
                by: 360.0,
                class: RoadClass::Street,
            },
        ),
        (
            0,
            Command::PlacePlant {
                kind: PlantKind::Wind,
                x: 500.0,
                y: 60.0,
            },
        ),
        (
            0,
            Command::BuildLine {
                ax: 500.0,
                ay: 60.0,
                bx: 360.0,
                by: 60.0,
            },
        ),
        (
            0,
            Command::PlaceWater {
                kind: WaterKind::Tower,
                x: -20.5,
                y: 180.25,
            },
        ),
        (
            10,
            Command::SetRoadClass {
                x: 60.0,
                y: 60.0,
                class: RoadClass::Street,
            },
        ),
        (10, Command::DeleteRoad { x: 300.0, y: 120.0 }),
        (
            5,
            Command::BuildGrid {
                ox: 700.25,
                oy: -40.5,
                ux: 0.6,
                uy: 0.8,
                cell: 80.0,
                cols: 2,
                rows: -1,
                class: RoadClass::Street,
            },
        ),
        (
            0,
            Command::FillBlock {
                x: -500.0,
                y: -500.0,
                zone: Zone::IndustrialHigh,
            },
        ),
    ];
    for (ticks, cmd) in steps {
        for _ in 0..ticks {
            world.tick();
        }
        log.apply(&mut world, cmd);
    }
    for _ in 0..300 {
        world.tick();
    }
    log.end = world.tick_count();
    (log, world)
}

#[test]
fn recorded_session_replays_to_the_same_world() {
    let (log, live) = session();
    assert_eq!(log.commands.len(), 12);
    let replayed = log.replay();
    assert_eq!(replayed.tick_count(), live.tick_count());
    assert_eq!(Outcome::of(&replayed), Outcome::of(&live));
}

#[test]
fn text_round_trips_bit_for_bit() {
    let (mut log, live) = session();
    log.expect = Some(Outcome::of(&live));
    let text = log.to_string();
    assert!(text.contains(" class 60 60 street\n"), "{text}");
    assert!(
        text.contains(" roadgrid 700.25 -40.5 0.6 0.8 80 2 -1 street\n"),
        "{text}"
    );
    assert_eq!(CommandLog::parse(&text), Ok(log.clone()));
    // Comments and blank lines are ignored.
    let commented = format!(
        "# recorded in a test\n\n{}",
        text.replace("\nend", "\n\n# done\nend")
    );
    assert_eq!(CommandLog::parse(&commented), Ok(log));
}

#[test]
fn a_save_and_load_partway_through_does_not_change_the_replay() {
    let (log, live) = session();
    let mut world = log.world();
    let next = log.advance(&mut world, 0, 100);
    assert!(next > 0 && next < log.commands.len());
    let mut loaded = World::load(&world.save().unwrap()).unwrap();
    log.advance(&mut loaded, next, log.end);
    assert_eq!(Outcome::of(&loaded), Outcome::of(&live));
}

#[test]
fn empty_map_logs_start_without_roads_and_round_trip() {
    let mut log = CommandLog::empty(3, 10, 10);
    let mut world = log.world();
    assert_eq!(world.roads.road_count(), 0);
    let road = Command::BuildRoad {
        ax: 0.0,
        ay: 0.0,
        bx: 240.0,
        by: 0.0,
        class: RoadClass::Street,
    };
    assert!(log.apply(&mut world, road));
    let text = log.to_string();
    assert!(text.contains("\nmap 10 10\n"), "{text}");
    let parsed = CommandLog::parse(&text).unwrap();
    assert_eq!(parsed, log);
    assert_eq!(parsed.replay().state_hash(), world.state_hash());
    let head = "sim-game command log 1\nseed 1\nmesh 3 3\nend 0\n";
    assert_eq!(
        error(head),
        "line 3: expected `map` or `grid`, found `mesh`"
    );
}

fn error(text: &str) -> String {
    CommandLog::parse(text).unwrap_err().to_string()
}

#[test]
fn malformed_logs_name_the_line_and_problem() {
    let head = "sim-game command log 1\nseed 1\ngrid 3 3\n";
    assert_eq!(error(""), "line 0: missing title line");
    assert!(error("sim-game command log 2\n").contains("unsupported command log version 2"));
    assert_eq!(
        error("sim-game save 1\n"),
        "line 1: expected `command`, found `save`"
    );
    assert_eq!(error(head), "line 3: missing end line");
    assert_eq!(
        error(&format!("{head}5 spawn 1\n4 spawn 1\nend 9\n")),
        "line 5: ticks must not decrease"
    );
    assert_eq!(
        error(&format!("{head}5 spawn 1\nend 4\n")),
        "line 5: `end` comes before the last command"
    );
    assert_eq!(
        error(&format!("{head}0 road 1 2 3 street\nend 0\n")),
        "line 4: `street` is not a valid coordinate"
    );
    assert_eq!(
        error(&format!("{head}0 fill 1 2 parkland\nend 0\n")),
        "line 4: unknown zone `parkland`"
    );
    assert_eq!(
        error(&format!("{head}0 class 1 2 avenue\nend 0\n")),
        "line 4: unknown road class `avenue`"
    );
    assert_eq!(
        error(&format!("{head}0 teleport\nend 0\n")),
        "line 4: unknown command `teleport`"
    );
    assert_eq!(
        error(&format!("{head}0 spawn 1 2\nend 0\n")),
        "line 4: unexpected `2`"
    );
    assert_eq!(
        error(&format!(
            "{head}end 0\nexpect hash xyz population 1 buildings 1 trips 1\n"
        )),
        "line 5: `xyz` is not a hexadecimal hash"
    );
    assert_eq!(
        error(&format!("{head}end 0\nend 1\n")),
        "line 5: expected `expect`, found `end`"
    );
}
