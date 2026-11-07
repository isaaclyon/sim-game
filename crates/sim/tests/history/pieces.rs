//! Undoing and redoing plants, lines, pumps, towers and fire stations, and their bulldozing.
use super::*;
use sim::power::PlantKind;
use sim::water::WaterKind;
use sim::world::StepKind;

/// A town with one long street to place things beside.
fn town() -> World {
    let mut w = empty();
    assert!(street(&mut w, (0.0, 0.0), (600.0, 0.0)));
    w
}

fn plants(w: &World) -> usize {
    w.power.active_plants().count()
}

#[test]
fn a_plant_is_undone_with_a_refund_and_redone_over_new_zoning() {
    let mut w = town();
    let start = balance(&w);
    assert!(w.apply(Command::PlacePlant {
        kind: PlantKind::Coal,
        x: 100.0,
        y: 40.0,
    }));
    let at = w.power.footprint(0);
    assert_eq!(
        w.history().undo_step().unwrap().kind,
        StepKind::Plant(PlantKind::Coal)
    );
    assert!(undo(&mut w));
    assert_eq!(plants(&w), 0);
    assert_eq!(balance(&w), start);
    // The player zones where it stood; making it again clears that land, as placing it does.
    assert!(w.apply(Command::PaintZone {
        ax: at.centre.0 - 5.0,
        ay: at.centre.1 - 5.0,
        bx: at.centre.0 + 5.0,
        by: at.centre.1 + 5.0,
        zone: sim::Zone::ResidentialLow,
    }));
    assert_eq!(
        w.zones.zone_at(at.centre.0, at.centre.1),
        Some(sim::Zone::ResidentialLow)
    );
    assert!(redo(&mut w));
    assert_eq!(
        w.zones.zone_at(at.centre.0, at.centre.1),
        Some(sim::Zone::None)
    );
    assert_eq!(plants(&w), 1);
    let p = w.power.active_plants().next().unwrap();
    assert_eq!(
        w.power.footprint(p).centre,
        at.centre,
        "back where it stood"
    );
    assert!(balance(&w) < start);
}

#[test]
fn a_bulldozed_plant_comes_back_for_free() {
    let mut w = town();
    assert!(w.apply(Command::PlacePlant {
        kind: PlantKind::Solar,
        x: 300.0,
        y: 80.0,
    }));
    let money = balance(&w);
    assert!(w.apply(Command::DeleteRoad { x: 300.0, y: 80.0 }));
    assert_eq!(plants(&w), 0);
    assert!(undo(&mut w));
    assert_eq!(plants(&w), 1);
    assert_eq!(balance(&w), money);
}

#[test]
fn lines_pumps_towers_and_stations_undo_and_redo() {
    let mut w = town();
    let start = balance(&w);
    let edits = [
        Command::BuildLine {
            ax: 50.0,
            ay: 100.0,
            bx: 50.0,
            by: 300.0,
        },
        Command::PlaceWater {
            kind: WaterKind::Pump,
            x: 200.0,
            y: 30.0,
        },
        Command::PlaceWater {
            kind: WaterKind::Tower,
            x: 300.0,
            y: -30.0,
        },
        Command::PlaceStation { x: 450.0, y: 30.0 },
    ];
    for cmd in edits {
        assert!(w.apply(cmd.clone()), "{cmd:?}");
    }
    let count = |w: &World| {
        (
            w.power.active_lines().count(),
            w.water.active_pieces().count(),
            w.fire_stats().stations,
        )
    };
    let placed = count(&w);
    assert_eq!(placed, (1, 2, 1));
    let spent = start - balance(&w);
    for _ in 0..4 {
        assert!(undo(&mut w));
    }
    assert_eq!(count(&w), (0, 0, 0));
    assert_eq!(balance(&w), start);
    for _ in 0..4 {
        assert!(redo(&mut w));
    }
    assert_eq!(count(&w), placed);
    assert_eq!(balance(&w), start - spent);
    // Bulldozing a station retires its trucks; undoing that gives it new ones.
    assert!(w.apply(Command::DeleteRoad { x: 450.0, y: 30.0 }));
    assert_eq!(w.fire_stats().stations, 0);
    assert!(undo(&mut w));
    assert_eq!(w.fire_stats().stations, 1);
}
