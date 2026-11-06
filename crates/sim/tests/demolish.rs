//! The bulldozer demolishes the building under the cursor (ADR 0045): at once, for free, and
//! the still-zoned lot regrows with a fresh design.
use sim::{Command, Config, World, Zone};

fn town(zone: Zone) -> World {
    let cfg = Config {
        ignore_demand: true,
        ..Config::default()
    };
    let mut w = World::generate(cfg, 3, 3);
    assert!(w.apply(Command::FillBlock {
        x: 50.0,
        y: 50.0,
        zone
    }));
    advance(&mut w, 61);
    w
}

fn advance(w: &mut World, seconds: u32) {
    for _ in 0..seconds * 10 {
        w.tick();
    }
}

/// The footprint's vertex average; every grown footprint is convex enough to contain it.
fn inside(w: &World, b: u32) -> (f32, f32) {
    let ring = w.buildings.footprint(b);
    let n = ring.len() as f32;
    let sum = ring.iter().fold((0.0, 0.0), |s, p| (s.0 + p.0, s.1 + p.1));
    (sum.0 / n, sum.1 / n)
}

fn bulldoze(w: &mut World, (x, y): (f32, f32)) -> bool {
    w.apply(Command::DeleteRoad { x, y })
}

fn demolish(w: &mut World, b: u32) -> bool {
    let at = inside(w, b);
    bulldoze(w, at)
}

#[test]
fn preview_outlines_the_building_under_the_cursor() {
    let w = town(Zone::ResidentialHigh);
    let b = w.buildings.active_buildings().next().unwrap();
    let (x, y) = inside(&w, b);
    let mut preview = Vec::new();
    w.write_bulldoze_preview(x, y, &mut preview);
    let ring = w.buildings.footprint(b);
    let closed: Vec<f32> = ring
        .iter()
        .chain(&ring[..1])
        .flat_map(|p| [p.0, p.1])
        .collect();
    assert_eq!(preview, closed, "a closed outline of the footprint");
}

#[test]
fn bulldozing_a_building_removes_it_now_and_keeps_roads_and_money() {
    let mut w = town(Zone::ResidentialHigh);
    let b = (0..w.homes.building.len())
        .find(|&h| w.homes.household[h].is_some())
        .map(|h| w.homes.building[h])
        .expect("an occupied home");
    let roads = w.roads.version;
    let balance = w.stats().money.balance;
    let version = w.buildings.version;
    let standing = w.buildings.active_buildings().count();
    assert!(demolish(&mut w, b));
    assert!(!w.buildings.active[b as usize]);
    assert_ne!(w.buildings.version, version, "geometry is resent");
    assert_eq!(w.buildings.active_buildings().count(), standing - 1);
    assert_eq!(w.roads.version, roads, "no road was touched");
    assert_eq!(w.stats().money.balance, balance, "demolition is free");
    for h in 0..w.homes.building.len() {
        if w.homes.building[h] == b {
            assert_eq!(w.homes.household[h], None, "nobody still lives there");
        }
    }
    for i in 0..w.citizens.active.len() {
        if w.citizens.active[i] {
            assert_ne!(w.citizens.job[i], Some(b));
            assert_ne!(w.citizens.location[i], b);
        }
    }
}

#[test]
fn demolished_lots_regrow_with_fresh_designs() {
    let mut w = town(Zone::ResidentialHigh);
    let before: Vec<_> = w
        .buildings
        .active_buildings()
        .map(|b| {
            let i = b as usize;
            (
                w.buildings.lot[i],
                w.buildings.height[i],
                w.buildings.footprint(b).to_vec(),
            )
        })
        .collect();
    for &(lot, _, _) in &before {
        let b = w
            .buildings
            .active_buildings()
            .find(|&b| w.buildings.lot[b as usize] == lot)
            .unwrap();
        assert!(demolish(&mut w, b));
    }
    assert_eq!(w.buildings.active_buildings().count(), 0);
    advance(&mut w, 61);
    let mut fresh = 0;
    for (lot, height, ring) in &before {
        let b = w
            .buildings
            .active_buildings()
            .find(|&b| w.buildings.lot[b as usize] == *lot)
            .expect("the still-zoned lot regrows");
        if w.buildings.height[b as usize] != *height || w.buildings.footprint(b) != ring {
            fresh += 1;
        }
    }
    assert!(
        fresh * 2 > before.len(),
        "{fresh}/{} redesigned",
        before.len()
    );
}

#[test]
fn demolition_is_deterministic_and_misses_change_nothing() {
    let mut a = town(Zone::CommercialHigh);
    let mut b = town(Zone::CommercialHigh);
    let target = a.buildings.active_buildings().next().unwrap();
    let at = inside(&a, target);
    assert!(bulldoze(&mut a, at));
    assert!(bulldoze(&mut b, at));
    advance(&mut a, 61);
    advance(&mut b, 61);
    assert_eq!(a.state_hash(), b.state_hash());
    let hash = a.state_hash();
    assert!(!bulldoze(&mut a, (-500.0, -500.0)));
    assert_eq!(a.state_hash(), hash);
}
