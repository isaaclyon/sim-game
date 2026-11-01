//! Cutting blocks into lots, and recutting them when roads change.

use std::collections::BTreeSet;

use sim::lot::LotId;
use sim::rng::Rng;
use sim::road::{RoadClass, RoadNetwork};
use sim::{Command, Config, World, Zone};

use super::{area, bounds, centroid, check_lots, empty, fill, rect, road, unlotted};

#[test]
fn lots_fill_a_zoned_block_and_face_their_road() {
    let mut w = empty();
    rect(&mut w, 0.0, 0.0, 100.0, 100.0);
    assert!(w.lots.is_empty(), "no lots without zoning");
    assert!(fill(&mut w, 50.0, 50.0, Zone::ResidentialLow));
    let lots = check_lots(&w);
    let covered: usize = lots.iter().map(|&l| w.lots.cells(l).len()).sum();
    assert!(
        covered >= 12 * 12 - 8,
        "lots cover the block, {covered} cells"
    );
    for &l in &lots {
        let l = l as usize;
        assert_eq!(w.lots.zone(l as LotId, &w.zones), Zone::ResidentialLow);
        let (cx, cy) = centroid(w.lots.outline(l as LotId));
        let (fx, fy) = (w.lots.facing_x[l], w.lots.facing_y[l]);
        assert!((fx.hypot(fy) - 1.0).abs() < 1e-4);
        let toward = (w.lots.front_x[l] - cx) * fx + (w.lots.front_y[l] - cy) * fy;
        assert!(toward > 0.0, "lot {l} faces away from its road");
    }
    let mut outlines = Vec::new();
    w.write_lot_outlines(&mut outlines);
    assert!(!outlines.is_empty() && outlines.len() % 4 == 0);

    // Unzoning keeps the lots and their ids (buildings empty out instead, #17).
    let version = w.lots.version;
    assert!(fill(&mut w, 50.0, 50.0, Zone::None));
    assert_eq!(w.lots.active_lots().collect::<BTreeSet<_>>(), lots);
    assert_eq!(w.lots.version, version);
    assert_eq!(w.lots.zone(*lots.first().unwrap(), &w.zones), Zone::None);
}

#[test]
fn road_edits_recut_only_the_lots_they_change() {
    let mut w = empty();
    rect(&mut w, 0.0, 0.0, 200.0, 100.0);
    road(&mut w, 200.0, 0.0, 300.0, 0.0);
    road(&mut w, 300.0, 0.0, 300.0, 100.0);
    road(&mut w, 300.0, 100.0, 200.0, 100.0);
    assert!(fill(&mut w, 50.0, 50.0, Zone::ResidentialLow));
    assert!(fill(&mut w, 250.0, 50.0, Zone::CommercialLow));
    let before = check_lots(&w);
    let east: BTreeSet<LotId> = before
        .iter()
        .copied()
        .filter(|&l| w.lots.front_x[l as usize] > 200.0)
        .collect();
    road(&mut w, 100.0, -50.0, 100.0, 150.0);
    let after = check_lots(&w);
    assert!(east.is_subset(&after), "the block next door is untouched");
    assert!(
        before.difference(&after).count() > 0,
        "lots by the new road are recut"
    );
    assert!(after.iter().any(|l| !before.contains(l)));
    assert!(
        after.iter().any(|&l| w.lots.road[l as usize] >= 8),
        "new lots face the new road"
    );
    // Bulldozing it merges the halves again.
    assert!(w.apply(Command::DeleteRoad { x: 100.0, y: 50.0 }));
    let merged = check_lots(&w);
    let active: BTreeSet<u32> = w.roads.active_roads().collect();
    assert!(
        merged
            .iter()
            .all(|&l| active.contains(&w.lots.road[l as usize]))
    );
    assert!(east.is_subset(&merged));
    // A road that leaves zoned land unlotted: bulldozing a block's edge opens it.
    assert!(w.apply(Command::DeleteRoad { x: 300.0, y: 50.0 }));
    let open = check_lots(&w);
    assert!(
        open.iter()
            .all(|&l| w.lots.front_x[l as usize] <= 200.0 + 1.0)
    );
}

#[test]
fn zoning_the_generated_town_is_deterministic() {
    let run = || {
        let cfg = Config::default();
        let mut w = World::generate(cfg, 8, 8);
        w.apply(Command::SpawnAgents { count: 500 });
        for _ in 0..200 {
            w.tick();
        }
        let b = w.roads.bounds();
        assert!(w.apply(Command::PaintZone {
            ax: b[0],
            ay: b[1],
            bx: b[2],
            by: b[3],
            zone: Zone::ResidentialLow,
        }));
        check_lots(&w);
        assert!(w.lots.active_lots().count() > 100);
        for _ in 0..600 {
            w.tick();
        }
        assert!(w.stats().trips_completed > 0);
        w.state_hash()
    };
    assert_eq!(run(), run());
}

#[test]
fn merging_then_opening_a_block_retires_all_its_lots() {
    let mut w = empty();
    rect(&mut w, 0.0, 0.0, 100.0, 100.0);
    road(&mut w, 100.0, 0.0, 200.0, 0.0);
    road(&mut w, 200.0, 0.0, 200.0, 100.0);
    road(&mut w, 200.0, 100.0, 100.0, 100.0);
    assert!(fill(&mut w, 50.0, 50.0, Zone::ResidentialLow));
    assert!(fill(&mut w, 150.0, 50.0, Zone::ResidentialLow));
    assert!(w.apply(Command::DeleteRoad { x: 100.0, y: 50.0 }));
    assert!(
        w.lots.active_lots().next().is_some(),
        "the merged block keeps lots"
    );
    // Lots kept through the merge must know their block grew, so opening it finds them.
    assert!(w.apply(Command::DeleteRoad { x: 200.0, y: 50.0 }));
    assert_eq!(w.lots.active_lots().count(), 0, "no closed block, no lots");
    let mut outlines = Vec::new();
    w.write_lot_outlines(&mut outlines);
    assert!(outlines.is_empty());
}

#[test]
fn lots_from_facing_roads_meet_on_one_line_without_slivers() {
    let mut w = empty();
    // A skewed four-sided block, and one with a straight side facing a sagging curve.
    let q = [(0.0, 0.0), (112.0, -4.0), (122.0, 100.0), (-3.0, 96.0)];
    for i in 0..4 {
        let (a, b) = (q[i], q[(i + 1) % 4]);
        road(&mut w, a.0, a.1, b.0, b.1);
    }
    road(&mut w, 200.0, 100.0, 420.0, 115.0);
    road(&mut w, 420.0, 115.0, 440.0, 20.0);
    road(&mut w, 200.0, 20.0, 200.0, 100.0);
    assert!(w.apply(Command::BuildCurve {
        ax: 200.0,
        ay: 20.0,
        cx: 320.0,
        cy: -60.0,
        bx: 440.0,
        by: 20.0,
        class: RoadClass::Street,
    }));
    assert!(fill(&mut w, 50.0, 50.0, Zone::ResidentialLow));
    assert!(fill(&mut w, 320.0, 60.0, Zone::ResidentialLow));
    check_lots(&w);
    assert_eq!(unlotted(&w), 0, "every zoned cell has a lot");
}

/// A perfect lattice zoned whole: every block is cut the same way, into rectangles facing
/// two of its sides, with no alleys.
#[test]
fn a_perfect_grid_cuts_every_block_the_same_way() {
    let cfg = Config::default();
    let spacing = cfg.grid_spacing;
    let n = 5;
    let mut w = World::new(
        cfg,
        RoadNetwork::generate_lattice(n, n, spacing),
        Rng::new(1),
    );
    let roads = w.roads.road_count();
    let e = (n - 1) as f32 * spacing;
    assert!(w.apply(Command::PaintZone {
        ax: 0.0,
        ay: 0.0,
        bx: e,
        by: e,
        zone: Zone::ResidentialLow,
    }));
    let lots = check_lots(&w);
    assert_eq!(unlotted(&w), 0);
    assert_eq!(
        w.roads.road_count(),
        roads,
        "blocks this size need no alley"
    );

    // Each block's lots, moved to the block's own corner, rounded to 10 cm and sorted.
    let mut blocks: Vec<Vec<Vec<(i32, i32)>>> = vec![Vec::new(); ((n - 1) * (n - 1)) as usize];
    for &l in &lots {
        let o = w.lots.outline(l);
        let b = bounds(o);
        assert!(
            o.len() == 4 && ((b[2] - b[0]) * (b[3] - b[1]) - area(o)).abs() < 1.0,
            "lot {l} is not a rectangle: {o:?}"
        );
        let (cx, cy) = centroid(o);
        let (bx, by) = ((cx / spacing).floor(), (cy / spacing).floor());
        let mut shape: Vec<(i32, i32)> = o
            .iter()
            .map(|p| {
                let x = ((p.0 - bx * spacing) * 10.0).round() as i32;
                let y = ((p.1 - by * spacing) * 10.0).round() as i32;
                (x, y)
            })
            .collect();
        // Start every outline at its smallest point, so equal shapes compare equal.
        let first = (0..shape.len()).min_by_key(|&i| shape[i]).unwrap_or(0);
        shape.rotate_left(first);
        blocks[(by as u32 * (n - 1) + bx as u32) as usize].push(shape);
    }
    for b in &mut blocks {
        b.sort();
    }
    assert_eq!(blocks[0].len(), 14, "7 lots on each of two sides");
    for (i, b) in blocks.iter().enumerate() {
        assert_eq!(b, &blocks[0], "block {i} is cut differently from block 0");
    }
}
