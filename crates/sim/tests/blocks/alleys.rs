//! Alleys and connector streets through blocks too deep to reach from their edges.

use std::collections::BTreeSet;

use sim::lot::LotId;
use sim::road::ALLEY_SPEED;
use sim::{Command, Zone};

use super::{check_lots, empty, fill, rect, road, unlotted};

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
