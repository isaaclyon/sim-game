//! Money (ADR 0036): roads cost money to build and keep, residents and filled jobs pay taxes,
//! a flat grant arrives every hour whatever the city's size, and a road the city cannot afford
//! is refused.
use sim::road::RoadClass;
use sim::world::{GRANT, JOB_TAX, Money, RESIDENT_TAX, START_FUNDS};
use sim::{Command, Config, Era, World, Zone};

/// Ticks in one clock hour at the default `dt` (`sim::clock`: 18 clock seconds per sim second).
const HOUR: u32 = 2_000;

fn advance(w: &mut World, ticks: u32) {
    for _ in 0..ticks {
        w.tick();
    }
}

fn road(w: &mut World, ax: f32, ay: f32, bx: f32, by: f32, class: RoadClass) -> bool {
    w.apply(Command::BuildRoad {
        ax,
        ay,
        bx,
        by,
        class,
    })
}

fn money(w: &World) -> Money {
    w.stats().money
}

#[test]
fn a_fresh_city_is_an_empty_map_with_starting_funds() {
    let w = World::empty(Config::default(), 60, 60);
    assert_eq!(w.roads.road_count(), 0);
    let [x0, y0, x1, y1] = w.bounds();
    assert!(x0 <= 0.0 && y0 <= 0.0 && x1 >= 59.0 * 120.0 && y1 >= 59.0 * 120.0);
    assert_eq!(
        money(&w),
        Money {
            balance: START_FUNDS,
            taxes: 0,
            grant: GRANT,
            upkeep: 0,
            utilities: 0,
        }
    );
}

#[test]
fn the_grant_arrives_every_clock_hour_before_anything_is_built() {
    let mut w = World::empty(Config::default(), 20, 20);
    advance(&mut w, HOUR / 2);
    assert_eq!(money(&w).balance, START_FUNDS + GRANT / 2);
    advance(&mut w, HOUR / 2);
    assert_eq!(money(&w).balance, START_FUNDS + GRANT);
}

#[test]
fn building_charges_what_the_preview_quotes_by_length_and_class() {
    // Town rates; a village pays half (ADR 0056, tests/eras.rs).
    let mut w = World::empty(Config::default(), 20, 20).with_era(Era::Town);
    let quote = w.road_cost(0.0, 0.0, 1000.0, 0.0, RoadClass::Street);
    assert_eq!(quote, Some(10_000));
    assert!(road(&mut w, 0.0, 0.0, 1000.0, 0.0, RoadClass::Street));
    assert_eq!(money(&w).balance, START_FUNDS - 10_000);

    let main = w.road_cost(500.0, -200.0, 500.0, 200.0, RoadClass::Main);
    assert_eq!(main, Some(10_000), "main roads cost 25 per metre");
    let curve = w.curve_cost(0.0, 400.0, 200.0, 600.0, 400.0, 400.0, RoadClass::Street);
    assert!(curve.is_some_and(|c| c > 4_000), "a curve costs its length");
    assert_eq!(
        w.road_cost(f32::NAN, 0.0, 1.0, 1.0, RoadClass::Street),
        None
    );
}

#[test]
fn a_crossing_pays_for_its_whole_length_and_a_duplicate_is_refused() {
    // Town rates; a village pays half (ADR 0056, tests/eras.rs).
    let mut w = World::empty(Config::default(), 20, 20).with_era(Era::Town);
    assert!(road(&mut w, 0.0, 0.0, 500.0, 0.0, RoadClass::Street));
    assert_eq!(w.road_cost(0.0, 0.0, 500.0, 0.0, RoadClass::Street), None);
    assert_eq!(
        w.road_cost(250.0, -100.0, 250.0, 100.0, RoadClass::Street),
        Some(2_000),
        "a crossing pays for its whole length"
    );
}

#[test]
fn a_road_the_city_cannot_afford_is_refused_and_changes_nothing() {
    // The town era unlocks main roads (ADR 0056), so only money refuses this one.
    let mut w = World::empty(Config::default(), 60, 60).with_era(Era::Town);
    let hash = w.state_hash();
    // 7 km of main road at $25/m is $175,000, more than a fresh city has.
    assert_eq!(
        w.road_cost(0.0, 0.0, 7000.0, 0.0, RoadClass::Main),
        Some(175_000)
    );
    assert!(!road(&mut w, 0.0, 0.0, 7000.0, 0.0, RoadClass::Main));
    assert_eq!(w.roads.road_count(), 0);
    assert_eq!(w.state_hash(), hash);
    assert!(w.can_afford(START_FUNDS) && !w.can_afford(START_FUNDS + 1));
}

#[test]
fn upkeep_is_charged_each_clock_hour() {
    // Town rates; a village pays half (ADR 0056, tests/eras.rs).
    let mut w = World::empty(Config::default(), 60, 60).with_era(Era::Town);
    assert!(road(&mut w, 0.0, 0.0, 1000.0, 0.0, RoadClass::Street));
    assert_eq!(
        money(&w).upkeep,
        100,
        "streets cost 0.10 per metre per hour"
    );
    advance(&mut w, HOUR);
    assert_eq!(money(&w).balance, START_FUNDS - 10_000 - 100 + GRANT);
}

#[test]
fn upkeep_beyond_the_grant_runs_into_debt_and_stops_new_roads() {
    // A generated grid carries far more road than the grant pays for.
    let mut w = World::generate(Config::default(), 20, 20);
    let m = money(&w);
    assert_eq!(m.balance, START_FUNDS);
    let per_hour = m.upkeep - GRANT;
    assert!(per_hour > 0, "upkeep must outrun the grant: {m:?}");
    let hours = (m.balance / per_hour + 1) as u32;
    advance(&mut w, hours * HOUR);
    let m = money(&w);
    assert_eq!(m.balance, START_FUNDS - hours as i64 * per_hour);
    assert!(m.balance < 0, "upkeep keeps charging in debt: {m:?}");
    let (ax, ay, bx, by) = (40.0, 40.0, 80.0, 80.0);
    assert!(w.road_cost(ax, ay, bx, by, RoadClass::Street).is_some());
    assert!(!road(&mut w, ax, ay, bx, by, RoadClass::Street));
}

#[test]
fn a_city_in_debt_can_still_make_free_edits() {
    let mut w = World::generate(Config::default(), 20, 20);
    let per_hour = money(&w).upkeep - GRANT;
    advance(&mut w, (START_FUNDS / per_hour + 1) as u32 * HOUR);
    let balance = money(&w).balance;
    assert!(balance < 0, "{balance}");
    assert!(w.can_afford(0) && !w.can_afford(1));

    // Drawing a street over a main road is a free downgrade (ADR 0048).
    let main = w
        .roads
        .active_roads()
        .find(|&r| w.roads.road_class(r) == RoadClass::Main)
        .expect("the grid has main roads");
    let (a, b) = w.roads.road_ends(main);
    let n = &w.roads;
    let (ax, ay, bx, by) = (
        n.node_x[a as usize],
        n.node_y[a as usize],
        n.node_x[b as usize],
        n.node_y[b as usize],
    );
    let ends = [ax, ay, 0.0, bx, by, 0.0];
    assert_eq!(w.draft_cost(&ends, RoadClass::Street), Some(0));
    assert!(road(&mut w, ax, ay, bx, by, RoadClass::Street));
    assert_eq!(w.roads.road_class(main), RoadClass::Street);
    assert_eq!(money(&w).balance, balance);

    // Upgrading it back costs money, so debt still refuses it.
    assert!(!road(&mut w, ax, ay, bx, by, RoadClass::Main));
    assert_eq!(w.roads.road_class(main), RoadClass::Street);
}

#[test]
fn upkeep_accrues_by_the_minute_without_losing_fractions() {
    // Town rates; a village pays half (ADR 0056, tests/eras.rs).
    let mut w = World::empty(Config::default(), 20, 20).with_era(Era::Town);
    assert!(road(&mut w, 0.0, 0.0, 300.0, 0.0, RoadClass::Street));
    let start = money(&w).balance;
    assert_eq!(money(&w).upkeep, 30);
    advance(&mut w, HOUR / 2);
    assert_eq!(money(&w).balance, start + (GRANT - 30) / 2);
    advance(&mut w, HOUR / 2);
    assert_eq!(money(&w).balance, start + GRANT - 30);
}

#[test]
fn bulldozing_is_free_and_ends_the_road_upkeep() {
    let mut w = World::empty(Config::default(), 20, 20);
    assert!(road(&mut w, 0.0, 0.0, 1000.0, 0.0, RoadClass::Street));
    let balance = money(&w).balance;
    assert!(w.apply(Command::DeleteRoad { x: 500.0, y: 0.0 }));
    assert_eq!(money(&w).balance, balance);
    assert_eq!(money(&w).upkeep, 0);
}

#[test]
fn residents_and_filled_jobs_pay_taxes() {
    let mut w = World::empty(Config::default(), 20, 20);
    for i in 0..4 {
        let at = i as f32 * 120.0;
        assert!(road(&mut w, 0.0, at, 360.0, at, RoadClass::Street));
        assert!(road(&mut w, at, 0.0, at, 360.0, RoadClass::Street));
    }
    for (x, y, zone) in [
        (60.0, 60.0, Zone::ResidentialLow),
        (180.0, 60.0, Zone::ResidentialLow),
        (300.0, 60.0, Zone::CommercialLow),
        (60.0, 180.0, Zone::IndustrialLow),
    ] {
        assert!(w.apply(Command::FillBlock { x, y, zone }));
    }
    advance(&mut w, HOUR);
    let s = w.stats();
    assert!(s.population > 0 && s.unemployed < s.population, "{s:?}");
    // Every resident pays; only adults hold jobs (ADR 0054).
    let employed = (s.population - s.children - s.unemployed) as i64;
    assert_eq!(
        s.money.taxes,
        s.population as i64 * RESIDENT_TAX + employed * JOB_TAX
    );
}

#[test]
fn money_survives_a_save_and_load() {
    let mut w = World::empty(Config::default(), 20, 20);
    assert!(road(&mut w, 0.0, 0.0, 330.0, 0.0, RoadClass::Street));
    advance(&mut w, 1_234);
    let loaded = World::load(&w.save().expect("saves")).expect("loads");
    assert_eq!(money(&loaded), money(&w));
    assert_eq!(loaded.state_hash(), w.state_hash());
    let (mut a, mut b) = (w, loaded);
    advance(&mut a, HOUR);
    advance(&mut b, HOUR);
    assert_eq!(money(&a), money(&b));
}
