//! Low-density commerce is a small-shop district, not full-lot commercial blocks.
use sim::{Command, Config, World, Zone};

fn shops(zone: Zone) -> World {
    let mut w = World::new(
        Config {
            ignore_demand: true,
            ..Config::default()
        },
        sim::road::RoadNetwork::generate_lattice(4, 3, 120.0),
        sim::rng::Rng::new(1),
    );
    for x in [60.0, 180.0, 300.0] {
        assert!(w.apply(Command::FillBlock { x, y: 60.0, zone }));
    }
    for _ in 0..1200 {
        w.tick();
    }
    w
}

fn shares_wall(a: &[(f32, f32)], b: &[(f32, f32)]) -> bool {
    for (i, p) in a.iter().enumerate() {
        let q = a[(i + 1) % a.len()];
        let (dx, dy) = (q.0 - p.0, q.1 - p.1);
        let length = dx.hypot(dy);
        for (j, r) in b.iter().enumerate() {
            let s = b[(j + 1) % b.len()];
            let cross = |t: (f32, f32)| ((t.0 - p.0) * dy - (t.1 - p.1) * dx).abs() / length;
            if cross(*r) > 0.01 || cross(s) > 0.01 {
                continue;
            }
            let project = |t: (f32, f32)| ((t.0 - p.0) * dx + (t.1 - p.1) * dy) / length;
            let (r, s) = (project(*r), project(s));
            if r.max(s).min(length) - r.min(s).max(0.0) > 1.0 {
                return true;
            }
        }
    }
    false
}

#[test]
fn low_shops_are_shallow_rectangles_mostly_one_storey_with_some_pairs() {
    let w = shops(Zone::CommercialLow);
    let buildings: Vec<_> = w.buildings.active_buildings().collect();
    assert!(buildings.len() > 12);
    let mut one_storey = 0;
    let mut attached = 0;
    let mut jobs = 0;
    for &b in &buildings {
        let ring = w.buildings.footprint(b);
        assert_eq!(ring.len(), 4, "shops must not fill wedge-shaped lots");
        for i in 0..4 {
            let (p, q) = (ring[i], ring[(i + 1) % 4]);
            assert!(
                (p.0 - q.0).hypot(p.1 - q.1) <= 18.01,
                "a shop is not a deep block"
            );
        }
        if w.buildings.height[b as usize] == 4.0 {
            one_storey += 1;
        }
        let neighbors = buildings
            .iter()
            .filter(|&&other| other != b && shares_wall(ring, w.buildings.footprint(other)))
            .count();
        assert!(
            neighbors <= 1,
            "attached groups stop at two shops, not continuous rows"
        );
        attached += usize::from(neighbors == 1);
        let info = w.building_info(b).unwrap();
        jobs += info.capacity as usize;
        assert!(
            (2..=11).contains(&info.capacity),
            "staffing stays proportional to a small trading floor"
        );
    }
    assert!(
        (buildings.len() * 5..=buildings.len() * 10).contains(&jobs),
        "ordinary shops support small teams, not just two employees"
    );
    assert!(
        one_storey * 10 >= buildings.len() * 6,
        "most shops have one storey"
    );
    assert!(attached > 0, "the district includes some attached shops");
    assert!(
        attached * 2 < buildings.len(),
        "most shops stand separately"
    );
}

#[test]
fn high_commerce_keeps_large_footprints_and_inspection_stays_sim_owned() {
    let w = shops(Zone::CommercialHigh);
    assert!(w.buildings.active_buildings().any(|b| {
        let ring = w.buildings.footprint(b);
        (0..ring.len()).any(|i| {
            let (p, q) = (ring[i], ring[(i + 1) % ring.len()]);
            (p.0 - q.0).hypot(p.1 - q.1) > 25.0
        }) && w.buildings.height[b as usize] >= 12.0
    }));
    let again = shops(Zone::CommercialLow);
    assert_eq!(again.state_hash(), shops(Zone::CommercialLow).state_hash());
}
