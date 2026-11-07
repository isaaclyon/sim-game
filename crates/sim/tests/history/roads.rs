//! Undoing and redoing road edits: builds, joins, splits, bulldozes, class changes, grids and
//! curve chains.
use super::*;
use sim::world::StepKind;

#[test]
fn a_road_is_undone_with_a_refund_and_redone_at_its_cost() {
    let mut w = empty();
    let start = balance(&w);
    assert!(street(&mut w, (0.0, 0.0), (200.0, 0.0)));
    let built = roads(&w);
    let cost = start - balance(&w);
    assert!(cost > 0);
    assert_eq!(w.history().undo_step().unwrap().kind, StepKind::Road);
    assert!(undo(&mut w));
    assert_eq!(w.roads.road_count(), 0);
    assert_eq!(balance(&w), start);
    assert!(redo(&mut w));
    assert_eq!(roads(&w), built);
    assert_eq!(balance(&w), start - cost);
}

#[test]
fn undoing_a_straight_join_unmerges_and_redo_joins_again() {
    let mut w = empty();
    assert!(street(&mut w, (0.0, 0.0), (100.0, 0.0)));
    let first = roads(&w);
    assert!(street(&mut w, (100.0, 0.0), (200.0, 0.0)));
    let joined = roads(&w);
    assert_eq!(joined.len(), 1);
    assert!(undo(&mut w));
    assert_eq!(roads(&w), first);
    assert!(redo(&mut w));
    assert_eq!(roads(&w), joined);
    assert!(undo(&mut w) && undo(&mut w));
    assert_eq!(w.roads.road_count(), 0);
}

#[test]
fn a_bulldozed_side_street_comes_back_with_its_junction() {
    let mut w = empty();
    assert!(street(&mut w, (0.0, 0.0), (200.0, 0.0)));
    assert!(street(&mut w, (100.0, 0.0), (100.0, 150.0)));
    let tee = roads(&w);
    assert_eq!(tee.len(), 3);
    let money = balance(&w);
    assert!(w.apply(Command::DeleteRoad { x: 100.0, y: 100.0 }));
    // The main line joins back into one road once the side street is gone (ADR 0040).
    assert_eq!(w.roads.road_count(), 1);
    assert_eq!(w.history().undo_step().unwrap().kind, StepKind::Bulldoze);
    assert!(undo(&mut w));
    assert_eq!(roads(&w), tee);
    assert_eq!(
        balance(&w),
        money,
        "bulldozing was free, so undoing it is too"
    );
    assert!(redo(&mut w));
    assert_eq!(w.roads.road_count(), 1);
}

#[test]
fn undoing_a_crossing_road_rejoins_the_road_it_split() {
    let mut w = empty();
    assert!(street(&mut w, (0.0, 100.0), (300.0, 100.0)));
    let one = roads(&w);
    assert!(street(&mut w, (150.0, 0.0), (150.0, 250.0)));
    let cross = roads(&w);
    assert_eq!(cross.len(), 4);
    assert!(undo(&mut w));
    assert_eq!(roads(&w), one);
    assert!(redo(&mut w));
    assert_eq!(roads(&w), cross);
}

#[test]
fn a_class_change_is_undone_in_place() {
    let mut w = empty();
    assert!(street(&mut w, (0.0, 0.0), (200.0, 0.0)));
    let id = w.roads.active_roads().next().unwrap();
    let money = balance(&w);
    assert!(w.apply(Command::SetRoadClass {
        x: 100.0,
        y: 0.0,
        class: RoadClass::Main,
    }));
    let upgrade = money - balance(&w);
    assert!(upgrade > 0);
    assert!(undo(&mut w));
    assert_eq!(
        w.roads.active_roads().collect::<Vec<_>>(),
        [id],
        "same road, not a new one"
    );
    assert_eq!(w.roads.road_class(id), RoadClass::Street);
    assert_eq!(balance(&w), money);
    assert!(redo(&mut w));
    assert_eq!(w.roads.road_class(id), RoadClass::Main);
    assert_eq!(balance(&w), money - upgrade);
}

#[test]
fn a_grid_is_one_step() {
    let mut w = empty();
    let start = balance(&w);
    assert!(w.apply(Command::BuildGrid {
        ox: 0.0,
        oy: 0.0,
        ux: 1.0,
        uy: 0.0,
        cell: 80.0,
        cols: 3,
        rows: 2,
        class: RoadClass::Street,
    }));
    let grid = roads(&w);
    assert!(grid.len() > 10);
    assert!(undo(&mut w));
    assert_eq!(w.roads.road_count(), 0);
    assert_eq!(balance(&w), start);
    assert_eq!(w.history().len(), (0, 1));
    assert!(redo(&mut w));
    assert_eq!(roads(&w), grid);
}

fn curve(w: &mut World, a: (f32, f32), c: (f32, f32), b: (f32, f32), chain: bool) -> bool {
    w.apply(Command::BuildCurve {
        ax: a.0,
        ay: a.1,
        cx: c.0,
        cy: c.1,
        bx: b.0,
        by: b.1,
        class: RoadClass::Street,
        chain,
    })
}

#[test]
fn a_continuous_curve_chain_is_one_step_until_another_edit() {
    let mut w = empty();
    let start = balance(&w);
    assert!(curve(&mut w, (0.0, 0.0), (50.0, 50.0), (100.0, 0.0), false));
    assert!(curve(
        &mut w,
        (100.0, 0.0),
        (150.0, -50.0),
        (200.0, 0.0),
        true
    ));
    assert!(curve(
        &mut w,
        (200.0, 0.0),
        (250.0, 50.0),
        (300.0, 0.0),
        true
    ));
    let chain = roads(&w);
    assert_eq!(w.history().len(), (1, 0));
    // A new chain starts a new step, and any other edit closes the chain.
    assert!(curve(
        &mut w,
        (0.0, 300.0),
        (50.0, 350.0),
        (100.0, 300.0),
        false
    ));
    assert!(street(&mut w, (400.0, 0.0), (400.0, 200.0)));
    assert!(curve(
        &mut w,
        (400.0, 200.0),
        (450.0, 250.0),
        (500.0, 200.0),
        true
    ));
    assert_eq!(w.history().len(), (4, 0));
    for _ in 0..3 {
        assert!(undo(&mut w));
    }
    assert_eq!(roads(&w), chain);
    assert!(undo(&mut w));
    assert_eq!(w.roads.road_count(), 0);
    assert_eq!(balance(&w), start);
    assert!(redo(&mut w));
    assert_eq!(roads(&w), chain);
}

#[test]
fn a_draft_that_continues_a_curve_is_a_chain_piece() {
    let mut w = empty();
    let first = [0.0, 0.0, 0.0, 50.0, 50.0, 0.0, 100.0, 0.0, 0.0];
    let Some(cmd @ Command::BuildCurve { chain: false, .. }) =
        w.draft_command(&first, RoadClass::Street)
    else {
        panic!("a first curve piece is not a chain piece");
    };
    assert!(w.apply(cmd));
    // Continuation: start (flagged), end, then the previous piece's control (ADR 0046).
    let next = [100.0, 0.0, 8.0, 200.0, 0.0, 0.0, 50.0, 50.0, 0.0];
    let cmd = w.draft_command(&next, RoadClass::Street);
    assert!(
        matches!(cmd, Some(Command::BuildCurve { chain: true, .. })),
        "{cmd:?}"
    );
    assert!(w.apply(cmd.unwrap()));
    assert_eq!(w.history().len(), (1, 0));
}

#[test]
fn redo_is_refused_when_the_city_cannot_pay() {
    let mut w = empty();
    assert!(street(&mut w, (0.0, 0.0), (200.0, 0.0)));
    let cost = w.history().undo_step().unwrap().cost;
    assert!(undo(&mut w));
    let spare = balance(&w) - cost + 1;
    w.add_funds_for_bench(-spare);
    assert!(!redo(&mut w));
    assert_eq!(
        w.history().len(),
        (0, 1),
        "the step waits until the city can pay"
    );
    w.add_funds_for_bench(1);
    assert!(redo(&mut w));
    assert_eq!(balance(&w), 0);
}

#[test]
fn a_road_an_alley_split_since_is_still_undone_and_redone() {
    let mut w = empty();
    for (a, b) in [
        ((0.0, 0.0), (400.0, 0.0)),
        ((400.0, 0.0), (400.0, 200.0)),
        ((400.0, 200.0), (0.0, 200.0)),
        ((0.0, 200.0), (0.0, 0.0)),
    ] {
        assert!(street(&mut w, a, b));
    }
    // Zoning a deep block cuts an alley across it, splitting both short sides (ADR 0016).
    assert!(w.apply(Command::FillBlock {
        x: 200.0,
        y: 100.0,
        zone: sim::Zone::ResidentialLow,
    }));
    assert_eq!(w.roads.road_count(), 7);
    let zoned = roads(&w);
    assert!(
        undo(&mut w),
        "the left side, now in two pieces, is taken back"
    );
    assert_eq!(w.roads.road_count(), 5);
    assert!(redo(&mut w));
    assert_eq!(
        roads(&w),
        zoned,
        "it comes back split where the alley meets it"
    );
}
