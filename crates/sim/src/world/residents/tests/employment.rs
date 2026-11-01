use super::*;

/// Recount from citizen state and check the authoritative counters agree exactly.
fn check(w: &World) -> (u32, u32) {
    let s = w.stats();
    let active = (0..w.citizens.active.len()).filter(|&i| w.citizens.active[i]);
    let employed = active
        .clone()
        .filter(|&i| w.citizens.job[i].is_some())
        .count() as u32;
    assert_eq!(s.population, active.count() as u32);
    assert!(employed <= s.population);
    assert_eq!(employed, w.residents.workers.iter().sum::<u32>());
    assert_eq!(s.unemployed, s.population - employed);
    let percent = if s.population == 0 {
        0.0
    } else {
        s.unemployed as f32 * 100.0 / s.population as f32
    };
    assert_eq!(s.unemployment_percent, percent);
    (s.population, s.unemployed)
}

fn housing_only() -> World {
    let mut w = World::new(
        Config::default(),
        crate::road::RoadNetwork::generate_lattice(4, 3, 120.0),
        crate::rng::Rng::new(1),
    );
    assert!(w.apply(Command::FillBlock {
        x: 50.0,
        y: 50.0,
        zone: Zone::ResidentialHigh,
    }));
    w
}

#[test]
fn empty_town_has_no_unemployment_and_a_zero_rate() {
    let mut w = housing_only();
    assert_eq!(check(&w), (0, 0));
    assert_eq!(w.stats().unemployment_percent, 0.0);
    advance(&mut w, 2);
    assert_eq!(check(&w), (0, 0));
}

#[test]
fn residents_without_jobs_are_unemployed_until_assigned() {
    let mut w = housing_only();
    advance(&mut w, 120);
    let (population, unemployed) = check(&w);
    assert!(population > 0);
    assert_eq!(unemployed, population);
    assert_eq!(w.stats().unemployment_percent, 100.0);
    // Apartments house more residents than one block of shops employs, leaving a mixture.
    assert!(w.apply(Command::FillBlock {
        x: 170.0,
        y: 50.0,
        zone: Zone::CommercialLow,
    }));
    advance(&mut w, 400);
    let (population, unemployed) = check(&w);
    assert!(unemployed < population, "assignment employs residents");
    assert!(
        unemployed > 0,
        "limited jobs leave some residents unemployed"
    );
    assert!(w.citizens.job.iter().any(Option::is_some));
}

#[test]
fn workplace_demolition_counts_job_loss() {
    let mut w = town();
    let (population, unemployed) = check(&w);
    let c = w.citizens.job.iter().position(Option::is_some).unwrap();
    let job = w.citizens.job[c].unwrap();
    let lost = w.residents.workers[job as usize];
    w.buildings.active[job as usize] = false;
    w.reconcile_residents();
    assert_eq!(check(&w), (population, unemployed + lost));
}

#[test]
fn departure_removes_employed_and_unemployed_residents_from_both_counts() {
    let mut w = town();
    isolate(&mut w);
    // Make one household fully unemployed so both kinds of departure are covered.
    let households: Vec<_> = (0..w.households.active.len() as u32).collect();
    let jobless = households[0];
    for c in w.households.members[jobless as usize].clone() {
        w.release_job(c);
    }
    let employed = *households[1..]
        .iter()
        .find(|&&h| {
            w.households.members[h as usize]
                .iter()
                .any(|&c| w.citizens.job[c as usize].is_some())
        })
        .unwrap();
    // Remove every home so relocation is impossible and households leave town.
    for h in 0..w.homes.building.len() {
        w.buildings.active[w.homes.building[h] as usize] = false;
    }
    for hh in [jobless, employed] {
        let (population, unemployed) = check(&w);
        let members = &w.households.members[hh as usize];
        let size = members.len() as u32;
        let idle = members
            .iter()
            .filter(|&&c| w.citizens.job[c as usize].is_none())
            .count() as u32;
        w.relocate_household(hh);
        assert_eq!(check(&w), (population - size, unemployed - idle));
    }
}

#[test]
fn relocation_keeps_jobs() {
    let mut w = town();
    isolate(&mut w);
    let hh = (0..w.households.active.len())
        .find(|&h| {
            w.households.members[h]
                .iter()
                .any(|&c| w.citizens.job[c as usize].is_some())
        })
        .unwrap();
    let size = w.households.members[hh].len();
    let donor = (0..w.homes.building.len())
        .find(|&h| h != w.households.home[hh] as usize && w.homes.capacity[h] as usize >= size)
        .unwrap();
    w.relocate_household(w.homes.household[donor].unwrap());
    let before = check(&w);
    w.relocate_household(hh as u32);
    assert!(w.households.active[hh]);
    assert_eq!(check(&w), before);
}

#[test]
fn disconnected_workplace_still_employs_its_workers() {
    let mut w = town();
    let c = w.citizens.job.iter().position(Option::is_some).unwrap();
    let job = w.citizens.job[c].unwrap();
    let before = check(&w);
    let a = w.roads.add_node(10_000.0, 0.0);
    let b = w.roads.add_node(10_010.0, 0.0);
    w.roads.add_road(a, b, crate::road::RoadClass::Street);
    w.sync_roads();
    let l = w.buildings.lot[job as usize] as usize;
    w.lots.road[l] = w.roads.out_lanes(a)[0] / 2;
    w.lots.front_x[l] = 10_000.0;
    w.lots.front_y[l] = 0.0;
    w.reconcile_residents();
    advance(&mut w, 300);
    assert_eq!(w.citizens.job[c], Some(job));
    let after = check(&w);
    assert_eq!(after.0, before.0);
    assert!(after.1 <= before.1, "nobody loses a disconnected job");
}

#[test]
fn paused_road_demolition_updates_unemployment_without_a_tick() {
    let mut w = town();
    let (population, unemployed) = check(&w);
    assert!(population > unemployed);
    let tick = w.totals.tick;
    let roads: Vec<_> = w.roads.active_roads().collect();
    for r in roads {
        let (x, y) = w
            .roads
            .lane_point(r * 2, w.roads.lane_len[r as usize * 2] / 2.0, 0.0);
        w.apply(Command::DeleteRoad { x, y });
        check(&w);
    }
    assert_eq!(w.totals.tick, tick);
    assert_eq!(check(&w), (0, 0));
    assert_eq!(w.stats().unemployment_percent, 0.0);
}
