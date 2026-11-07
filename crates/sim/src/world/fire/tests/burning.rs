//! Fires starting, burning down, ruins and regrowth, without trucks.

use super::*;
use crate::world::fire::{BURN_S, FIRE_THRESHOLD, RUIN_S, burn::risk};

/// Seconds until the first fire starts, if one does within `limit`.
fn first_fire(w: &mut World, limit: u32) -> Option<u32> {
    (0..limit).find(|_| {
        advance(w, 1);
        w.fire_stats().burning > 0
    })
}

#[test]
fn no_fire_starts_below_the_population_threshold_or_with_fires_off() {
    let mut w = town(600);
    assert!(w.residents.population < FIRE_THRESHOLD);
    assert_eq!(first_fire(&mut w, 3_000), None);
    assert!(
        w.residents.population < FIRE_THRESHOLD,
        "the fixture stays small"
    );

    let mut off = town(600);
    assert!(off.apply(Command::SetFires { on: false }));
    assert!(!off.apply(Command::SetFires { on: false }), "already off");
    off.residents.population = 10 * FIRE_THRESHOLD;
    assert_eq!(first_fire(&mut off, 3_000), None);
    assert!(!off.fire_stats().enabled);

    // Above the threshold fires start, and turning fires off puts them out.
    let mut on = town(600);
    on.residents.population = 10 * FIRE_THRESHOLD;
    assert!(first_fire(&mut on, 3_000).is_some());
    assert!(on.apply(Command::SetFires { on: false }));
    assert_eq!(on.fire_stats().burning, 0);
    assert_eq!(on.fire_stats().destroyed, 0);
}

#[test]
fn industry_and_density_burn_more_often() {
    assert!(risk(Zone::ResidentialLow) < risk(Zone::ResidentialHigh));
    assert!(risk(Zone::CommercialLow) < risk(Zone::CommercialHigh));
    assert!(risk(Zone::ResidentialHigh) < risk(Zone::IndustrialLow));
    assert!(risk(Zone::IndustrialLow) < risk(Zone::IndustrialHigh));
    // Counted over many checks in the town: industry (low) outburns dense homes, which
    // outburn low commerce.
    let mut w = town(600);
    w.residents.population = 10 * FIRE_THRESHOLD;
    let mut started = [0u32; 7];
    for _ in 0..100_000 {
        w.ignite();
        for &b in &w.fire.fire_building {
            started[w.buildings.zone[b as usize] as usize] += 1;
        }
        w.fire.fire_building.clear();
        w.fire.fire_until.clear();
        w.fire.fire_truck.clear();
    }
    let per = |zone: Zone| {
        let n = standing(&w)
            .iter()
            .filter(|&&b| w.buildings.zone[b as usize] == zone)
            .count();
        started[zone as usize] as f32 / n as f32
    };
    let (ind, res, com) = (
        per(Zone::IndustrialLow),
        per(Zone::ResidentialHigh),
        per(Zone::CommercialLow),
    );
    assert!(ind > res && res > com, "per building: {ind} {res} {com}");
}

#[test]
fn an_unattended_fire_destroys_its_building_and_moves_or_removes_its_households() {
    let mut w = town(900);
    let b = busiest_home(&w);
    let lot = w.buildings.lot[b as usize];
    let living = households(&w, b);
    assert!(living > 0);
    let (population, vacant) = (w.residents.population, w.residents.vacant.len());
    let until = w.totals.tick + w.secs_to_ticks(BURN_S);
    w.start_fire(b, until);
    assert_eq!(w.fire_stats().unattended, 1, "no station covers it");
    advance(&mut w, BURN_S as u32 - 1);
    assert!(w.buildings.active[b as usize], "still burning");
    advance(&mut w, 2);
    assert!(!w.buildings.active[b as usize]);
    let s = w.fire_stats();
    assert_eq!((s.burning, s.destroyed, s.saved), (0, 1, 0));
    assert!(w.fire.ruined(lot));
    assert_eq!(households(&w, b), 0);
    // Every household moved into a vacant home or left the city with its members.
    let moved = living.min(vacant);
    assert!(
        w.residents.population <= population,
        "{population} -> {}",
        w.residents.population
    );
    assert!(moved == living || w.residents.population < population);
    assert!(w.vacancies_hold());
}

#[test]
fn a_burnt_workplace_leaves_its_workers_unemployed() {
    let mut w = town(900);
    let b = standing(&w)
        .into_iter()
        .max_by_key(|&b| w.residents.workers[b as usize])
        .unwrap();
    let workers = w.residents.workers[b as usize];
    assert!(workers > 0);
    let employed = w.residents.employed;
    w.start_fire(b, w.totals.tick + 1);
    w.tick();
    w.tick();
    assert!(!w.buildings.active[b as usize]);
    assert!(w.citizens.job.iter().all(|&j| j != Some(b)));
    assert!(
        w.residents.employed <= employed - workers + 1,
        "they search again as usual"
    );
}

#[test]
fn ruins_hold_the_lot_for_a_day_then_it_regrows() {
    let mut w = town(900);
    let b = busiest_home(&w);
    let lot = w.buildings.lot[b as usize];
    w.start_fire(b, w.totals.tick + 1);
    advance(&mut w, 1);
    assert!(w.fire.ruined(lot));
    // The fixture is short of jobs, so demand alone would hold the lot; take it out of the way.
    w.cfg.ignore_demand = true;
    let on_lot = |w: &World| {
        standing(w)
            .into_iter()
            .any(|b| w.buildings.lot[b as usize] == lot)
    };
    let mut ruins = Vec::new();
    w.write_ruins(&mut ruins);
    assert_eq!(ruins[0] as usize, w.lots.outline(lot).len());
    advance(&mut w, RUIN_S as u32 - 10);
    assert!(!on_lot(&w) && w.fire.ruined(lot));
    advance(&mut w, 20);
    assert!(!w.fire.ruined(lot));
    assert!(
        (0..60).any(|_| {
            advance(&mut w, 60);
            on_lot(&w)
        }),
        "the lot regrows under the normal gates"
    );
}

#[test]
fn bulldozing_ruins_clears_them_early() {
    let mut w = town(900);
    let b = busiest_home(&w);
    let lot = w.buildings.lot[b as usize];
    w.start_fire(b, w.totals.tick + 1);
    advance(&mut w, 1);
    let ring = w.lots.outline(lot);
    let n = ring.len() as f32;
    let (x, y) = ring
        .iter()
        .fold((0.0, 0.0), |(x, y), p| (x + p.0 / n, y + p.1 / n));
    let mut preview = Vec::new();
    w.write_bulldoze_preview(x, y, &mut preview);
    assert_eq!(preview.len(), 2 * (ring.len() + 1), "the ruin's outline");
    let roads = w.roads.active_roads().count();
    assert!(w.apply(Command::DeleteRoad { x, y }));
    assert!(!w.fire.ruined(lot));
    assert_eq!(
        w.roads.active_roads().count(),
        roads,
        "no road was taken instead"
    );
}
