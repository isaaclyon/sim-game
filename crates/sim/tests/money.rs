//! Money (ADR 0036): roads cost money to build and keep, residents and filled jobs pay taxes,
//! and a road the city cannot afford is refused.
use sim::road::RoadClass;
use sim::world::{JOB_TAX, Money, RESIDENT_TAX, START_FUNDS};
use sim::{Command, Config, World, Zone};

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
            upkeep: 0
        }
    );
}

#[test]
fn building_charges_what_the_preview_quotes_by_length_and_class() {
    let mut w = World::empty(Config::default(), 20, 20);
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
    let mut w = World::empty(Config::default(), 20, 20);
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
    let mut w = World::empty(Config::default(), 60, 60);
    let hash = w.state_hash();
    assert!(!road(&mut w, 0.0, 0.0, 5000.0, 0.0, RoadClass::Main));
    assert_eq!(w.roads.road_count(), 0);
    assert_eq!(w.state_hash(), hash);
    assert!(w.can_afford(START_FUNDS) && !w.can_afford(START_FUNDS + 1));
}

#[test]
fn upkeep_is_charged_each_clock_hour_and_can_run_into_debt() {
    let mut w = World::empty(Config::default(), 60, 60);
    assert!(road(&mut w, 0.0, 0.0, 1000.0, 0.0, RoadClass::Street));
    assert_eq!(
        money(&w).upkeep,
        100,
        "streets cost 0.10 per metre per hour"
    );
    advance(&mut w, HOUR);
    assert_eq!(money(&w).balance, START_FUNDS - 10_000 - 100);

    // Spend almost everything on main roads, then let upkeep push the balance below zero.
    for y in [200.0, 400.0, 600.0] {
        assert!(road(&mut w, 0.0, y, 500.0, y, RoadClass::Main));
    }
    let left = money(&w).balance;
    assert_eq!(left, START_FUNDS - 10_000 - 100 - 37_500);
    let per_hour = money(&w).upkeep;
    assert_eq!(per_hour, 100 + 375);
    let hours = (left / per_hour + 1) as u32;
    advance(&mut w, hours * HOUR);
    let m = money(&w);
    assert_eq!(m.balance, left - hours as i64 * per_hour);
    assert!(m.balance < 0, "upkeep keeps charging in debt: {m:?}");
    assert!(!road(&mut w, 0.0, 800.0, 20.0, 800.0, RoadClass::Street));
}

#[test]
fn upkeep_accrues_by_the_minute_without_losing_fractions() {
    let mut w = World::empty(Config::default(), 20, 20);
    assert!(road(&mut w, 0.0, 0.0, 300.0, 0.0, RoadClass::Street));
    let start = money(&w).balance;
    assert_eq!(money(&w).upkeep, 30);
    advance(&mut w, HOUR / 2);
    assert_eq!(money(&w).balance, start - 15);
    advance(&mut w, HOUR / 2);
    assert_eq!(money(&w).balance, start - 30);
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
    let employed = (s.population - s.unemployed) as i64;
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
