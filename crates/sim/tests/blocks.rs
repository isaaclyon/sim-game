//! Blocks, block fill, lots and alleys (ADR 0016).

use std::collections::BTreeSet;

use sim::block::{LOT_REACH, ROAD_HALF_WIDTH};
use sim::lot::{LotId, MIN_AREA_SIDE};
use sim::rng::Rng;
use sim::road::{ALLEY_SPEED, RoadNetwork};
use sim::{Command, Config, World, Zone};

fn empty() -> World {
    World::new(Config::default(), RoadNetwork::default(), Rng::new(1))
}

fn road(w: &mut World, ax: f32, ay: f32, bx: f32, by: f32) {
    assert!(w.apply(Command::BuildRoad { ax, ay, bx, by }));
}

/// A closed rectangle of four roads.
fn rect(w: &mut World, x0: f32, y0: f32, x1: f32, y1: f32) {
    road(w, x0, y0, x1, y0);
    road(w, x1, y0, x1, y1);
    road(w, x1, y1, x0, y1);
    road(w, x0, y1, x0, y0);
}

fn fill(w: &mut World, x: f32, y: f32, zone: Zone) -> bool {
    w.apply(Command::FillBlock { x, y, zone })
}

/// Zoned cells clear of every road that no lot owns.
fn unlotted(w: &World) -> usize {
    (0..w.zones.cells.len() as u32)
        .filter(|&i| w.zones.cells[i as usize] != 0 && w.lots.owner(i).is_none())
        .filter(|&i| {
            let (x, y) = w.zones.centre(i);
            road_dist(w, x, y) >= ROAD_HALF_WIDTH
        })
        .count()
}

fn zoned(w: &World) -> usize {
    w.zones.cells.iter().filter(|&&c| c != 0).count()
}

/// Distance from `(x, y)` to the nearest active road centreline, from the road snapshot.
fn road_dist(w: &World, x: f32, y: f32) -> f32 {
    let mut segs = Vec::new();
    w.write_road_segments(&mut segs);
    segs.chunks(5)
        .map(|s| {
            let (dx, dy) = (s[2] - s[0], s[3] - s[1]);
            let t = (((x - s[0]) * dx + (y - s[1]) * dy) / (dx * dx + dy * dy)).clamp(0.0, 1.0);
            (s[0] + dx * t - x).hypot(s[1] + dy * t - y)
        })
        .fold(f32::MAX, f32::min)
}

/// Every active lot's cells are disjoint, zoned-grid cells out of the roads.
fn check_lots(w: &World) -> BTreeSet<LotId> {
    let mut seen = BTreeSet::new();
    let lots: BTreeSet<LotId> = w.lots.active_lots().collect();
    for &lot in &lots {
        assert!(w.lots.cells(lot).len() >= 2);
        for &c in w.lots.cells(lot) {
            assert!(seen.insert(c), "cell {c} is in two lots");
            assert_eq!(w.lots.owner(c), Some(lot));
            let (x, y) = w.zones.centre(c);
            let d = road_dist(w, x, y);
            assert!(d >= ROAD_HALF_WIDTH - 0.01, "lot cell under a road ({d} m)");
            assert!(d <= LOT_REACH + 0.01, "lot cell out of reach ({d} m)");
        }
        assert_one_piece(w, lot);
        check_building_area(w, lot);
    }
    lots
}

/// The lot's cells all connect through shared edges.
fn assert_one_piece(w: &World, lot: LotId) {
    let cells = w.lots.cells(lot);
    let cols = w.zones.cols;
    let mut seen = BTreeSet::from([cells[0]]);
    let mut stack = vec![cells[0]];
    while let Some(c) = stack.pop() {
        for n in [c.wrapping_sub(1), c + 1, c.wrapping_sub(cols), c + cols] {
            let side = n / cols == c / cols || n % cols == c % cols;
            if side && cells.binary_search(&n).is_ok() && seen.insert(n) {
                stack.push(n);
            }
        }
    }
    assert_eq!(
        seen.len(),
        cells.len(),
        "lot {lot} is in more than one piece"
    );
}

/// A building area, if any, is at least 8 x 8 m and lies inside its lot.
fn check_building_area(w: &World, lot: LotId) {
    let l = lot as usize;
    let (bw, bd) = (w.lots.area_w[l], w.lots.area_d[l]);
    if bw == 0.0 {
        return;
    }
    assert!(bw >= MIN_AREA_SIDE && bd >= MIN_AREA_SIDE, "{bw} x {bd}");
    let (vx, vy) = (w.lots.facing_x[l], w.lots.facing_y[l]);
    for i in 0..=4 {
        for j in 0..=4 {
            // Points across the area, pulled 1 cm in from its edges.
            let su = (i as f32 / 2.0 - 1.0) * (1.0 - 0.02 / bw);
            let sv = (j as f32 / 2.0 - 1.0) * (1.0 - 0.02 / bd);
            let x = w.lots.area_x[l] - vy * bw / 2.0 * su + vx * bd / 2.0 * sv;
            let y = w.lots.area_y[l] + vx * bw / 2.0 * su + vy * bd / 2.0 * sv;
            let c = ((x - w.zones.x0) / 8.0).floor() as u32;
            let r = ((y - w.zones.y0) / 8.0).floor() as u32;
            assert_eq!(
                w.lots.owner(r * w.zones.cols + c),
                Some(lot),
                "lot {lot}'s building area leaves it at ({x}, {y})"
            );
        }
    }
}

#[test]
fn one_click_fills_a_block_but_not_the_roads_around_it() {
    let mut w = empty();
    rect(&mut w, 0.0, 0.0, 100.0, 100.0);
    let hash = w.state_hash();
    let mut outline = Vec::new();
    w.write_block_preview(50.0, 50.0, &mut outline);
    assert_eq!(outline.len(), 8, "four corners");
    assert_eq!(w.state_hash(), hash, "previewing changes nothing");

    assert!(fill(&mut w, 30.0, 70.0, Zone::ResidentialLow));
    // Cell centres 4, 12, ..., 96 on each axis are inside and at least 4 m from the roads.
    assert_eq!(zoned(&w), 12 * 12);
    assert_eq!(w.zones.zone_at(50.0, 50.0), Some(Zone::ResidentialLow));
    assert_eq!(w.zones.zone_at(50.0, -2.0), Some(Zone::None));
    assert_eq!(w.zones.zone_at(101.0, 50.0), Some(Zone::None));
    assert!(
        !fill(&mut w, 30.0, 70.0, Zone::ResidentialLow),
        "nothing new"
    );
    assert!(fill(&mut w, 30.0, 70.0, Zone::CommercialHigh));
    assert_eq!(w.zones.zone_at(95.0, 95.0), Some(Zone::CommercialHigh));
    assert_eq!(zoned(&w), 12 * 12);
}

#[test]
fn a_click_outside_every_closed_block_changes_nothing() {
    let mut w = empty();
    assert!(!fill(&mut w, 0.0, 0.0, Zone::ResidentialLow), "no roads");
    rect(&mut w, 0.0, 0.0, 100.0, 100.0);
    road(&mut w, 200.0, 0.0, 300.0, 0.0);
    road(&mut w, 300.0, 0.0, 300.0, 100.0); // Open on two sides.
    let hash = w.state_hash();
    let version = w.zones.version;
    for (x, y) in [
        (150.0, 50.0),
        (250.0, 50.0),
        (-50.0, 50.0),
        (f32::NAN, 50.0),
        (50.0, f32::INFINITY),
    ] {
        assert!(!fill(&mut w, x, y, Zone::IndustrialLow), "({x}, {y})");
        let mut outline = Vec::new();
        w.write_block_preview(x, y, &mut outline);
        assert!(outline.is_empty());
    }
    assert_eq!(w.zones.version, version);
    assert_eq!(w.state_hash(), hash);
    assert!(w.lots.is_empty());
}

#[test]
fn fill_follows_curved_edges() {
    let mut w = empty();
    road(&mut w, 0.0, 0.0, 100.0, 0.0);
    road(&mut w, 100.0, 0.0, 100.0, 100.0);
    road(&mut w, 0.0, 100.0, 0.0, 0.0);
    // The top bulges up to y = 150 at x = 50: y = 100 + 200 t (1 - t), x = 100 t.
    assert!(w.apply(Command::BuildCurve {
        ax: 0.0,
        ay: 100.0,
        cx: 50.0,
        cy: 200.0,
        bx: 100.0,
        by: 100.0,
    }));
    assert!(fill(&mut w, 50.0, 130.0, Zone::ResidentialHigh));
    let zone = |x, y| w.zones.zone_at(x, y);
    assert_eq!(zone(50.0, 130.0), Some(Zone::ResidentialHigh));
    assert_eq!(zone(50.0, 20.0), Some(Zone::ResidentialHigh));
    assert_eq!(zone(10.0, 110.0), Some(Zone::ResidentialHigh)); // Curve at y = 121 here.
    assert_eq!(zone(10.0, 130.0), Some(Zone::None));
    assert_eq!(zone(50.0, 160.0), Some(Zone::None));
    for (i, &c) in w.zones.cells.iter().enumerate() {
        if c != 0 {
            let (x, y) = w.zones.centre(i as u32);
            assert!(
                y < 100.0 + 2.0 * x * (1.0 - x / 100.0),
                "({x}, {y}) is past the curve"
            );
        }
    }
    check_lots(&w);
    assert!(
        w.lots.active_lots().any(|l| w.lots.road[l as usize] == 3),
        "lots face the curve"
    );
}

#[test]
fn a_road_network_inside_a_block_keeps_its_own_block_and_frontage() {
    let mut w = empty();
    rect(&mut w, 0.0, 0.0, 300.0, 300.0);
    rect(&mut w, 100.0, 100.0, 200.0, 200.0);
    // A dead end into the outer block.
    road(&mut w, 0.0, 250.0, 60.0, 250.0);
    assert!(fill(&mut w, 50.0, 50.0, Zone::ResidentialLow));
    assert_eq!(
        w.zones.zone_at(150.0, 150.0),
        Some(Zone::None),
        "inner block"
    );
    assert_eq!(
        w.zones.zone_at(30.0, 251.0),
        Some(Zone::None),
        "under the dead end"
    );
    assert_eq!(w.zones.zone_at(30.0, 258.0), Some(Zone::ResidentialLow));
    assert_eq!(w.zones.zone_at(90.0, 150.0), Some(Zone::ResidentialLow));
    let lots = check_lots(&w);
    // Lots face the inner square's outside and the dead end, not only the outer roads.
    let inner = (4..8).collect::<Vec<_>>();
    assert!(
        lots.iter()
            .any(|&l| inner.contains(&w.lots.road[l as usize]))
    );
    assert!(lots.iter().any(|&l| w.lots.road[l as usize] == 8));
    assert!(fill(&mut w, 150.0, 150.0, Zone::CommercialLow));
    assert_eq!(w.zones.zone_at(150.0, 150.0), Some(Zone::CommercialLow));
    assert_eq!(w.zones.zone_at(90.0, 150.0), Some(Zone::ResidentialLow));
    check_lots(&w);
}

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
        let cells = w.lots.cells(l as LotId);
        let (mut cx, mut cy) = (0.0, 0.0);
        for &c in cells {
            let (x, y) = w.zones.centre(c);
            cx += x / cells.len() as f32;
            cy += y / cells.len() as f32;
        }
        let (fx, fy) = (w.lots.facing_x[l], w.lots.facing_y[l]);
        assert!((fx.hypot(fy) - 1.0).abs() < 1e-4);
        let toward = (w.lots.front_x[l] - cx) * fx + (w.lots.front_y[l] - cy) * fy;
        assert!(toward > 0.0, "lot {l} faces away from its road");
    }
    let mut outlines = Vec::new();
    w.write_lot_outlines(&mut outlines);
    assert!(!outlines.is_empty() && outlines.len() % 4 == 0);
    // On a plain square block every lot has room for a building.
    assert!(lots.iter().all(|&l| w.lots.area_w[l as usize] > 0.0));
    let mut areas = Vec::new();
    w.write_building_areas(&mut areas);
    assert_eq!(areas.len(), lots.len() * 16, "four edges per lot");

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
fn deep_blocks_get_an_alley_that_cars_can_use_and_players_can_bulldoze() {
    let mut w = empty();
    rect(&mut w, 0.0, 0.0, 400.0, 200.0);
    let roads = w.roads.road_count();
    assert!(fill(&mut w, 200.0, 100.0, Zone::ResidentialLow));
    // Four times as long as it is deep: one alley along it, splitting both short sides.
    assert_eq!(w.roads.road_count(), roads + 3);
    let alley: Vec<u32> = w
        .roads
        .active_roads()
        .filter(|&r| w.roads.lane_speed[r as usize * 2] == ALLEY_SPEED)
        .collect();
    assert_eq!(alley.len(), 1);
    let (a, b) = w.roads.road_ends(alley[0]);
    for n in [a, b] {
        assert!((w.roads.node_y[n as usize] - 100.0).abs() < 8.0);
        assert_eq!(
            w.roads.out_lanes(n).len(),
            3,
            "a T-junction with the side road"
        );
    }
    let lots = check_lots(&w);
    assert_eq!(unlotted(&w), 0, "every zoned cell has a lot");
    assert!(lots.iter().any(|&l| w.lots.road[l as usize] == alley[0]));

    // Cars route over the alley like any road.
    w.apply(Command::SpawnAgents { count: 200 });
    for _ in 0..3_000 {
        w.tick();
    }
    assert!(w.stats().trips_completed > 0);

    // Bulldozing the alley sticks; zoning the interior again brings one back.
    assert!(w.apply(Command::DeleteRoad { x: 200.0, y: 100.0 }));
    assert_eq!(w.roads.road_count(), roads + 2);
    check_lots(&w);
    assert!(
        !fill(&mut w, 200.0, 100.0, Zone::ResidentialLow),
        "already zoned"
    );
    assert!(fill(&mut w, 200.0, 100.0, Zone::ResidentialHigh));
    assert!(w.roads.road_count() > roads + 2, "an alley came back");
    check_lots(&w);
}

#[test]
fn big_square_blocks_get_connector_streets_until_every_cell_is_reached() {
    let run = || {
        let mut w = empty();
        rect(&mut w, 0.0, 0.0, 300.0, 300.0);
        let roads = w.roads.road_count();
        assert!(fill(&mut w, 150.0, 150.0, Zone::IndustrialLow));
        assert!(w.roads.road_count() > roads);
        assert!(
            w.roads
                .active_roads()
                .skip(roads)
                .any(|r| w.roads.lane_speed[r as usize * 2] != ALLEY_SPEED),
            "a square block is cut across first"
        );
        check_lots(&w);
        let unreached = unlotted(&w);
        assert!(unreached < 6, "{unreached} zoned cells without a lot");
        w.state_hash()
    };
    assert_eq!(run(), run(), "deterministic");
}

#[test]
fn painting_part_of_a_block_makes_lots_and_alleys_only_where_painted() {
    let mut w = empty();
    rect(&mut w, 0.0, 0.0, 400.0, 200.0);
    road(&mut w, 400.0, 0.0, 800.0, 0.0);
    road(&mut w, 800.0, 0.0, 800.0, 200.0);
    road(&mut w, 800.0, 200.0, 400.0, 200.0);
    let roads = w.roads.road_count();
    // Along the edge only: within reach, so no alley.
    assert!(w.apply(Command::PaintZone {
        ax: 0.0,
        ay: 0.0,
        bx: 400.0,
        by: 30.0,
        zone: Zone::CommercialLow,
    }));
    assert_eq!(w.roads.road_count(), roads);
    let lots = check_lots(&w);
    assert!(!lots.is_empty());
    assert!(lots.iter().all(|&l| w.lots.front_x[l as usize] <= 400.0));
    // Into the middle of the block: now it needs an alley; the neighbour still doesn't.
    assert!(w.apply(Command::PaintZone {
        ax: 150.0,
        ay: 80.0,
        bx: 250.0,
        by: 120.0,
        zone: Zone::CommercialLow,
    }));
    assert_eq!(w.roads.road_count(), roads + 3);
    check_lots(&w);
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
fn an_alley_keeps_the_lots_across_the_street() {
    let mut w = empty();
    rect(&mut w, -100.0, 0.0, 0.0, 200.0);
    road(&mut w, 0.0, 0.0, 400.0, 0.0);
    road(&mut w, 400.0, 0.0, 400.0, 200.0);
    road(&mut w, 400.0, 200.0, 0.0, 200.0);
    assert!(fill(&mut w, -50.0, 100.0, Zone::ResidentialLow));
    let west: BTreeSet<LotId> = w.lots.active_lots().collect();
    assert!(!west.is_empty());
    let roads = w.roads.road_count();
    assert!(fill(&mut w, 200.0, 100.0, Zone::ResidentialLow));
    assert!(w.roads.road_count() > roads, "the deep block got an alley");
    let after: BTreeSet<LotId> = w.lots.active_lots().collect();
    assert!(
        west.is_subset(&after),
        "zoning next door retires no lots here"
    );
    let active: BTreeSet<u32> = w.roads.active_roads().collect();
    for &l in &west {
        let l = l as usize;
        let r = w.lots.road[l];
        assert!(active.contains(&r));
        // The frontage road still passes through the lot's front point.
        let (a, b) = w.roads.road_ends(r);
        let (ax, ay) = (w.roads.node_x[a as usize], w.roads.node_y[a as usize]);
        let (bx, by) = (w.roads.node_x[b as usize], w.roads.node_y[b as usize]);
        let (fx, fy) = (w.lots.front_x[l], w.lots.front_y[l]);
        let inside = |v: f32, p: f32, q: f32| v >= p.min(q) - 0.01 && v <= p.max(q) + 0.01;
        assert!(
            inside(fx, ax, bx) && inside(fy, ay, by),
            "lot {l} fronts road {r}"
        );
    }
    check_lots(&w);
}

#[test]
fn repainting_without_changing_the_interior_does_not_bring_an_alley_back() {
    let mut w = empty();
    rect(&mut w, 0.0, 0.0, 400.0, 200.0);
    assert!(fill(&mut w, 200.0, 100.0, Zone::ResidentialLow));
    assert!(w.apply(Command::DeleteRoad { x: 200.0, y: 100.0 }));
    let roads = w.roads.road_count();
    // Same zone; only cells under and beyond the roads change.
    assert!(w.apply(Command::PaintZone {
        ax: -16.0,
        ay: -16.0,
        bx: 416.0,
        by: 216.0,
        zone: Zone::ResidentialLow,
    }));
    assert_eq!(w.roads.road_count(), roads, "a bulldozed alley stays gone");
}
