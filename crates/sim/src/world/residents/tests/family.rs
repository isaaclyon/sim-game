//! Children, births, growing up and moving out (ADR 0054).
use super::*;
use crate::world::residents::family::{ADULT_YEARS, CHILDHOOD_DAYS, MOVE_OUT_DAYS};
use std::cmp::Reverse;

/// Low and dense homes, shops and industry, settled.
fn families() -> World {
    let mut w = World::new(
        Config {
            ignore_demand: true,
            ..Config::default()
        },
        crate::road::RoadNetwork::generate_lattice(5, 3, 120.0),
        crate::rng::Rng::new(1),
    );
    for (x, zone) in [
        (50.0, Zone::ResidentialLow),
        (170.0, Zone::CommercialLow),
        (290.0, Zone::IndustrialLow),
        (410.0, Zone::ResidentialHigh),
    ] {
        assert!(w.apply(Command::FillBlock { x, y: 50.0, zone }));
    }
    advance(&mut w, 120);
    w
}

fn children(w: &World) -> Vec<u32> {
    (0..w.citizens.active.len() as u32)
        .filter(|&c| w.citizens.active[c as usize] && w.citizens.child[c as usize])
        .collect()
}

/// A household whose home has room for one more, after making room if needed.
fn roomy_household(w: &mut World) -> u32 {
    (0..w.households.active.len() as u32)
        .find(|&hh| {
            let h = w.households.home[hh as usize] as usize;
            w.households.active[hh as usize]
                && w.households.members[hh as usize].len() < w.homes.capacity[h] as usize
        })
        .expect("a household with room")
}

/// A child born into a household with room, who comes of age at the next tick.
fn grown_child(w: &mut World) -> u32 {
    let hh = roomy_household(w);
    let before = w.households.members[hh as usize].len();
    while w.households.members[hh as usize].len() == before {
        w.check_birth(hh);
    }
    let c = *w.households.members[hh as usize].last().unwrap();
    let now = w.totals.tick;
    w.citizens.born[c as usize] = now as i64 + 1 - w.days_to_ticks(CHILDHOOD_DAYS) as i64;
    w.residents.grown.push(Reverse((now + 1, c)));
    advance(w, 1);
    assert!(!w.citizens.child[c as usize]);
    c
}

#[test]
fn arriving_families_bring_children_who_stay_home() {
    let w = families();
    let kids = children(&w);
    assert!(!kids.is_empty());
    assert_eq!(w.stats().children as usize, kids.len());
    for hh in 0..w.households.active.len() {
        let first = w.households.members[hh][0] as usize;
        assert!(!w.citizens.child[first], "the first member is an adult");
        let info = w.citizen_info(first as u32).unwrap();
        assert!(!info.grew_up && !info.wants_home, "arrived grown up");
    }
    for &c in &kids {
        let i = c as usize;
        assert_eq!(w.citizens.job[i], None);
        assert!(!w.citizens.owns_car[i]);
        assert_eq!(w.citizens.activity[i], Activity::Home);
        assert_eq!(w.citizens.location[i], w.home_building(c));
        assert_eq!(
            (w.citizens.due[i], w.citizens.job_due[i]),
            (u64::MAX, u64::MAX)
        );
        assert!(w.age_years(c) < ADULT_YEARS);
        let info = w.citizen_info(c).unwrap();
        assert!(info.child && info.age < ADULT_YEARS);
    }
    let s = w.stats();
    let adults = s.population - s.children;
    assert_eq!(s.unemployed, adults - w.residents.employed);
}

#[test]
fn births_need_an_adult_and_room_at_home() {
    let mut w = families();
    let hh = roomy_household(&mut w);
    let h = w.households.home[hh as usize] as usize;
    let capacity = w.homes.capacity[h] as usize;
    let population = w.residents.population;
    for _ in 0..500 {
        w.check_birth(hh);
        assert!(w.households.members[hh as usize].len() <= capacity);
    }
    let members = w.households.members[hh as usize].clone();
    assert_eq!(
        members.len(),
        capacity,
        "a home with room fills with babies"
    );
    let born = w.residents.population - population;
    assert!(born > 0);
    let baby = *members.last().unwrap() as usize;
    assert!(w.citizens.child[baby] && !w.citizens.owns_car[baby]);
    assert_eq!(w.citizens.born[baby], w.totals.tick as i64);
    // Children alone never have babies.
    let other = roomy_household(&mut w);
    for &c in &w.households.members[other as usize].clone() {
        w.citizens.child[c as usize] = true;
    }
    let size = w.households.members[other as usize].len();
    for _ in 0..500 {
        w.check_birth(other);
    }
    assert_eq!(w.households.members[other as usize].len(), size);
}

#[test]
fn birth_checks_run_from_the_event_heap() {
    let mut w = families();
    let households = w.households.active.iter().filter(|&&a| a).count();
    assert_eq!(w.residents.births.len(), households);
    let children = w.residents.children;
    advance(&mut w, (4.0 * 4800.0) as u32);
    assert!(w.residents.children > children, "families had babies");
    // Exactly one pending check per active household; stale ones drop when they come up.
    let mut checks: Vec<u32> = w.residents.births.iter().map(|e| e.0.1).collect();
    checks.retain(|&hh| w.households.active[hh as usize]);
    checks.sort_unstable();
    checks.dedup();
    let active = w.households.active.iter().filter(|&&a| a).count();
    assert_eq!(checks.len(), active);
}

#[test]
fn a_child_who_comes_of_age_looks_for_work_and_a_home() {
    let mut w = families();
    let children = w.residents.children;
    let c = grown_child(&mut w);
    let i = c as usize;
    assert_eq!(w.residents.children, children);
    assert_eq!(w.age_years(c), ADULT_YEARS);
    let deadline = w.citizens.move_by[i];
    let came_of_age = w.citizens.born[i] + w.days_to_ticks(CHILDHOOD_DAYS) as i64;
    assert_eq!(
        deadline,
        came_of_age as u64 + w.days_to_ticks(MOVE_OUT_DAYS)
    );
    assert!(w.residents.movers.contains(&(deadline, c)));
    let info = w.citizen_info(c).unwrap();
    assert!(!info.child && info.grew_up && info.wants_home);
    assert_ne!(w.citizens.job_due[i], u64::MAX, "a job search is due");
    // Free a job near home, in case every job is taken.
    let home = w.home_building(c);
    let worker = (0..w.citizens.active.len() as u32)
        .filter(|&o| w.citizens.job[o as usize].is_some())
        .min_by(|&a, &b| {
            let d = |o: u32| w.front_distance(home, w.citizens.job[o as usize].unwrap());
            d(a).total_cmp(&d(b))
        })
        .unwrap();
    w.release_job(worker);
    advance(&mut w, 5);
    assert!(w.citizens.job[i].is_some(), "takes a job");
    assert!(
        !w.citizens.owns_car[i],
        "no car until moving into their own home"
    );
}

#[test]
fn grown_children_take_vacant_homes_before_arrivals() {
    let mut w = families();
    let c = grown_child(&mut w);
    let parents = w.citizens.household[c as usize];
    let leaving = (0..w.households.active.len() as u32)
        .find(|&hh| hh != parents && w.households.active[hh as usize])
        .unwrap();
    let home = w.households.home[leaving as usize];
    let gone = w.households.members[leaving as usize].len() as u32;
    let population = w.residents.population;
    let households = w.households.active.len();
    w.household_leaves_city(leaving);
    advance(&mut w, 60);
    let own = w.citizens.household[c as usize];
    assert_ne!(own, parents);
    assert_eq!(own as usize, households, "a new household");
    assert_eq!(w.households.home[own as usize], home);
    assert_eq!(w.households.members[own as usize], vec![c]);
    assert_eq!(w.homes.household[home as usize], Some(own));
    assert!(!w.households.members[parents as usize].contains(&c));
    assert!(w.residents.movers.iter().all(|&(_, m)| m != c));
    assert_eq!(w.citizens.move_by[c as usize], u64::MAX);
    let info = w.citizen_info(c).unwrap();
    assert!(info.grew_up && !info.wants_home, "has a home of their own");
    assert_eq!(
        w.citizens.family[c as usize], parents,
        "keeps the family name"
    );
    // Only the leaving household is gone; births in the meantime may add to it.
    assert!(w.citizens.active[c as usize]);
    assert!(w.residents.population >= population - gone);
    assert!(w.vacancies_hold());
}

#[test]
fn grown_children_with_no_home_in_time_leave_the_city() {
    let mut w = families();
    let c = grown_child(&mut w);
    let i = c as usize;
    advance(&mut w, 5);
    let parents = w.citizens.household[i];
    let (population, employed) = (w.residents.population, w.residents.employed);
    let had_job = w.citizens.job[i].is_some() as u32;
    let deadline = w.citizens.move_by[i];
    w.residents.movers.remove(&(deadline, c));
    w.citizens.move_by[i] = w.totals.tick + 1;
    w.residents.movers.insert((w.totals.tick + 1, c));
    advance(&mut w, 1);
    assert!(!w.citizens.active[i]);
    assert_eq!(w.residents.population, population - 1);
    assert_eq!(w.residents.employed, employed - had_job);
    assert!(w.households.active[parents as usize]);
    assert!(!w.households.members[parents as usize].contains(&c));
    assert!(w.residents.movers.is_empty());
}

#[test]
fn families_save_and_hash_their_state() {
    let mut w = families();
    let c = grown_child(&mut w);
    assert!(
        w.residents
            .movers
            .contains(&(w.citizens.move_by[c as usize], c))
    );
    let mut loaded = World::load(&w.save().unwrap()).unwrap();
    assert_eq!(loaded.state_hash(), w.state_hash());
    advance(&mut w, 300);
    advance(&mut loaded, 300);
    assert_eq!(loaded.state_hash(), w.state_hash());
    // Birth ticks, deadlines and the birth stream are all hashed.
    let hash = w.state_hash();
    w.citizens.born[0] -= 1;
    assert_ne!(w.state_hash(), hash);
    w.citizens.born[0] += 1;
    w.residents.birth_rng.next_u64();
    assert_ne!(w.state_hash(), hash);
}

#[test]
fn inconsistent_family_saves_are_refused() {
    let mut w = families();
    w.residents.children += 1;
    assert!(World::load(&w.save().unwrap()).is_err());
    let mut w = families();
    let c = grown_child(&mut w);
    w.citizens.move_by[c as usize] += 1;
    assert!(World::load(&w.save().unwrap()).is_err());
}

#[test]
fn families_grow_the_same_way_every_run() {
    let (mut a, mut b) = (families(), families());
    advance(&mut a, 2 * 4800);
    advance(&mut b, 2 * 4800);
    assert_eq!(a.state_hash(), b.state_hash());
}
