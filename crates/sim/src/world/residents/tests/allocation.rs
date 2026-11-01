use super::*;

#[test]
fn apartments_have_independent_homes_and_halls_have_only_one_job_floor() {
    let mut w = World::new(
        Config::default(),
        crate::road::RoadNetwork::generate_lattice(4, 3, 120.0),
        crate::rng::Rng::new(1),
    );
    for (x, zone) in [(50.0, Zone::ResidentialHigh), (170.0, Zone::IndustrialHigh)] {
        w.apply(Command::FillBlock { x, y: 50.0, zone });
    }
    advance(&mut w, 120);
    for b in w.buildings.active_buildings() {
        let i = b as usize;
        let area = crate::poly::area(&crate::poly::ring(w.buildings.footprint(b).iter().copied()));
        if w.buildings.zone[i] == Zone::ResidentialHigh {
            let homes: Vec<_> = w
                .homes
                .building
                .iter()
                .enumerate()
                .filter(|&(_, &v)| v == b)
                .map(|(h, _)| h)
                .collect();
            assert!(homes.len() > 1);
            let occupants: std::collections::BTreeSet<_> = homes
                .iter()
                .map(|&h| w.homes.household[h].unwrap())
                .collect();
            assert_eq!(occupants.len(), homes.len());
            assert!(
                homes
                    .iter()
                    .all(|&h| (2..=7).contains(&w.homes.capacity[h]))
            );
        } else {
            assert_eq!(w.residents.jobs[i], (area / 60.0).floor().max(1.0) as u32);
        }
    }
}

#[test]
fn reachable_vacancy_is_not_starved_by_many_nearby_disconnected_employers() {
    let mut w = town();
    isolate(&mut w);
    let home_node = w.building_node(w.home_building(0)).unwrap();
    let near_a = w.roads.add_node(10.0, 10.0);
    let near_b = w.roads.add_node(20.0, 10.0);
    w.roads
        .add_road(near_a, near_b, crate::road::RoadClass::Street);
    w.sync_roads();
    let near_road = w.roads.out_lanes(near_a)[0] / 2;
    let far_a = w.roads.add_node(10000.0, 0.0);
    let far_b = w.roads.add_node(10010.0, 0.0);
    w.roads
        .add_road(home_node, far_a, crate::road::RoadClass::Street);
    w.roads
        .add_road(far_a, far_b, crate::road::RoadClass::Street);
    w.sync_roads();
    let far_road = w.roads.out_lanes(far_b)[0] / 2;
    let employers: Vec<_> = w
        .buildings
        .active_buildings()
        .filter(|&b| w.residents.jobs[b as usize] > 0)
        .collect();
    assert!(employers.len() > 8);
    for &b in &employers {
        let l = w.buildings.lot[b as usize] as usize;
        // Route-access fixture: nearby workplaces are isolated, one distant vacancy reachable.
        w.lots.road[l] = near_road;
        w.lots.front_x[l] = 10.0;
        w.lots.front_y[l] = 10.0;
        w.residents.workers[b as usize] = 0;
    }
    let far = employers[0];
    let l = w.buildings.lot[far as usize] as usize;
    w.lots.road[l] = far_road;
    w.lots.front_x[l] = 10000.0;
    w.lots.front_y[l] = 0.0;
    assert_eq!(w.choose_destination(0, true), Some(far));
}

#[test]
fn unreachable_jobs_and_shops_are_not_assigned_even_if_nearby() {
    let mut w = town();
    isolate(&mut w);
    let home = w.home_building(0);
    let from = w.building_node(home).unwrap();
    // Keep the home's access road, but isolate its two ends from the rest of the town.
    let access = w.lots.road[w.buildings.lot[home as usize] as usize];
    let ends = [
        w.roads.lane_from[access as usize * 2],
        w.roads.lane_to[access as usize * 2],
    ];
    let cut: Vec<_> = w
        .roads
        .active_roads()
        .filter(|&r| {
            r != access
                && (ends.contains(&w.roads.lane_from[r as usize * 2])
                    || ends.contains(&w.roads.lane_to[r as usize * 2]))
        })
        .collect();
    for &r in &cut {
        w.roads.remove_road(r);
    }
    w.sync_roads();
    assert_eq!(w.building_node(home), Some(from));
    for b in 0..w.residents.registered {
        if w.building_node(b as u32).is_some_and(|n| ends.contains(&n)) {
            w.residents.jobs[b] = w.residents.workers[b];
        }
    }
    assert!(w.choose_destination(0, true).is_none());
    let target = w
        .buildings
        .active_buildings()
        .find(|&b| {
            w.buildings.zone[b as usize] == Zone::IndustrialLow && w.building_node(b).is_some()
        })
        .unwrap();
    let completed = w.totals.trips_completed;
    journey(&mut w, 0, target, Activity::Work);
    advance(&mut w, 61);
    assert!(w.totals.paths_failed > 0);
    assert_eq!(w.totals.trips_completed, completed);
    assert_eq!(w.citizens.location[0], home);
    assert_eq!(w.citizens.destination[0], Some(target));
    // New roads get new IDs; cache invalidation must see additions as well as tombstones.
    for r in cut {
        let (a, b) = (
            w.roads.lane_from[r as usize * 2],
            w.roads.lane_to[r as usize * 2],
        );
        w.roads.add_road(a, b, crate::road::RoadClass::Street);
    }
    w.sync_roads();
    assert!(w.choose_destination(0, true).is_some());
    advance(&mut w, 300);
    assert_eq!(
        w.citizens.location[0], target,
        "actual routing resumes after reconnection"
    );
}
