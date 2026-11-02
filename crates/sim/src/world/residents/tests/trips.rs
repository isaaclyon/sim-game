use super::*;
use crate::agents::AgentState;

#[test]
fn same_junction_arrives_without_allocating_car_or_failing_path() {
    let mut w = town();
    isolate(&mut w);
    let home = w.home_building(0);
    let before = (
        w.agents.len(),
        w.totals.paths_failed,
        w.totals.trips_completed,
    );
    journey(&mut w, 0, home, Activity::Shop);
    assert_eq!(w.citizens.location[0], home);
    assert!(w.citizens.destination[0].is_none());
    assert_eq!(
        (w.agents.len(), w.totals.paths_failed),
        (before.0, before.1)
    );
    assert_eq!(w.totals.trips_completed, before.2 + 1);
}

#[test]
fn route_failure_waits_and_retries_without_arrival_or_losing_job() {
    let mut w = town();
    isolate(&mut w);
    driver(&mut w, 0);
    let home = w.home_building(0);
    let target = distant(&w, 0);
    assign_job(&mut w, 0, target);
    let from = w.building_node(home).unwrap();
    let roads: Vec<_> = w.roads.out_lanes(from).iter().map(|l| l / 2).collect();
    for r in roads {
        w.roads.remove_road(r);
    }
    w.sync_roads();
    let completed = w.totals.trips_completed;
    journey(&mut w, 0, target, Activity::Work);
    advance(&mut w, 100);
    assert_eq!(w.citizens.location[0], home);
    assert_eq!(w.citizens.destination[0], Some(target));
    assert_eq!(w.citizens.job[0], Some(target));
    assert_eq!(w.totals.trips_completed, completed);
    assert!(w.citizens.due[0] > w.totals.tick);
}

#[test]
fn road_invalidation_recovers_last_activity_and_reuses_same_identity() {
    let mut w = town();
    isolate(&mut w);
    driver(&mut w, 0);
    let home = w.home_building(0);
    let target = distant(&w, 0);
    journey(&mut w, 0, target, Activity::Work);
    w.tick();
    let a = w.citizens.agent[0].unwrap();
    assert_eq!(w.agents.state[a as usize], AgentState::Driving);
    let road = w.agents.path[a as usize][0] / 2;
    let completed = w.totals.trips_completed;
    // Splitting uses the same invalidation hook without removing the home.
    w.remove_cars_using(&[road]);
    assert_eq!(w.citizens.location[0], home);
    assert_eq!(w.agents.node[a as usize], w.building_node(home).unwrap());
    assert_eq!(w.agents.state[a as usize], AgentState::Dormant);
    assert_eq!(w.totals.trips_completed, completed);
    advance(&mut w, 31);
    assert_eq!(w.citizens.agent[0], Some(a));
    assert!(
        w.traffic
            .queues
            .iter()
            .flatten()
            .filter(|&&id| id == a)
            .count()
            <= 1
    );
}

#[test]
fn deferred_path_request_uses_current_access_and_does_not_fake_arrival() {
    let mut w = town();
    isolate(&mut w);
    driver(&mut w, 0);
    let target = distant(&w, 0);
    journey(&mut w, 0, target, Activity::Work);
    let a = w.citizens.agent[0].unwrap() as usize;
    let home = w.home_building(0);
    // Mimic an edit changing the frontage junction while the request awaits its budget.
    w.agents.node[a] = w.agents.dest[a];
    w.plan_paths();
    assert_eq!(w.agents.node[a], w.building_node(home).unwrap());
    assert_eq!(w.citizens.location[0], home);
    assert_eq!(w.citizens.destination[0], Some(target));
    assert_eq!(w.agents.state[a], AgentState::Entering);
}

#[test]
fn stuck_safety_valve_recovers_without_arrival() {
    let mut w = town();
    isolate(&mut w);
    driver(&mut w, 0);
    let home = w.home_building(0);
    let target = w
        .buildings
        .active_buildings()
        .find(|&b| w.buildings.zone[b as usize] == Zone::IndustrialLow)
        .unwrap();
    journey(&mut w, 0, target, Activity::Work);
    w.tick();
    let a = w.citizens.agent[0].unwrap() as usize;
    assert!(w.agents.path[a].len() >= 2);
    let lane = w.agents.path[a][0];
    let next = w.agents.path[a][1];
    let blocker = w.agents.push(w.roads.lane_from[next as usize]);
    w.traffic.enter(next, blocker, &mut w.agents);
    w.agents.pos[a] = w.roads.lane_len[lane as usize];
    w.agents.cleared[a] = true;
    w.cfg.stuck_limit_s = 0.1;
    let completed = w.totals.trips_completed;
    w.resolve_exits(&[lane]);
    assert_eq!(w.totals.despawned_stuck, 1);
    assert_eq!(w.totals.trips_completed, completed);
    assert_eq!(w.citizens.location[0], home);
    assert_eq!(w.citizens.destination[0], Some(target));
    assert_eq!(w.agents.state[a], AgentState::Dormant);
    assert_eq!(w.agents.node[a], w.building_node(home).unwrap());
}

#[test]
fn arrival_starts_duration_and_only_then_changes_location() {
    let mut w = town();
    isolate(&mut w);
    driver(&mut w, 0);
    let home = w.home_building(0);
    let target = distant(&w, 0);
    journey(&mut w, 0, target, Activity::Work);
    assert_eq!(w.citizens.location[0], home);
    assert_eq!(w.citizens.due[0], u64::MAX);
    let mut arrived = false;
    for _ in 0..3000 {
        w.tick();
        if w.citizens.destination[0].is_none() {
            assert_eq!(w.citizens.location[0], target);
            assert_eq!(
                w.citizens.due[0],
                w.totals.tick - 1 + w.secs_to_ticks(w.citizens.work_duration[0] as f32)
            );
            arrived = true;
            break;
        }
    }
    assert!(arrived);
    let agent = w.citizens.agent[0].unwrap();
    assert_eq!(w.agents.state[agent as usize], AgentState::Dormant);
}
