//! Filling a block with one click, and how blocks follow their roads.

use sim::road::RoadClass;
use sim::{Command, Zone};

use super::{check_lots, empty, fill, rect, road, zoned};

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
