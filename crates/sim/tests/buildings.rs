//! Buildings grow on lots, close gradually when rezoned, and leave with retired lots.
use sim::road::RoadClass;
use sim::{Command, Config, World, Zone};

/// Geometry and closure tests fill whole single-use blocks, so they grow without demand.
fn full() -> Config {
    Config {
        ignore_demand: true,
        ..Config::default()
    }
}

fn town(zone: Zone) -> World {
    let mut w = World::generate(full(), 3, 3);
    assert!(w.apply(Command::FillBlock {
        x: 50.0,
        y: 50.0,
        zone
    }));
    w
}

fn advance(w: &mut World, seconds: u32) {
    for _ in 0..seconds * 10 {
        w.tick();
    }
}

fn snapshot(w: &World) -> Vec<f32> {
    let mut out = Vec::new();
    w.buildings.write_snapshot(&w.lots, &mut out);
    out
}

#[test]
fn growth_is_delayed_staggered_and_finishes_within_a_minute() {
    let mut w = town(Zone::ResidentialLow);
    let lots = w.lots.active_lots().count();
    assert!(lots > 3);
    assert!(
        snapshot(&w).is_empty(),
        "painting while paused creates no buildings"
    );
    advance(&mut w, 10);
    let early = w.buildings.active_buildings().count();
    assert!(
        early > 0 && early < lots,
        "a few buildings appear before the rest: {early}/{lots}"
    );
    advance(&mut w, 51);
    assert_eq!(w.buildings.active_buildings().count(), lots);
    let done = snapshot(&w);
    let version = w.buildings.version;
    advance(&mut w, 60);
    assert_eq!(snapshot(&w), done, "a lot receives at most one building");
    assert_eq!(
        w.buildings.version, version,
        "idle ticks do not resend geometry"
    );
}

#[test]
fn all_six_uses_grow_with_their_own_height_and_footprint() {
    for zone in [
        Zone::ResidentialLow,
        Zone::ResidentialHigh,
        Zone::CommercialLow,
        Zone::CommercialHigh,
        Zone::IndustrialLow,
        Zone::IndustrialHigh,
    ] {
        let mut w = town(zone);
        advance(&mut w, 61);
        assert!(w.buildings.active_buildings().count() > 0);
        for id in w.buildings.active_buildings() {
            let i = id as usize;
            assert_eq!(w.buildings.zone[i], zone);
            assert!(w.buildings.height[i] >= 3.0);
            let ring = w.buildings.footprint(id);
            assert!(ring.len() >= 4);
            assert!(ring.iter().all(|p| p.0.is_finite() && p.1.is_finite()));
        }
    }
}

#[test]
fn snapshot_front_edge_faces_the_lot_road() {
    for zone in [
        Zone::ResidentialLow,
        Zone::CommercialHigh,
        Zone::IndustrialLow,
    ] {
        let mut w = town(zone);
        advance(&mut w, 61);
        let data = snapshot(&w);
        let mut i = 0;
        for id in w.buildings.active_buildings() {
            let (front, n) = (data[i + 3] as usize, data[i + 4] as usize);
            assert_eq!(n, w.buildings.footprint(id).len());
            assert!(front < n);
            let at = |k: usize| (data[i + 5 + 2 * (k % n)], data[i + 6 + 2 * (k % n)]);
            let ((ax, ay), (bx, by)) = (at(front), at(front + 1));
            let len = (bx - ax).hypot(by - ay);
            // Counter-clockwise rings: the outward normal is the edge turned clockwise.
            let normal = ((by - ay) / len, -(bx - ax) / len);
            let lot = w.buildings.lot[id as usize] as usize;
            let facing = (w.lots.facing_x[lot], w.lots.facing_y[lot]);
            assert!(
                normal.0 * facing.0 + normal.1 * facing.1 > 0.7,
                "{zone:?} front edge faces its road"
            );
            i += 5 + 2 * n;
        }
        assert_eq!(i, data.len());
    }
}

#[test]
fn repaint_and_unzone_close_buildings_gradually_then_replace_them() {
    let mut w = town(Zone::ResidentialLow);
    advance(&mut w, 10);
    let original = snapshot(&w);
    let old: Vec<_> = w.buildings.active_buildings().collect();
    assert!(!original.is_empty());
    let version = w.buildings.version;
    assert!(w.apply(Command::FillBlock {
        x: 50.0,
        y: 50.0,
        zone: Zone::None
    }));
    assert_eq!(
        w.buildings.version, version,
        "a paused repaint keeps geometry"
    );
    assert!(old.iter().all(|&b| w.building_closing(b)));
    // Closure steps are 3–12 sim seconds apart (ADR 0023); a house needs at most two.
    for _ in 0..29 {
        w.tick();
    }
    assert_eq!(snapshot(&w), original);
    advance(&mut w, 22);
    assert!(snapshot(&w).is_empty());
    assert!(w.buildings.version > version);
    advance(&mut w, 61);
    assert!(snapshot(&w).is_empty(), "unzoned lots stay vacant");
    assert!(w.apply(Command::FillBlock {
        x: 50.0,
        y: 50.0,
        zone: Zone::CommercialHigh
    }));
    advance(&mut w, 61);
    let zones: Vec<_> = w
        .buildings
        .active_buildings()
        .map(|b| w.buildings.zone[b as usize])
        .collect();
    assert!(!zones.is_empty());
    assert!(zones.iter().all(|&z| z == Zone::CommercialHigh));
}

#[test]
fn restoring_the_original_zone_keeps_closing_buildings() {
    let mut w = town(Zone::ResidentialLow);
    advance(&mut w, 10);
    let old: Vec<_> = w.buildings.active_buildings().collect();
    for zone in [Zone::ResidentialHigh, Zone::ResidentialLow] {
        assert!(w.apply(Command::FillBlock {
            x: 50.0,
            y: 50.0,
            zone
        }));
    }
    assert!(old.iter().all(|&b| !w.building_closing(b)));
    advance(&mut w, 61);
    assert!(old.iter().all(|&b| w.buildings.active[b as usize]));
}

#[test]
fn road_edits_remove_retired_lots_buildings_without_waiting_for_a_tick() {
    let mut w = town(Zone::ResidentialHigh);
    advance(&mut w, 61);
    let old: Vec<_> = w.buildings.active_buildings().collect();
    let version = w.buildings.version;
    assert!(w.apply(Command::BuildRoad {
        ax: 0.0,
        ay: 60.0,
        bx: 120.0,
        by: 60.0,
        class: RoadClass::Street
    }));
    assert!(old.iter().any(|&b| !w.buildings.active[b as usize]));
    assert!(w.buildings.version > version);
    for b in w.buildings.active_buildings() {
        assert!(w.lots.active[w.buildings.lot[b as usize] as usize]);
    }
    advance(&mut w, 61);
    for b in w.buildings.active_buildings() {
        assert!(w.lots.active[w.buildings.lot[b as usize] as usize]);
    }
}

#[test]
fn growth_and_pending_work_are_deterministic_and_snapshots_are_read_only() {
    let mut a = town(Zone::ResidentialHigh);
    let mut b = town(Zone::ResidentialHigh);
    for _ in 0..610 {
        assert_eq!(a.state_hash(), b.state_hash());
        let hash = a.state_hash();
        assert_eq!(snapshot(&a), snapshot(&b));
        assert_eq!(a.state_hash(), hash);
        a.tick();
        b.tick();
    }
    let id = a.buildings.active_buildings().next().unwrap() as usize;
    a.buildings.height[id] += 1.0;
    assert_ne!(
        a.state_hash(),
        b.state_hash(),
        "building state participates in replay checks"
    );
}

#[test]
fn growth_does_not_always_build_shortest_first() {
    let mut w = town(Zone::ResidentialHigh);
    advance(&mut w, 61);
    assert!(
        w.buildings.height.windows(2).any(|h| h[0] > h[1]),
        "short and tall buildings should mix during growth"
    );
}

#[test]
fn opening_a_block_clears_its_buildings_immediately() {
    let mut w = town(Zone::CommercialHigh);
    advance(&mut w, 61);
    assert!(!snapshot(&w).is_empty());
    assert!(w.apply(Command::DeleteRoad { x: 60.0, y: 0.0 }));
    assert!(snapshot(&w).is_empty());
}

#[test]
fn curved_street_buildings_follow_lots_without_replacing_them_with_boxes() {
    let mut w = World::generate(full(), 0, 0);
    for (ax, ay, bx, by) in [
        (0.0, 0.0, 120.0, 0.0),
        (120.0, 0.0, 120.0, 120.0),
        (0.0, 120.0, 0.0, 0.0),
    ] {
        assert!(w.apply(Command::BuildRoad {
            ax,
            ay,
            bx,
            by,
            class: RoadClass::Street
        }));
    }
    assert!(w.apply(Command::BuildCurve {
        ax: 120.0,
        ay: 120.0,
        cx: 60.0,
        cy: 180.0,
        bx: 0.0,
        by: 120.0,
        class: RoadClass::Street
    }));
    assert!(w.apply(Command::FillBlock {
        x: 50.0,
        y: 50.0,
        zone: Zone::CommercialHigh
    }));
    advance(&mut w, 61);
    assert!(w.buildings.active_buildings().count() > 3);
    assert!(
        w.buildings
            .active_buildings()
            .any(|b| w.buildings.footprint(b).len() > 4)
    );
}

#[test]
fn growth_does_not_consume_traffic_randomness() {
    let mut a = World::generate(Config::default(), 3, 3);
    let mut b = town(Zone::ResidentialLow);
    for w in [&mut a, &mut b] {
        assert!(w.apply(Command::SpawnAgents { count: 100 }));
        advance(w, 61);
    }
    let (mut left, mut right) = (Vec::new(), Vec::new());
    a.write_agent_snapshot(&mut left);
    b.write_agent_snapshot(&mut right);
    assert_eq!(left, right);
    assert_eq!(a.stats().trips_completed, b.stats().trips_completed);
}

#[test]
fn industrial_buildings_vary_in_height_and_survive_repainting_and_nearby_road_edits() {
    let mut w = town(Zone::IndustrialHigh);
    advance(&mut w, 61);
    let heights: Vec<_> = w
        .buildings
        .active_buildings()
        .map(|b| w.buildings.height[b as usize])
        .collect();
    assert!(heights.windows(2).any(|h| h[0] != h[1]));
    let before = snapshot(&w);
    w.apply(Command::FillBlock {
        x: 50.0,
        y: 50.0,
        zone: Zone::ResidentialLow,
    });
    assert_eq!(snapshot(&w), before);
    // A road outside the block refreshes nearby lots, without changing its boundary.
    w.apply(Command::BuildRoad {
        ax: 0.0,
        ay: 0.0,
        bx: -80.0,
        by: 0.0,
        class: RoadClass::Street,
    });
    assert_eq!(
        snapshot(&w),
        before,
        "unchanged factory lots must keep their buildings"
    );
}

#[test]
fn industrial_halls_never_take_the_pointed_shape_of_corner_lots() {
    for zone in [Zone::IndustrialLow, Zone::IndustrialHigh] {
        let mut w = World::generate(full(), 6, 6);
        assert!(w.apply(Command::PaintZone {
            ax: -100.0,
            ay: -100.0,
            bx: 800.0,
            by: 800.0,
            zone,
        }));
        advance(&mut w, 61);
        assert!(w.buildings.active_buildings().count() > 30);
        for b in w.buildings.active_buildings() {
            let ring = w.buildings.footprint(b);
            assert_eq!(
                ring.len(),
                4,
                "industrial hall {b} must be a box, not a lot-shaped wedge"
            );
            for i in 0..4 {
                let a = ring[i];
                let p = ring[(i + 1) % 4];
                let q = ring[(i + 2) % 4];
                let u = (p.0 - a.0, p.1 - a.1);
                let v = (q.0 - p.0, q.1 - p.1);
                let cosine = (u.0 * v.0 + u.1 * v.1).abs() / (u.0.hypot(u.1) * v.0.hypot(v.1));
                assert!(cosine < 0.002, "industrial hall {b} has a pointed corner");
            }
        }
    }
}

#[test]
fn industrial_roof_choices_are_varied_persisted_and_hashed() {
    let mut a = town(Zone::IndustrialHigh);
    let mut b = town(Zone::IndustrialHigh);
    advance(&mut a, 61);
    advance(&mut b, 61);
    assert_eq!(snapshot(&a), snapshot(&b));
    assert!(a.buildings.roof.windows(2).any(|r| r[0] != r[1]));
    let hash = a.state_hash();
    let id = a.buildings.active_buildings().next().unwrap() as usize;
    a.buildings.roof[id] = (a.buildings.roof[id] + 1) % 4;
    assert_ne!(a.state_hash(), hash, "roof choice belongs to sim state");
    assert_ne!(
        snapshot(&a),
        snapshot(&b),
        "the renderer receives the persisted choice"
    );
}

#[test]
fn unzoned_towns_never_grow_buildings() {
    let mut w = World::generate(Config::default(), 3, 3);
    advance(&mut w, 120);
    assert!(snapshot(&w).is_empty());
    assert_eq!(w.buildings.version, 0);
}
