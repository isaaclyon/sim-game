//! Blocks, block fill, lots and alleys (ADR 0016).

use std::collections::BTreeSet;

use sim::block::{LOT_REACH, MIN_LOT_AREA, ROAD_HALF_WIDTH};
use sim::lot::LotId;
use sim::rng::Rng;
use sim::road::{ALLEY_SPEED, RoadClass, RoadNetwork};
use sim::{Command, Config, World, Zone};

fn empty() -> World {
    World::new(Config::default(), RoadNetwork::default(), Rng::new(1))
}

fn road(w: &mut World, ax: f32, ay: f32, bx: f32, by: f32) {
    assert!(w.apply(Command::BuildRoad {
        ax,
        ay,
        bx,
        by,
        class: RoadClass::Street
    }));
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

/// Every active lot is one simple shape of at least `MIN_LOT_AREA`, out of the roads, facing
/// a road it fronts; no two overlap, and each owns the zone cells whose centres it covers.
fn check_lots(w: &World) -> BTreeSet<LotId> {
    let lots: BTreeSet<LotId> = w.lots.active_lots().collect();
    let mut seen = BTreeSet::new();
    for &lot in &lots {
        let o = w.lots.outline(lot);
        assert!(o.len() >= 3, "lot {lot} has no shape");
        assert!(area(o) >= MIN_LOT_AREA - 0.5, "lot {lot} is {} m²", area(o));
        assert_simple(lot, o);
        // Spikes and slivers, where lots from two roads meet, have hair-thin corners.
        assert!(sharpest(o) > 5.0, "lot {lot} has a {}° corner", sharpest(o));
        for &c in w.lots.cells(lot) {
            assert!(seen.insert(c), "cell {c} is in two lots");
            assert_eq!(w.lots.owner(c), Some(lot));
            assert!(
                inside(o, w.zones.centre(c)),
                "lot {lot} owns cell {c} outside it"
            );
        }
        for (x, y) in o.iter().copied() {
            let d = road_dist(w, x, y);
            assert!(
                d >= ROAD_HALF_WIDTH - 0.05,
                "lot {lot} corner under a road ({d} m)"
            );
        }
        let (cx, cy) = centroid(o);
        assert!(
            road_dist(w, cx, cy) <= LOT_REACH,
            "lot {lot} is out of reach"
        );
        let l = lot as usize;
        let edge = o
            .iter()
            .zip(o.iter().cycle().skip(1))
            .map(|(&a, &b)| seg_dist(a, b, (w.lots.front_x[l], w.lots.front_y[l])))
            .fold(f32::MAX, f32::min);
        assert!(
            edge <= ROAD_HALF_WIDTH + 0.5,
            "lot {lot}'s front is {edge} m from it"
        );
    }
    // No overlaps: a 2 m sample grid hits at most one lot.
    for &a in &lots {
        let oa = w.lots.outline(a);
        let ra = bounds(oa);
        for &b in lots.range(a + 1..) {
            let ob = w.lots.outline(b);
            let rb = bounds(ob);
            let (x0, y0, x1, y1) = (
                ra[0].max(rb[0]),
                ra[1].max(rb[1]),
                ra[2].min(rb[2]),
                ra[3].min(rb[3]),
            );
            let mut y = y0 + 0.5;
            while y < y1 {
                let mut x = x0 + 0.5;
                while x < x1 {
                    let p = (x, y);
                    let deep = |o: &[(f32, f32)]| {
                        inside(o, p)
                            && o.iter()
                                .zip(o.iter().cycle().skip(1))
                                .all(|(&u, &v)| seg_dist(u, v, p) > 0.05)
                    };
                    assert!(!(deep(oa) && deep(ob)), "lots {a} and {b} overlap at {p:?}");
                    x += 2.0;
                }
                y += 2.0;
            }
        }
    }
    lots
}

fn area(o: &[(f32, f32)]) -> f32 {
    o.iter()
        .zip(o.iter().cycle().skip(1))
        .map(|(a, b)| a.0 * b.1 - b.0 * a.1)
        .sum::<f32>()
        .abs()
        / 2.0
}

/// The centre of the lot's area.
fn centroid(o: &[(f32, f32)]) -> (f32, f32) {
    let (mut a, mut cx, mut cy) = (0.0, 0.0, 0.0);
    for (p, q) in o.iter().zip(o.iter().cycle().skip(1)) {
        let c = p.0 * q.1 - q.0 * p.1;
        a += c;
        cx += (p.0 + q.0) * c;
        cy += (p.1 + q.1) * c;
    }
    (cx / (3.0 * a), cy / (3.0 * a))
}

fn bounds(o: &[(f32, f32)]) -> [f32; 4] {
    o.iter()
        .fold([f32::MAX, f32::MAX, f32::MIN, f32::MIN], |r, p| {
            [r[0].min(p.0), r[1].min(p.1), r[2].max(p.0), r[3].max(p.1)]
        })
}

fn inside(o: &[(f32, f32)], p: (f32, f32)) -> bool {
    let mut c = false;
    for (a, b) in o.iter().zip(o.iter().cycle().skip(1)) {
        if (a.1 > p.1) != (b.1 > p.1) && p.0 < a.0 + (p.1 - a.1) / (b.1 - a.1) * (b.0 - a.0) {
            c = !c;
        }
    }
    c
}

fn seg_dist(a: (f32, f32), b: (f32, f32), p: (f32, f32)) -> f32 {
    let (dx, dy) = (b.0 - a.0, b.1 - a.1);
    let len2 = dx * dx + dy * dy;
    let t = if len2 > 0.0 {
        (((p.0 - a.0) * dx + (p.1 - a.1) * dy) / len2).clamp(0.0, 1.0)
    } else {
        0.0
    };
    (a.0 + dx * t - p.0).hypot(a.1 + dy * t - p.1)
}

/// The outline is one ring whose edges cross only where neighbours meet.
fn assert_simple(lot: LotId, o: &[(f32, f32)]) {
    let n = o.len();
    let cross = |a: (f32, f32), b: (f32, f32), c: (f32, f32)| {
        (b.0 - a.0) * (c.1 - a.1) - (b.1 - a.1) * (c.0 - a.0)
    };
    for i in 0..n {
        for j in i + 2..n {
            if i == 0 && j == n - 1 {
                continue;
            }
            let (a, b, c, d) = (o[i], o[(i + 1) % n], o[j], o[(j + 1) % n]);
            let proper =
                cross(a, b, c) * cross(a, b, d) < 0.0 && cross(c, d, a) * cross(c, d, b) < 0.0;
            assert!(!proper, "lot {lot} crosses itself");
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
        class: RoadClass::Street
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

/// The sharpest corner of the outline, degrees.
fn sharpest(o: &[(f32, f32)]) -> f32 {
    let n = o.len();
    (0..n)
        .map(|i| {
            let (p, c, q) = (o[(i + n - 1) % n], o[i], o[(i + 1) % n]);
            let (a, b) = ((p.0 - c.0, p.1 - c.1), (q.0 - c.0, q.1 - c.1));
            let cos = (a.0 * b.0 + a.1 * b.1) / (a.0.hypot(a.1) * b.0.hypot(b.1));
            cos.clamp(-1.0, 1.0).acos().to_degrees()
        })
        .fold(f32::MAX, f32::min)
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

#[test]
fn industrial_blocks_mix_small_medium_and_factory_sized_lots() {
    let mut w = empty();
    rect(&mut w, 0.0, 0.0, 240.0, 120.0);
    fill(&mut w, 40.0, 40.0, Zone::IndustrialHigh);
    check_lots(&w);
    let widths: Vec<_> = w
        .lots
        .active_lots()
        .map(|l| w.lots.frontage[l as usize])
        .collect();
    assert!(widths.iter().any(|&v| v < 24.0), "keep small workshops");
    assert!(
        widths.iter().any(|&v| (24.0..48.0).contains(&v)),
        "add warehouses"
    );
    assert!(widths.iter().any(|&v| v >= 48.0), "add factory-sized lots");
    assert_eq!(unlotted(&w), 0);
}

#[test]
fn industrial_curves_keep_connected_lots_and_stable_ids_after_repainting() {
    let mut w = empty();
    road(&mut w, 0.0, 0.0, 240.0, 0.0);
    road(&mut w, 240.0, 0.0, 240.0, 100.0);
    road(&mut w, 0.0, 100.0, 0.0, 0.0);
    w.apply(Command::BuildCurve {
        ax: 0.0,
        ay: 100.0,
        cx: 120.0,
        cy: 140.0,
        bx: 240.0,
        by: 100.0,
        class: RoadClass::Street,
    });
    fill(&mut w, 50.0, 50.0, Zone::IndustrialLow);
    let ids = check_lots(&w);
    assert!(ids.iter().any(|&l| w.lots.frontage[l as usize] > 24.0));
    fill(&mut w, 50.0, 50.0, Zone::ResidentialHigh);
    road(&mut w, 0.0, 0.0, -80.0, 0.0);
    assert_eq!(
        check_lots(&w),
        ids,
        "an unchanged curved factory site keeps its identity"
    );
}

#[test]
fn road_edits_give_replacement_lots_the_current_zones_sizes() {
    for zone in [Zone::IndustrialHigh, Zone::ResidentialLow] {
        let mut w = empty();
        rect(&mut w, 0.0, 0.0, 240.0, 120.0);
        fill(&mut w, 40.0, 40.0, Zone::IndustrialHigh);
        fill(&mut w, 40.0, 40.0, zone);
        let old_slots = w.lots.len();
        // This joins and splits a frontage road from outside the block.
        road(&mut w, 120.0, 0.0, 120.0, -80.0);
        check_lots(&w);
        let replacements: Vec<_> = w
            .lots
            .active_lots()
            .filter(|&l| l as usize >= old_slots)
            .map(|l| w.lots.frontage[l as usize])
            .collect();
        assert!(!replacements.is_empty());
        if zone == Zone::ResidentialLow {
            assert!(
                replacements.iter().all(|&v| v < 24.0),
                "retired factory plots must not become oversized house plots: {replacements:?}"
            );
        } else {
            assert!(
                replacements.iter().filter(|&&v| v >= 24.0).count() >= 2,
                "replacement industry still needs a mix of site sizes: {replacements:?}"
            );
        }
    }
}

#[test]
fn industrial_lot_layout_is_seeded_but_residential_layout_is_unchanged() {
    let layout = |seed, zone| {
        let mut w = empty();
        w.cfg.seed = seed;
        rect(&mut w, 0.0, 0.0, 240.0, 120.0);
        fill(&mut w, 40.0, 40.0, zone);
        let mut out = Vec::new();
        w.lots.write_outlines(&mut out);
        out
    };
    assert_eq!(
        layout(1, Zone::IndustrialHigh),
        layout(1, Zone::IndustrialHigh)
    );
    assert_ne!(
        layout(1, Zone::IndustrialHigh),
        layout(2, Zone::IndustrialHigh)
    );
    assert_eq!(
        layout(1, Zone::ResidentialLow),
        layout(2, Zone::ResidentialLow)
    );
}

#[test]
fn industrial_lot_sizes_do_not_swallow_other_zones_or_change_on_repaint() {
    let mut w = empty();
    rect(&mut w, 0.0, 0.0, 240.0, 120.0);
    // Paint both uses before cutting to exercise a mixed block.
    w.zones.paint(0.0, 0.0, 120.0, 120.0, Zone::ResidentialLow);
    w.apply(Command::PaintZone {
        ax: 120.0,
        ay: 0.0,
        bx: 240.0,
        by: 120.0,
        zone: Zone::IndustrialHigh,
    });
    let ids = check_lots(&w);
    for &l in &ids {
        if w.lots.frontage[l as usize] > 24.0 {
            assert!(
                w.lots
                    .cells(l)
                    .iter()
                    .all(|&c| w.zones.zone(c) == Zone::IndustrialHigh)
            );
        }
    }
    let mut before = Vec::new();
    w.lots.write_outlines(&mut before);
    fill(&mut w, 40.0, 40.0, Zone::CommercialHigh);
    let mut after = Vec::new();
    w.lots.write_outlines(&mut after);
    assert_eq!(check_lots(&w), ids);
    assert_eq!(
        after, before,
        "repainting must not repartition existing lots"
    );
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
