//! Pollution lowers land value, and land value steers growth (ADR 0037).
use sim::rng::Rng;
use sim::road::RoadNetwork;
use sim::{Command, Config, World, Zone};

/// Sim seconds in one day of the clock (`sim::clock`): 4 sim hours.
const DAY_S: u32 = 4 * 3600;

fn advance(w: &mut World, seconds: u32) {
    for _ in 0..seconds * 10 {
        w.tick();
    }
}

/// A 7 × 3 street lattice with 120 m blocks; block (i, j) has its centre at
/// `(60 + 120 i, 60 + 120 j)`.
fn lattice(ignore_demand: bool) -> World {
    World::new(
        Config {
            ignore_demand,
            ..Config::default()
        },
        RoadNetwork::generate_lattice(8, 4, 120.0),
        Rng::new(5),
    )
}

fn fill(w: &mut World, x: f32, y: f32, zone: Zone) {
    assert!(w.apply(Command::FillBlock { x, y, zone }));
}

fn built_in(w: &World, x0: f32, x1: f32) -> usize {
    w.buildings
        .active_buildings()
        .filter(|&b| {
            let lot = w.buildings.lot[b as usize] as usize;
            (x0..x1).contains(&w.lots.front_x[lot])
        })
        .count()
}

fn factory_town() -> World {
    let mut w = lattice(true);
    fill(&mut w, 60.0, 180.0, Zone::IndustrialHigh);
    advance(&mut w, 120);
    assert!(built_in(&w, 0.0, 120.0) > 0, "the factory block grew");
    w
}

#[test]
fn a_fresh_city_is_clean() {
    let w = lattice(false);
    let s = w.pollution_at(300.0, 180.0).expect("inside the map");
    assert_eq!((s.air, s.ground, s.noise), (0.0, 0.0, 0.0));
    assert_eq!(s.land_value, 1.0);
    assert!(w.pollution_at(1e7, 0.0).is_none());
}

#[test]
fn factories_pollute_their_surroundings_most() {
    let mut w = factory_town();
    advance(&mut w, 600);
    let near = w.pollution_at(60.0, 180.0).unwrap();
    let far = w.pollution_at(780.0, 180.0).unwrap();
    assert!(
        near.air > 0.2 && near.noise > 0.1 && near.ground > 0.0,
        "{near:?}"
    );
    assert!(near.land_value < 0.7, "{near:?}");
    assert_eq!(far.air, 0.0, "{far:?}");
    assert!(far.land_value > near.land_value);
}

#[test]
fn ground_pollution_lingers_after_the_factory_goes() {
    let mut w = factory_town();
    advance(&mut w, DAY_S / 2);
    let running = w.pollution_at(60.0, 180.0).unwrap();
    fill(&mut w, 60.0, 180.0, Zone::None);
    advance(&mut w, 1800);
    assert_eq!(built_in(&w, 0.0, 120.0), 0, "the factory closed");
    let after = w.pollution_at(60.0, 180.0).unwrap();
    assert!(
        after.air < running.air / 4.0,
        "air follows current sources: {after:?}"
    );
    assert!(
        after.ground > running.ground / 2.0,
        "ground lingers: {after:?}"
    );
    advance(&mut w, 4 * DAY_S);
    assert_eq!(
        w.pollution_at(60.0, 180.0).unwrap().ground,
        0.0,
        "and fades"
    );
}

#[test]
fn traffic_makes_noise_on_its_roads() {
    let mut quiet = lattice(false);
    advance(&mut quiet, 600);
    let mut busy = lattice(false);
    busy.apply(Command::SpawnAgents { count: 400 });
    advance(&mut busy, 600);
    let on_road = |w: &World| w.pollution_at(360.0, 240.0).unwrap();
    assert_eq!(on_road(&quiet).noise, 0.0);
    assert!(on_road(&busy).noise > 0.05, "{:?}", on_road(&busy));
}

/// An old factory site: the factories ran for half a day, closed, and their air cleared, so
/// only ground pollution is left and nobody has a job yet.
fn old_factory_site() -> World {
    let mut w = factory_town();
    advance(&mut w, DAY_S / 2);
    fill(&mut w, 60.0, 180.0, Zone::None);
    advance(&mut w, 1800);
    assert_eq!(built_in(&w, 0.0, 120.0), 0, "the factories closed");
    w.cfg.ignore_demand = false;
    w
}

#[test]
fn homes_grow_on_clean_land_before_dirty_land() {
    let mut w = old_factory_site();
    fill(&mut w, 60.0, 180.0, Zone::ResidentialLow);
    fill(&mut w, 660.0, 180.0, Zone::ResidentialLow);
    advance(&mut w, 300);
    let (dirty, clean) = (built_in(&w, 0.0, 120.0), built_in(&w, 600.0, 720.0));
    assert!(clean > 0, "clean land grows");
    assert_eq!(dirty, 0, "dirty land waits while demand is modest");
}

#[test]
fn industry_ignores_land_value() {
    let mut w = old_factory_site();
    fill(&mut w, 60.0, 180.0, Zone::IndustrialLow);
    advance(&mut w, 300);
    assert!(built_in(&w, 0.0, 120.0) > 0);
}

#[test]
fn pollution_is_deterministic_and_survives_a_save() {
    let mut a = factory_town();
    a.apply(Command::SpawnAgents { count: 200 });
    advance(&mut a, 605);
    let mut b = World::load(&a.save().unwrap()).unwrap();
    assert_eq!(a.state_hash(), b.state_hash());
    advance(&mut a, 300);
    advance(&mut b, 300);
    assert_eq!(a.state_hash(), b.state_hash());
    assert_eq!(a.pollution_at(60.0, 180.0), b.pollution_at(60.0, 180.0));
}
