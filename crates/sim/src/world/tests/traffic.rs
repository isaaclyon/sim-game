use super::*;
use crate::traffic::SPACING;

#[test]
fn same_seed_same_city() {
    let (mut a, mut b) = (small_world(7), small_world(7));
    for _ in 0..1_500 {
        a.tick();
        b.tick();
    }
    assert_eq!(a.state_hash(), b.state_hash());
    assert_ne!(a.state_hash(), {
        let mut c = small_world(8);
        (0..1_500).for_each(|_| c.tick());
        c.state_hash()
    });
}

#[test]
fn agents_complete_trips() {
    let mut world = small_world(1);
    for _ in 0..3_000 {
        world.tick();
    }
    let s = world.stats();
    assert!(s.trips_completed > 500, "{s:?}");
    assert_eq!(
        s.parked + s.awaiting_path + s.entering + s.driving,
        s.agents
    );
}

#[test]
fn building_across_busy_traffic_keeps_it_running() {
    let mut world = small_world(3);
    for _ in 0..1_500 {
        world.tick();
    }
    let b = world.roads.bounds();
    let (w, h) = (b[2] - b[0], b[3] - b[1]);
    // Long diagonals, crossing many streets each. A few may cross a jittered street at too
    // sharp an angle and be refused, which is fine; most must build.
    let mut built = 0;
    for k in 0..4 {
        let f = k as f32 / 8.0;
        let (y0, y1) = (b[1] + f * h, b[1] + (f + 0.5) * h);
        let (x0, x1) = (b[0] + f * w, b[0] + (f + 0.5) * w);
        built += build(&mut world, b[0] + 7.0, y0, b[2] - 7.0, y1) as u32;
        built += build(&mut world, x0, b[1] + 7.0, x1, b[3] - 7.0) as u32;
        for _ in 0..300 {
            world.tick();
        }
    }
    assert!(built >= 5, "built {built} of 8");
    let trips = world.stats().trips_completed;
    for _ in 0..1_500 {
        world.tick();
    }
    let s = world.stats();
    assert!(s.despawned_by_edit > 0, "{s:?}");
    assert!(s.trips_completed > trips + 200, "{s:?}");
    assert_eq!(
        s.parked + s.awaiting_path + s.entering + s.driving,
        s.agents
    );
    // Every queued car is on the lane it thinks it is on.
    for (lane, q) in world.traffic.queues.iter().enumerate() {
        for &agent in q {
            assert_eq!(world.agents.lane[agent as usize] as usize, lane);
            assert_eq!(world.agents.state[agent as usize], AgentState::Driving);
        }
    }
}

#[test]
fn waiting_cars_never_wait_for_each_other() {
    // Long waits (past patience) a tick or two apart, compared in lane order. Counting a
    // car's wait as soon as its lane was visited once made pairs of cars that each thought
    // the other had waited longer, and both sat until the stuck limit.
    const LONG: u16 = 300;
    let check = |cars: &[(LaneId, LaneId, u16)]| {
        let mut world = crossroads(RoadClass::Street);
        for &(from, to, waited) in cars {
            wait_at_line(&mut world, from, to, waited);
        }
        for _ in 0..600 {
            world.tick();
        }
        let s = world.stats();
        assert_eq!(
            (s.trips_completed, s.despawned_stuck),
            (cars.len() as u64, 0),
            "{cars:?}"
        );
    };
    // Lanes: arm k (east, north, west, south) enters on 2k + 1 and leaves on 2k.
    let (in_e, in_s, out_w) = (1, 7, 4);
    for offset in 0..5 {
        // Merging into one exit: the car from the south yields to the one on its right.
        check(&[(in_e, out_w, LONG), (in_s, out_w, LONG - 2 + offset)]);
    }
    for code in 0..81 {
        // Four cars going straight on, each yielding to the one on its right.
        let waits = [code % 3, code / 3 % 3, code / 9 % 3, code / 27];
        let cars: Vec<_> = (0..4u32)
            .map(|k| (2 * k + 1, 2 * ((k + 2) % 4), LONG + waits[k as usize]))
            .collect();
        check(&cars);
    }
}

#[test]
fn signal_turns_green_for_waiting_traffic() {
    let mut world = crossroads(RoadClass::Main);
    let mut signals = Vec::new();
    world.write_signal_snapshot(&mut signals);
    assert_eq!(signals.len(), 4 * SIGNAL_STRIDE);
    // Arm k's incoming lane is 2k + 1; its straight-on exit is the opposite arm's lane.
    let roads = world.roads.clone();
    let state = |signals: &[f32], lane: LaneId| {
        let heading = roads.lane_heading(lane, roads.lane_len[lane as usize]);
        signals
            .chunks(SIGNAL_STRIDE)
            .find(|s| (s[2] - heading).abs() < 1e-3)
            .expect("a signal head per approach")[3]
    };
    let from = (0..4)
        .map(|k| 2 * k + 1)
        .find(|&l| state(&signals, l) == 0.0)
        .expect("a red approach");
    let to = (from + 3) % 8;
    let car = wait_at_line(&mut world, from, to, 0) as usize;
    let red_ticks = (0..300)
        .take_while(|_| {
            world.tick();
            world.agents.lane[car] == from && !world.agents.cleared[car]
        })
        .count();
    // Green stays with the other pair for its minimum time, then switches on demand.
    assert!((70..150).contains(&red_ticks), "red for {red_ticks} ticks");
    assert_eq!(
        world.agents.stuck_ticks[car], 0,
        "a red light is not gridlock"
    );
    world.write_signal_snapshot(&mut signals);
    assert_eq!(state(&signals, from), 1.0, "green for the waiting car");
}

#[test]
fn queue_discharges_without_stopping_each_car() {
    // Ten cars queued at an empty junction, all going straight on. Once the queue is moving,
    // nobody should stop again. Letting only the front car ask, and only once the car ahead
    // had left the junction, made every car stop at the line and start from rest.
    let mut world = crossroads(RoadClass::Street);
    let (from, to) = (5, 0); // West arm in, east arm out.
    let cars: Vec<_> = (0..10)
        .map(|k| {
            let car = wait_at_line(&mut world, from, to, 0);
            world.agents.pos[car as usize] -= k as f32 * SPACING;
            car
        })
        .collect();
    let mut started = [false; 10];
    let ticks = (0..1_000)
        .take_while(|_| {
            world.tick();
            for (k, &c) in cars.iter().enumerate() {
                let c = c as usize;
                if world.agents.lane[c] != from {
                    continue;
                }
                let speed = world.agents.speed[c];
                assert!(
                    !started[k] || speed > 1.0,
                    "car {k} stopped again at {} m",
                    world.agents.pos[c]
                );
                started[k] |= speed > 2.0;
            }
            cars.iter().any(|&c| world.agents.lane[c as usize] == from)
        })
        .count();
    let secs = ticks as f32 * world.cfg.dt;
    assert!(secs < 15.0, "queue took {secs} s to clear");
}

#[test]
fn dense_traffic_does_not_gridlock() {
    // 6,000 cars fill most of this small town's road space. Before intersection rules it
    // lost hundreds of them to the stuck limit in these ten minutes.
    let mut world = small_world(1);
    world.apply(Command::SpawnAgents { count: 4_000 });
    for _ in 0..6_000 {
        world.tick();
    }
    let s = world.stats();
    assert!(s.trips_completed > 1_500, "{s:?}");
    assert!(s.despawned_stuck <= 5, "{s:?}");
}
