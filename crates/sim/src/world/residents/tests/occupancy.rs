use super::*;
use crate::citizen::household_size;
use crate::rng::Rng;

#[test]
fn household_distribution_is_truncated_and_small_sizes_remain_common() {
    let mut rng = Rng::new(1);
    let mut counts = [0; 7];
    for _ in 0..100_000 {
        counts[household_size(7, &mut rng) as usize - 1] += 1;
    }
    for (actual, weight) in counts.into_iter().zip([25, 35, 18, 12, 6, 3, 1]) {
        assert!((actual - weight * 1000_i32).abs() < 600);
    }
    let mut singles = 0;
    for _ in 0..10_000 {
        let size = household_size(2, &mut rng);
        assert!(size <= 2);
        singles += usize::from(size == 1);
    }
    assert!((3900..4400).contains(&singles));
}

#[test]
fn jobs_are_limited_and_industry_is_not_a_shop() {
    let mut w = town();
    for b in 0..w.residents.registered {
        assert!(w.residents.workers[b] <= w.residents.jobs[b]);
        let actual = w
            .citizens
            .job
            .iter()
            .filter(|&&job| job == Some(b as u32))
            .count();
        assert_eq!(actual, w.residents.workers[b] as usize);
    }
    for _ in 0..100 {
        let b = w.choose_destination(0, false).unwrap();
        assert_eq!(w.buildings.zone[b as usize], Zone::CommercialLow);
    }
}

#[test]
fn relocation_reserves_one_home_for_all_members_and_departure_cleans_ownership() {
    let mut w = town();
    isolate(&mut w);
    w.totals.tick = 36_000; // Relocation at midnight must not trigger a commute.
    let hh = w.citizens.household[0];
    let old = w.households.home[hh as usize];
    let members = w.households.members[hh as usize].clone();
    // With every home occupied, this household leaves and releases a suitable vacancy.
    let donor = (0..w.homes.building.len())
        .find(|&h| h != old as usize && w.homes.capacity[h] as usize >= members.len())
        .unwrap();
    let donor_household = w.homes.household[donor].unwrap();
    w.relocate_household(donor_household);
    assert!(!w.households.active[donor_household as usize]);
    assert!(w.homes.household[donor].is_none());
    w.relocate_household(hh);
    assert_eq!(w.households.home[hh as usize], donor as u32);
    assert_eq!(w.homes.household[donor], Some(hh));
    for &c in &members {
        assert!(w.citizens.active[c as usize]);
        assert_eq!(w.home_building(c), w.homes.building[donor]);
        assert_eq!(w.citizens.location[c as usize], w.homes.building[donor]);
        if w.citizens.child[c as usize] {
            assert_eq!(w.citizens.due[c as usize], u64::MAX, "children stay home");
            continue;
        }
        let at = crate::clock::clock_at(w.citizens.due[c as usize], w.cfg.dt);
        assert_eq!(at.day, 2);
        assert!((7 * 3600..=16 * 3600).contains(&at.second_of_day));
    }
    for h in 0..w.homes.building.len() {
        w.buildings.active[w.homes.building[h] as usize] = false;
    }
    let before = w.residents.population;
    w.relocate_household(hh);
    assert_eq!(w.residents.population, before - members.len() as u32);
    assert!(w.homes.household[donor].is_none());
    for &c in &members {
        assert!(!w.citizens.active[c as usize]);
        assert!(w.citizens.job[c as usize].is_none());
        assert!(w.citizens.destination[c as usize].is_none());
    }
}

#[test]
fn removed_destination_and_removed_activity_location_never_count_as_arrival() {
    let mut w = town();
    isolate(&mut w);
    let target = distant(&w, 0);
    let home = w.home_building(0);
    w.totals.tick = 36_000;
    journey(&mut w, 0, target, Activity::Work);
    w.tick();
    let before = w.totals.trips_completed;
    w.buildings.active[target as usize] = false;
    w.reconcile_residents();
    assert_eq!(w.totals.trips_completed, before);
    assert_eq!(w.citizens.location[0], home);
    assert!(w.citizens.destination[0].is_none());
    assert!(!w.citizens.job.contains(&Some(target)));
    w.citizens.location[0] = target;
    w.reconcile_residents();
    assert_eq!(w.citizens.location[0], home);
    let at = crate::clock::clock_at(w.citizens.due[0], w.cfg.dt);
    assert_eq!(at.day, 2);
    if w.citizens.job[0].is_some() {
        assert!((at.second_of_day as i64 - w.citizens.departure[0] as i64).abs() <= 1);
    } else {
        assert!((10 * 3600..=16 * 3600).contains(&at.second_of_day));
    }
}
