//! Gradual closure of repainted or unzoned buildings (ADR 0023).
use super::*;
use crate::agents::AgentState;
use crate::world::closure::{STEP_MAX_S, STEP_MIN_S};

fn paint(w: &mut World, x: f32, zone: Zone) {
    assert!(w.apply(Command::FillBlock { x, y: 50.0, zone }));
}

fn in_block(w: &World, zone: Zone) -> Vec<u32> {
    w.buildings
        .active_buildings()
        .filter(|&b| w.buildings.zone[b as usize] == zone)
        .collect()
}

/// Every count the stats report agrees with citizen state, and nobody refers to a removed
/// building or holds a job at one.
fn consistent(w: &World) {
    let active = (0..w.citizens.active.len()).filter(|&i| w.citizens.active[i]);
    let employed = active
        .clone()
        .filter(|&i| w.citizens.job[i].is_some())
        .count() as u32;
    assert_eq!(w.residents.population, active.clone().count() as u32);
    assert_eq!(w.residents.employed, employed);
    assert_eq!(employed, w.residents.workers.iter().sum::<u32>());
    for i in active {
        let on = |b: u32| w.buildings.active[b as usize];
        assert!(on(w.citizens.location[i]));
        assert!(w.citizens.destination[i].is_none_or(on));
        assert!(w.citizens.job[i].is_none_or(|b| on(b) && !w.building_closing(b)));
        assert!(on(w.home_building(i as u32)));
        if w.citizens.activity[i] == Activity::Work {
            let at = w.citizens.destination[i].unwrap_or(w.citizens.location[i]);
            assert_eq!(w.citizens.job[i], Some(at), "no jobless work");
        }
    }
    assert!(w.vacancies_hold());
    assert!(w.supply_holds());
    // The inspector (ADR 0022) sees every reference of a resident, and none after departure.
    for c in 0..w.citizens.active.len() as u32 {
        let info = w.citizen_info(c).unwrap();
        let i = c as usize;
        assert_eq!(info.resident, w.citizens.active[i]);
        if info.resident {
            assert!(info.home.is_some() && info.location.is_some());
            assert_eq!(info.job.map(|p| p.building), w.citizens.job[i]);
            assert_eq!(
                info.destination.map(|p| p.building),
                w.citizens.destination[i]
            );
        } else {
            assert!(info.home.is_none() && info.job.is_none() && info.location.is_none());
        }
    }
    for hh in 0..w.households.active.len() {
        let members = &w.households.members[hh];
        let together = members
            .iter()
            .all(|&c| w.citizens.active[c as usize] == w.households.active[hh]);
        assert!(together, "households never split");
    }
}

/// Apartments next to shops, so closures involve several households and many workers.
fn dense() -> World {
    let mut w = World::new(
        Config {
            ignore_demand: true,
            ..Config::default()
        },
        crate::road::RoadNetwork::generate_lattice(4, 3, 120.0),
        crate::rng::Rng::new(1),
    );
    paint(&mut w, 50.0, Zone::ResidentialHigh);
    paint(&mut w, 170.0, Zone::CommercialLow);
    advance(&mut w, 200);
    w
}

#[test]
fn homes_are_registered_in_building_order() {
    let w = dense();
    assert!(w.homes.building.is_sorted());
}

#[test]
fn paused_repaint_keeps_buildings_then_households_leave_one_step_at_a_time() {
    let mut w = dense();
    let flats = in_block(&w, Zone::ResidentialHigh);
    let households = |w: &World| {
        (0..w.homes.building.len())
            .filter(|&h| w.homes.household[h].is_some() && flats.contains(&w.homes.building[h]))
            .count()
    };
    let before = households(&w);
    assert!(before > flats.len(), "several households per building");
    let (tick, version) = (w.totals.tick, w.buildings.version);
    paint(&mut w, 50.0, Zone::CommercialHigh);
    assert_eq!(w.totals.tick, tick);
    assert_eq!(w.buildings.version, version, "repaint keeps geometry");
    assert!(flats.iter().all(|&b| w.building_closing(b)));
    assert_eq!(households(&w), before);
    consistent(&w);
    // Nothing moves before the shortest step; afterwards at most one household per step.
    for _ in 0..(STEP_MIN_S * 10.0) as u32 - 1 {
        w.tick();
    }
    assert_eq!(households(&w), before);
    let mut left = before;
    for _ in 0..(STEP_MIN_S * 10.0) as u32 {
        w.tick();
        let now = households(&w);
        assert!(left - now <= flats.len());
        left = now;
        consistent(&w);
    }
    // Every household goes within one long step each, then the next step removes the building.
    let bound = (before as f32 + 2.0) * STEP_MAX_S;
    for _ in 0..(bound * 10.0) as u32 {
        w.tick();
        consistent(&w);
    }
    assert_eq!(households(&w), 0);
    assert!(flats.iter().all(|&b| !w.buildings.active[b as usize]));
    assert!(w.buildings.version > version);
    // Replacements grow without another command, under new ids on the same lots.
    advance(&mut w, 61);
    let shops = in_block(&w, Zone::CommercialHigh);
    assert!(!shops.is_empty());
    let last = *flats.iter().max().unwrap();
    assert!(shops.iter().all(|&b| b > last));
    for b in shops {
        let lot = w.buildings.lot[b as usize];
        assert!(flats.iter().any(|&f| w.buildings.lot[f as usize] == lot));
    }
    consistent(&w);
}

#[test]
fn closing_households_move_to_a_free_eligible_home_or_leave_town() {
    let mut w = dense();
    isolate(&mut w);
    let flats = in_block(&w, Zone::ResidentialHigh);
    // A detached house with room becomes the only vacancy outside the closing block.
    paint(&mut w, 290.0, Zone::ResidentialLow);
    advance(&mut w, 120);
    for h in 0..w.homes.building.len() {
        if !flats.contains(&w.homes.building[h])
            && let Some(hh) = w.homes.household[h]
        {
            w.relocate_household(hh);
        }
    }
    assert!(
        (0..w.homes.building.len())
            .any(|h| w.homes.household[h].is_none() && !flats.contains(&w.homes.building[h]))
    );
    let closing: Vec<_> = (0..w.households.active.len())
        .filter(|&hh| {
            w.households.active[hh]
                && flats.contains(&w.homes.building[w.households.home[hh] as usize])
        })
        .collect();
    paint(&mut w, 50.0, Zone::None);
    let most = flats
        .iter()
        .map(|&b| w.homes.building.iter().filter(|&&h| h == b).count())
        .max()
        .unwrap();
    advance(&mut w, ((most + 2) as f32 * STEP_MAX_S) as u32);
    assert!(flats.iter().all(|&b| !w.buildings.active[b as usize]));
    consistent(&w);
    let (moved, left): (Vec<usize>, Vec<usize>) =
        closing.iter().partition(|&&hh| w.households.active[hh]);
    assert!(!moved.is_empty(), "some households found a free home");
    assert!(!left.is_empty(), "the rest left town");
    for hh in moved {
        let b = w.homes.building[w.households.home[hh] as usize];
        assert!(!flats.contains(&b) && w.building_accepts_assignments(b));
    }
    advance(&mut w, 61);
    assert!(
        in_block(&w, Zone::ResidentialHigh).is_empty(),
        "unzoned land stays vacant"
    );
}

#[test]
fn workplace_closure_unemploys_workers_at_once_and_sends_them_home() {
    let mut w = dense();
    let shops = in_block(&w, Zone::CommercialLow);
    let workers: Vec<_> = (0..w.citizens.active.len())
        .filter(|&i| w.citizens.job[i].is_some_and(|b| shops.contains(&b)))
        .collect();
    assert!(!workers.is_empty());
    let unemployed = w.stats().unemployed;
    paint(&mut w, 170.0, Zone::IndustrialLow);
    assert_eq!(w.stats().unemployed, unemployed + workers.len() as u32);
    for &i in &workers {
        assert!(w.citizens.job[i].is_none());
        assert_ne!(w.citizens.activity[i], Activity::Work);
    }
    consistent(&w);
    // Empty workplaces go at their first step; visitors and trips recover without arrival.
    advance(&mut w, STEP_MAX_S as u32 + 1);
    assert!(shops.iter().all(|&b| !w.buildings.active[b as usize]));
    consistent(&w);
    advance(&mut w, 61);
    assert!(
        !in_block(&w, Zone::IndustrialLow).is_empty(),
        "last target wins"
    );
    advance(&mut w, 300);
    consistent(&w);
}

#[test]
fn exact_restoration_cancels_closure_without_recalling_anyone() {
    let mut w = dense();
    let shops = in_block(&w, Zone::CommercialLow);
    let flats = in_block(&w, Zone::ResidentialHigh);
    let jobs = w.residents.employed;
    // A density change still mismatches, so it closes like any other repaint.
    paint(&mut w, 170.0, Zone::CommercialHigh);
    paint(&mut w, 50.0, Zone::ResidentialLow);
    assert!(shops.iter().all(|&b| w.building_closing(b)));
    assert!(flats.iter().all(|&b| w.building_closing(b)));
    assert!(shops.iter().all(|&b| !w.building_accepts_assignments(b)));
    paint(&mut w, 170.0, Zone::CommercialLow);
    assert!(shops.iter().all(|&b| !w.building_closing(b)));
    assert!(shops.iter().all(|&b| w.building_accepts_assignments(b)));
    assert!(
        w.residents.employed < jobs,
        "released jobs are not recalled"
    );
    advance(&mut w, 2 * STEP_MAX_S as u32);
    let population = w.residents.population;
    paint(&mut w, 50.0, Zone::ResidentialHigh);
    assert!(flats.iter().any(|&b| w.buildings.active[b as usize]));
    advance(&mut w, 300);
    // Restored buildings stand and admit people again: new hires and new households.
    assert!(shops.iter().all(|&b| w.buildings.active[b as usize]));
    assert!(shops.iter().any(|&b| w.residents.workers[b as usize] > 0));
    assert_eq!(
        w.residents.employed,
        w.residents.workers.iter().sum::<u32>()
    );
    let standing = flats.iter().filter(|&&b| w.buildings.active[b as usize]);
    assert!(standing.clone().all(|&b| !w.building_closing(b)));
    assert!(w.residents.population >= population);
    consistent(&w);
}

#[test]
fn repaint_during_closure_keeps_the_deadline_and_stale_entries_do_nothing() {
    let mut w = dense();
    let flats = in_block(&w, Zone::ResidentialHigh);
    let b = flats[0] as usize;
    paint(&mut w, 50.0, Zone::CommercialLow);
    let due = w.closures.due[b];
    paint(&mut w, 50.0, Zone::IndustrialLow);
    assert_eq!(
        w.closures.due[b], due,
        "a second repaint does not restart closure"
    );
    // Cancel and close again in the same paused tick; duplicate heap entries step once.
    paint(&mut w, 50.0, Zone::ResidentialHigh);
    paint(&mut w, 50.0, Zone::None);
    let due = w.closures.due[b];
    w.closures.heap.push(std::cmp::Reverse((due, b as u32)));
    let homes = |w: &World| {
        (0..w.homes.building.len())
            .filter(|&h| w.homes.building[h] == b as u32 && w.homes.household[h].is_some())
            .count()
    };
    let before = homes(&w);
    while w.totals.tick <= due {
        w.tick();
    }
    assert!(before - homes(&w) <= 1);
    assert!(!w.buildings.active[b] || w.closures.due[b] > w.totals.tick);
}

#[test]
fn relocation_cancels_queued_and_driving_trips_without_arrival() {
    let mut w = dense();
    isolate(&mut w);
    let flat = in_block(&w, Zone::ResidentialHigh)[0];
    let shop = in_block(&w, Zone::CommercialLow)[0];
    let movers: Vec<_> = (0..w.citizens.active.len() as u32)
        .filter(|&c| w.home_building(c) == flat)
        .collect();
    for &c in &movers {
        journey(&mut w, c, shop, Activity::Shop);
    }
    w.tick();
    paint(&mut w, 50.0, Zone::None);
    let completed = w.totals.trips_completed;
    let due = w.closures.due[flat as usize];
    while w.totals.tick <= due + 1 {
        w.tick();
    }
    let gone: Vec<_> = movers
        .iter()
        .filter(|&&c| !w.citizens.active[c as usize] || w.home_building(c) != flat)
        .collect();
    assert!(!gone.is_empty());
    for &&c in &gone {
        let i = c as usize;
        assert!(w.citizens.destination[i].is_none() || w.citizens.active[i]);
        if let Some(a) = w.citizens.agent[i] {
            assert!(!w.path_queue.contains(&a));
            assert!(!w.entering.contains(&a));
            if !w.citizens.active[i] {
                assert_eq!(w.agents.state[a as usize], AgentState::Dormant);
            }
        }
    }
    assert!(w.totals.trips_completed >= completed);
    consistent(&w);
}

#[test]
fn road_demolition_during_closure_stays_immediate_and_later_events_are_harmless() {
    let mut w = dense();
    paint(&mut w, 50.0, Zone::CommercialLow);
    let roads: Vec<_> = w.roads.active_roads().collect();
    for r in roads {
        let (x, y) = w
            .roads
            .lane_point(r * 2, w.roads.lane_len[r as usize * 2] / 2.0, 0.0);
        w.apply(Command::DeleteRoad { x, y });
    }
    assert_eq!(w.buildings.active_buildings().count(), 0);
    assert_eq!(w.residents.population, 0);
    advance(&mut w, 2 * STEP_MAX_S as u32);
    consistent(&w);
}

#[test]
fn closure_is_deterministic_and_hashed() {
    let mut a = dense();
    let mut b = dense();
    for w in [&mut a, &mut b] {
        paint(w, 50.0, Zone::CommercialHigh);
        paint(w, 170.0, Zone::None);
    }
    let flat = in_block(&a, Zone::ResidentialHigh)[0] as usize;
    let hash = a.state_hash();
    a.closures.due[flat] += 1;
    assert_ne!(a.state_hash(), hash, "closure deadlines are state");
    a.closures.due[flat] -= 1;
    for _ in 0..600 {
        assert_eq!(a.state_hash(), b.state_hash());
        a.tick();
        b.tick();
    }
    assert_eq!(a.state_hash(), b.state_hash());
}

/// The relocation index must pick exactly what a scan of every home would.
fn same_choice_as_linear_scan(w: &World) {
    assert!(w.vacancies_hold());
    let linear = |old: usize, size: usize| {
        (0..w.homes.building.len()).find(|&h| {
            h != old
                && w.homes.household[h].is_none()
                && w.homes.capacity[h] as usize >= size
                && w.building_accepts_assignments(w.homes.building[h])
        })
    };
    for size in 1..=7 {
        assert_eq!(w.vacant_home(usize::MAX, size), linear(usize::MAX, size));
    }
    for hh in (0..w.households.active.len()).filter(|&hh| w.households.active[hh]) {
        let (old, size) = (
            w.households.home[hh] as usize,
            w.households.members[hh].len(),
        );
        assert_eq!(w.vacant_home(old, size), linear(old, size));
    }
}

#[test]
fn vacancy_index_matches_a_linear_scan_through_every_lifecycle_path() {
    let mut w = dense();
    same_choice_as_linear_scan(&w);
    // Departures and moves open vacancies in several buildings.
    for hh in (0..w.households.active.len() as u32).step_by(5) {
        w.relocate_household(hh);
        same_choice_as_linear_scan(&w);
    }
    // Closure, a paused restore, and closure again.
    paint(&mut w, 50.0, Zone::CommercialLow);
    same_choice_as_linear_scan(&w);
    advance(&mut w, 10);
    same_choice_as_linear_scan(&w);
    paint(&mut w, 50.0, Zone::ResidentialHigh);
    same_choice_as_linear_scan(&w);
    advance(&mut w, 30);
    same_choice_as_linear_scan(&w);
    // Losing and regaining road access, and the registration of new homes.
    let r = w.lots.road[w.buildings.lot[in_block(&w, Zone::ResidentialHigh)[0] as usize] as usize];
    let (x, y) = w
        .roads
        .lane_point(r * 2, w.roads.lane_len[r as usize * 2] / 2.0, 0.0);
    let (ax, ay) = (
        w.roads.node_x[w.roads.lane_from[r as usize * 2] as usize],
        w.roads.node_y[w.roads.lane_from[r as usize * 2] as usize],
    );
    let (bx, by) = (
        w.roads.node_x[w.roads.lane_to[r as usize * 2] as usize],
        w.roads.node_y[w.roads.lane_to[r as usize * 2] as usize],
    );
    assert!(w.apply(Command::DeleteRoad { x, y }));
    same_choice_as_linear_scan(&w);
    advance(&mut w, 5);
    let class = crate::road::RoadClass::Street;
    assert!(w.apply(Command::BuildRoad {
        ax,
        ay,
        bx,
        by,
        class
    }));
    same_choice_as_linear_scan(&w);
    paint(&mut w, 290.0, Zone::ResidentialHigh);
    for _ in 0..20 {
        advance(&mut w, 10);
        same_choice_as_linear_scan(&w);
    }
    paint(&mut w, 290.0, Zone::None);
    paint(&mut w, 50.0, Zone::None);
    for _ in 0..30 {
        advance(&mut w, 10);
        same_choice_as_linear_scan(&w);
    }
}

#[test]
fn a_building_repainted_on_the_tick_it_grows_closes_and_admits_nobody() {
    let mut w = dense();
    paint(&mut w, 290.0, Zone::ResidentialHigh);
    let before = w.buildings.active.len();
    let grown = |w: &World| {
        (before..w.buildings.active.len())
            .find(|&b| w.lots.front_x[w.buildings.lot[b] as usize] > 240.0)
    };
    for _ in 0..1200 {
        if grown(&w).is_some() {
            break;
        }
        w.tick();
    }
    let fresh = grown(&w).expect("the painted block grows") as u32;
    assert!(
        w.residents.registered > fresh as usize,
        "growth registers its homes and jobs at once (ADR 0025)"
    );
    paint(&mut w, 290.0, Zone::CommercialLow);
    assert!(w.building_closing(fresh));
    assert!(!w.building_accepts_assignments(fresh));
    consistent(&w);
    let homes: Vec<_> = (0..w.homes.building.len())
        .filter(|&h| w.homes.building[h] == fresh)
        .collect();
    assert!(!homes.is_empty(), "edits register new buildings at once");
    // Short of the first closure step, which removes an empty building.
    advance(&mut w, 2);
    assert!(homes.iter().all(|&h| w.homes.household[h].is_none()));
    paint(&mut w, 290.0, Zone::ResidentialHigh);
    assert!(w.building_accepts_assignments(fresh));
    consistent(&w);
}
