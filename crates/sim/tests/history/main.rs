//! Undo and redo (ADR 0051): every placeable edit can be taken back and made again, as new
//! edits, with its money refunded and charged again, also after a save and load.
mod pieces;
mod roads;
mod view;

use sim::road::RoadClass;
use sim::{Command, Config, Era, World};

/// Empty land in the town era, so main roads are unlocked (ADR 0056).
fn empty() -> World {
    World::empty(Config::default(), 30, 30).with_era(Era::Town)
}

fn road(w: &mut World, a: (f32, f32), b: (f32, f32), class: RoadClass) -> bool {
    w.apply(Command::BuildRoad {
        ax: a.0,
        ay: a.1,
        bx: b.0,
        by: b.1,
        class,
    })
}

fn street(w: &mut World, a: (f32, f32), b: (f32, f32)) -> bool {
    road(w, a, b, RoadClass::Street)
}

fn undo(w: &mut World) -> bool {
    w.apply(Command::Undo)
}

fn redo(w: &mut World) -> bool {
    w.apply(Command::Redo)
}

fn balance(w: &World) -> i64 {
    w.money().balance
}

/// A road's two end positions in centimetres, lower first, and its class.
type Road = ((i32, i32), (i32, i32), RoadClass);

/// Every active road, sorted: the map as a player sees it, whatever the IDs.
fn roads(w: &World) -> Vec<Road> {
    let at = |n: u32| {
        let (x, y) = (w.roads.node_x[n as usize], w.roads.node_y[n as usize]);
        ((x * 100.0).round() as i32, (y * 100.0).round() as i32)
    };
    let mut out: Vec<_> = w
        .roads
        .active_roads()
        .map(|r| {
            let (a, b) = w.roads.road_ends(r);
            let (a, b) = (at(a).min(at(b)), at(a).max(at(b)));
            (a, b, w.roads.road_class(r))
        })
        .collect();
    out.sort_by_key(|&(a, b, c)| (a, b, c as u8));
    out
}

#[test]
fn nothing_to_undo_or_redo_is_refused() {
    let mut w = empty();
    assert!(!undo(&mut w));
    assert!(!redo(&mut w));
    assert_eq!(w.history().len(), (0, 0));
}

#[test]
fn history_survives_a_save_and_load() {
    let mut w = empty();
    assert!(street(&mut w, (0.0, 0.0), (200.0, 0.0)));
    assert!(street(&mut w, (0.0, 100.0), (200.0, 100.0)));
    assert!(undo(&mut w));
    let mut loaded = World::load(&w.save().expect("saves")).expect("loads");
    assert_eq!(loaded.history().len(), (1, 1));
    assert!(redo(&mut loaded));
    assert_eq!(loaded.roads.road_count(), 2);
    assert!(undo(&mut loaded) && undo(&mut loaded));
    assert_eq!(loaded.roads.road_count(), 0);
}

#[test]
fn a_new_edit_clears_redo_but_zoning_does_not() {
    let mut w = empty();
    assert!(street(&mut w, (0.0, 0.0), (200.0, 0.0)));
    assert!(street(&mut w, (0.0, 100.0), (200.0, 100.0)));
    assert!(undo(&mut w));
    assert!(w.apply(Command::PaintZone {
        ax: 0.0,
        ay: 0.0,
        bx: 50.0,
        by: 50.0,
        zone: sim::Zone::ResidentialLow,
    }));
    assert_eq!(w.history().len(), (1, 1));
    assert!(street(&mut w, (300.0, 0.0), (300.0, 200.0)));
    assert_eq!(w.history().len(), (2, 0));
    assert!(!redo(&mut w));
}

#[test]
fn history_keeps_at_most_the_newest_steps() {
    let mut w = empty();
    w.add_funds_for_bench(10_000_000);
    for i in 0..(sim::world::MAX_STEPS + 5) {
        let y = i as f32 * 20.0;
        assert!(street(&mut w, (0.0, y), (100.0, y)), "road {i}");
    }
    assert_eq!(w.history().len(), (sim::world::MAX_STEPS, 0));
}
