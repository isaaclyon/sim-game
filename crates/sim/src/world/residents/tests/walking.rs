//! Walking trips and household cars (ADR 0026).
use super::*;
use crate::agents::AgentState;
use crate::road::{RoadClass, RoadNetwork};
use crate::world::SNAPSHOT_STRIDE;

fn agent(w: &World, c: u32) -> usize {
    w.citizens.agent[c as usize].unwrap() as usize
}
fn arrive(w: &mut World, c: u32) {
    for _ in 0..20_000 {
        if w.citizens.destination[c as usize].is_none() {
            return;
        }
        w.tick();
    }
    panic!("citizen {c} never arrived");
}

#[test]
fn short_trips_walk_and_long_trips_drive() {
    let mut w = town();
    isolate(&mut w);
    let target = distant(&w, 0);
    w.citizens.owns_car[0] = true;
    w.citizens.walk_limit[0] = 1e6;
    journey(&mut w, 0, target, Activity::Work);
    assert!(!w.citizens.car_out[0]);
    assert!(w.citizen_info(0).unwrap().on_foot);
    w.tick();
    assert_eq!(w.agents.state[agent(&w, 0)], AgentState::Walking);
    assert_eq!(w.stats().walking, 1);

    let mut w = town();
    isolate(&mut w);
    driver(&mut w, 0);
    journey(&mut w, 0, target, Activity::Work);
    assert!(w.citizens.car_out[0]);
    assert!(!w.citizen_info(0).unwrap().on_foot);
    w.tick();
    assert_ne!(w.agents.state[agent(&w, 0)], AgentState::Walking);
}

#[test]
fn walk_follows_connected_sidewalks_and_arrives() {
    let mut w = town();
    isolate(&mut w);
    walker(&mut w, 0);
    let target = distant(&w, 0);
    journey(&mut w, 0, target, Activity::Work);
    w.tick();
    let a = agent(&w, 0);
    let path = w.agents.path[a].clone();
    for pair in path.windows(2) {
        assert_eq!(
            w.roads.lane_to[pair[0] as usize],
            w.roads.lane_from[pair[1] as usize]
        );
    }
    let last = *path.last().unwrap();
    assert!((0.0..=w.roads.lane_len[last as usize]).contains(&w.agents.walk_end[a]));
    let (completed, walks) = (w.totals.trips_completed, w.totals.walks_completed);
    arrive(&mut w, 0);
    assert_eq!(w.citizens.location[0], target);
    assert_eq!(w.agents.state[a], AgentState::Dormant);
    assert_eq!(w.totals.trips_completed, completed + 1);
    assert_eq!(w.totals.walks_completed, walks + 1);
}

#[test]
fn car_stays_with_its_owner_until_home() {
    let mut w = town();
    isolate(&mut w);
    driver(&mut w, 0);
    let work = distant(&w, 0);
    journey(&mut w, 0, work, Activity::Work);
    arrive(&mut w, 0);
    assert!(w.citizens.car_out[0]);
    // Away from home the car decides, not the distance: a short hop still drives.
    w.citizens.walk_limit[0] = 1e6;
    assert_eq!(w.walk_reach(0, false), None);
    let home = w.home_building(0);
    journey(&mut w, 0, home, Activity::Home);
    w.tick();
    assert_ne!(w.agents.state[agent(&w, 0)], AgentState::Walking);
    arrive(&mut w, 0);
    assert!(!w.citizens.car_out[0]);
}

#[test]
fn adults_without_a_car_only_take_jobs_and_shops_in_reach() {
    let mut w = town();
    isolate(&mut w);
    w.release_job(0);
    w.citizens.owns_car[0] = false;
    w.citizens.walk_limit[0] = 1.0;
    assert_eq!(w.choose_destination(0, true), None);
    assert_eq!(w.choose_destination(0, false), None);
    w.citizens.walk_limit[0] = 1e6;
    assert!(w.choose_destination(0, true).is_some());
    // An owner whose car stayed home shops on foot too.
    w.citizens.owns_car[0] = true;
    w.citizens.walk_limit[0] = 1.0;
    w.citizens.location[0] = (0..w.buildings.zone.len() as u32)
        .find(|&b| w.buildings.zone[b as usize] == Zone::IndustrialLow)
        .unwrap();
    assert_eq!(w.choose_destination(0, false), None);
    w.citizens.car_out[0] = true;
    assert!(w.choose_destination(0, false).is_some());
}

#[test]
fn a_town_walks_and_drives_and_every_car_is_in_one_place() {
    // The test town is small enough to walk across; every other owner drives everywhere.
    let mixed = || {
        let mut w = town();
        for c in (0..w.citizens.active.len()).step_by(2) {
            if w.citizens.owns_car[c] {
                w.citizens.walk_limit[c] = 10.0;
            }
        }
        w
    };
    let mut w = mixed();
    let mut w2 = mixed();
    for _ in 0..60 {
        advance(&mut w, 60);
        for i in 0..w.citizens.active.len() {
            let c = &w.citizens;
            assert!(!c.car_out[i] || c.owns_car[i], "only owners take a car out");
            if c.location[i] == w.home_building(i as u32) && c.destination[i].is_none() {
                assert!(!c.car_out[i], "a car at home is not also out");
            }
            if let (false, Some(job)) = (c.owns_car[i], c.job[i]) {
                let home = w.home_building(i as u32);
                assert!(w.front_distance(home, job) <= c.walk_limit[i]);
            }
        }
    }
    let s = w.stats();
    assert!(s.walks_completed > 0, "someone walked");
    assert!(s.trips_completed > s.walks_completed, "someone drove");
    advance(&mut w2, 3600);
    assert_eq!(w.state_hash(), w2.state_hash());
}

#[test]
fn road_edit_on_a_walk_restarts_it_from_the_last_place() {
    let mut w = town();
    isolate(&mut w);
    walker(&mut w, 0);
    let home = w.home_building(0);
    let target = distant(&w, 0);
    journey(&mut w, 0, target, Activity::Work);
    w.tick();
    let a = agent(&w, 0);
    let road = w.agents.path[a][0] / 2;
    w.remove_cars_using(&[road]);
    assert_eq!(w.agents.state[a], AgentState::Dormant);
    assert!(w.walking.is_empty());
    assert_eq!(w.citizens.location[0], home);
    assert_eq!(w.citizens.destination[0], Some(target));
    assert_eq!(w.totals.despawned_by_edit, 0, "walkers are not cars");
}

/// A walker on a red approach of a signalised cross waits at the stop line, calls for a
/// green although no car is waiting, and crosses once it comes.
#[test]
fn walkers_wait_at_red_signals_and_call_for_green() {
    let mut roads = RoadNetwork::default();
    let centre = roads.add_node(0.0, 0.0);
    for (x, y, class) in [
        (0.0, 100.0, RoadClass::Main),
        (0.0, -100.0, RoadClass::Main),
        (100.0, 0.0, RoadClass::Street),
        (-100.0, 0.0, RoadClass::Street),
    ] {
        let n = roads.add_node(x, y);
        assert!(roads.add_road(n, centre, class));
    }
    let mut w = World::new(Config::default(), roads, crate::rng::Rng::new(1));
    let c = w.citizens.push(0, 0, &mut crate::rng::Rng::new(3));
    // No household: only the walk itself runs.
    w.citizens.active[c as usize] = false;
    let out: Vec<_> = w.roads.out_lanes(centre).to_vec();
    let lane = out
        .iter()
        .map(|&l| l ^ 1)
        .find(|&l| !w.junctions.walk_allowed(&w.roads, l))
        .expect("one approach starts red");
    let next = *out.iter().find(|&&l| l != lane ^ 1).unwrap();
    let a = w.agents.push(centre) as usize;
    w.agents.citizen[a] = Some(c);
    w.citizens.agent[c as usize] = Some(a as u32);
    w.agents.path[a] = vec![lane, next];
    w.agents.lane[a] = lane;
    let stop = w.junctions.stop[lane as usize];
    w.agents.pos[a] = stop - 0.5;
    w.agents.walk_end[a] = w.roads.lane_len[next as usize];
    w.agents.state[a] = AgentState::Walking;
    w.walking.push(a as u32);

    advance(&mut w, 2);
    assert_eq!(w.agents.pos[a], stop);
    let mut snapshot = Vec::new();
    w.write_agent_snapshot(&mut snapshot);
    let entry = &snapshot[a * SNAPSHOT_STRIDE..(a + 1) * SNAPSHOT_STRIDE];
    assert_eq!((entry[3], entry[4]), (0.0, 1.0), "a waiting walker");
    let mut crossed = false;
    for _ in 0..600 {
        w.tick();
        // Signals change before walkers move, so the light after the tick is the one obeyed.
        if w.agents.lane[a] == next || w.agents.pos[a] > stop {
            assert!(
                w.junctions.walk_allowed(&w.roads, lane),
                "crossed on green only"
            );
            crossed = true;
            break;
        }
    }
    assert!(
        crossed,
        "the walker's call turned the signal within a minute"
    );
}
