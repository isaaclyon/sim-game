//! Dispatch, trucks in traffic, saves and determinism.

use super::*;
use crate::agents::AgentId;
use crate::road::{LaneId, RoadClass, RoadNetwork};
use crate::traffic::SPACING;
use crate::world::fire::{BURN_S, Task};

/// West of the road at x = 0 and east of the road at x = 360, clear of the coal plant.
const WEST: (f32, f32) = (-20.0, 60.0);
const EAST: (f32, f32) = (380.0, 200.0);

/// Standing buildings whose junction is no station's, farthest east first.
fn eastmost(w: &World) -> Vec<BuildingId> {
    let stations: Vec<_> = w.fire.cover.node.iter().flatten().copied().collect();
    let mut out: Vec<_> = standing(w)
        .into_iter()
        .filter(|&b| w.building_node(b).is_some_and(|n| !stations.contains(&n)))
        .collect();
    let x = |b: BuildingId| w.roads.node_x[w.building_node(b).unwrap() as usize];
    out.sort_by(|&a, &b| x(b).total_cmp(&x(a)).then(a.cmp(&b)));
    out
}

fn burn(w: &mut World, b: BuildingId, secs: f32) {
    let until = w.totals.tick + w.secs_to_ticks(secs);
    w.start_fire(b, until);
}

fn truck_kinds(w: &World) -> usize {
    let mut snap = Vec::new();
    w.write_agent_snapshot(&mut snap);
    snap.chunks(crate::world::SNAPSHOT_STRIDE)
        .filter(|a| a[3] >= 0.0 && a[4] == 2.0)
        .count()
}

#[test]
fn a_truck_that_arrives_in_time_saves_the_building_and_drives_home() {
    let mut w = town(600);
    assert!(station(&mut w, WEST.0, WEST.1));
    let b = eastmost(&w)[0];
    burn(&mut w, b, BURN_S);
    w.tick();
    assert_eq!(w.fire_stats().free_trucks, 1, "one truck is sent");
    assert_eq!(w.fire_stats().unattended, 0);
    let mut seen_driving = false;
    let secs = (0..BURN_S as u32).find(|_| {
        advance(&mut w, 1);
        seen_driving |= truck_kinds(&w) == 1;
        w.fire_stats().burning == 0
    });
    assert!(secs.is_some() && seen_driving);
    let s = w.fire_stats();
    assert_eq!((s.saved, s.destroyed), (1, 0));
    assert!(w.buildings.active[b as usize]);
    assert!(
        (0..300).any(|_| {
            advance(&mut w, 1);
            w.fire_stats().free_trucks == 2
        }),
        "the truck comes home"
    );
    assert_eq!(truck_kinds(&w), 0, "trucks at home are not drawn");
}

#[test]
fn a_fire_that_beats_the_truck_destroys_the_building_and_the_truck_turns_back() {
    let mut w = town(600);
    assert!(station(&mut w, WEST.0, WEST.1));
    let b = eastmost(&w)[0];
    burn(&mut w, b, 3.0);
    advance(&mut w, 4);
    let s = w.fire_stats();
    assert_eq!((s.saved, s.destroyed), (0, 1));
    assert!(w.fire.truck_task.contains(&Task::Returning));
    assert!((0..300).any(|_| {
        advance(&mut w, 1);
        w.fire_stats().free_trucks == 2
    }));
}

#[test]
fn fires_get_the_nearest_covering_station_with_a_free_truck() {
    let mut w = town(600);
    assert!(station(&mut w, WEST.0, WEST.1));
    assert!(station(&mut w, EAST.0, EAST.1));
    let east = eastmost(&w);
    burn(&mut w, east[0], BURN_S);
    w.tick();
    let station_of =
        |w: &World, f: usize| w.fire.truck_station[w.fire.fire_truck[f].unwrap() as usize];
    assert_eq!(station_of(&w, 0), 1, "the east station is nearer");
    burn(&mut w, east[1], BURN_S);
    burn(&mut w, east[2], BURN_S);
    w.tick();
    let stations: Vec<_> = (0..3).map(|f| station_of(&w, f)).collect();
    assert_eq!(stations, [1, 1, 0], "the east station has two trucks");
    assert_eq!(w.fire_stats().free_trucks, 1);
}

#[test]
fn trucks_cross_red_lights_but_queue_behind_cars() {
    // A signalised crossroads; arm k (east, north, west, south) owns lanes 2k (out) and
    // 2k + 1 (in).
    let world = || {
        let mut net = RoadNetwork::default();
        let centre = net.add_node(0.0, 0.0);
        for k in 0..4 {
            let angle = k as f32 * std::f32::consts::FRAC_PI_2;
            let arm = net.add_node(100.0 * angle.cos(), 100.0 * angle.sin());
            let class = if k % 2 == 0 {
                RoadClass::Main
            } else {
                RoadClass::Street
            };
            net.add_road(centre, arm, class);
        }
        World::new(Config::default(), net, crate::rng::Rng::new(1))
    };
    let at_line = |w: &mut World, from: LaneId, to: LaneId, back: f32| -> AgentId {
        let agent = w.agents.push(w.roads.lane_from[from as usize]);
        let a = agent as usize;
        w.agents.dest[a] = w.roads.lane_to[to as usize];
        w.agents.path[a] = vec![from, to];
        w.traffic.enter(from, agent, &mut w.agents);
        w.agents.pos[a] = w.junctions.stop[from as usize] - back;
        agent
    };
    // Find an approach that starts red: a car there waits.
    let red = (0..4).map(|k| 2 * k + 1).find(|&from| {
        let mut w = world();
        let car = at_line(&mut w, from, (from + 3) % 8, 0.0) as usize;
        (0..20).all(|_| {
            w.tick();
            w.agents.lane[car] == from && !w.agents.cleared[car]
        })
    });
    let from = red.expect("a red approach");
    let to = (from + 3) % 8;

    let mut w = world();
    let truck = at_line(&mut w, from, to, 0.0);
    w.fire.mark_truck(truck);
    let crossed = (0..20).any(|_| {
        w.tick();
        w.agents.lane[truck as usize] != from || w.agents.cleared[truck as usize]
    });
    assert!(crossed, "the truck runs the red light");

    let mut w = world();
    let car = at_line(&mut w, from, to, 0.0) as usize;
    let truck = at_line(&mut w, from, to, SPACING) as usize;
    w.fire.mark_truck(truck as AgentId);
    for _ in 0..20 {
        w.tick();
        assert_eq!(w.agents.lane[car], from, "the car waits for green");
        assert_eq!(w.agents.lane[truck], from, "the truck waits behind it");
        assert!(w.agents.pos[truck] < w.agents.pos[car]);
    }
}

#[test]
fn fire_state_saves_hashes_and_replays_identically() {
    let run = || {
        let mut w = town(600);
        assert!(station(&mut w, WEST.0, WEST.1));
        let east = eastmost(&w);
        burn(&mut w, east[0], BURN_S);
        burn(&mut w, east[1], 2.0);
        advance(&mut w, 5);
        w
    };
    let mut a = run();
    let mut b = run();
    assert_eq!(a.state_hash(), b.state_hash());
    let mut loaded = World::load(&a.save().unwrap()).unwrap();
    assert_eq!(loaded.state_hash(), a.state_hash());
    assert_eq!(loaded.fire_stats(), a.fire_stats());
    advance(&mut a, 200);
    advance(&mut loaded, 200);
    assert_eq!(loaded.state_hash(), a.state_hash());
    assert_eq!(loaded.fire_stats(), a.fire_stats());
    assert!(a.fire_stats().saved == 1 && a.fire_stats().destroyed == 1);

    // The setting is hashed.
    let before = b.state_hash();
    assert!(b.apply(Command::SetFires { on: false }));
    assert_ne!(b.state_hash(), before);
}

#[test]
fn inconsistent_fire_state_is_refused_on_load() {
    let mut w = town(60);
    assert!(station(&mut w, WEST.0, WEST.1));
    let refused = |w: &World| World::load(&w.save().unwrap()).err();
    assert_eq!(refused(&w), None);
    let mut bad = World::load(&w.save().unwrap()).unwrap();
    bad.fire.truck_agent[0] = u32::MAX;
    assert!(matches!(refused(&bad), Some(crate::SaveError::Invalid(_))));
    let mut bad = World::load(&w.save().unwrap()).unwrap();
    bad.fire.fire_building.push(u32::MAX);
    bad.fire.fire_until.push(0);
    bad.fire.fire_truck.push(None);
    assert!(matches!(refused(&bad), Some(crate::SaveError::Invalid(_))));
}

#[test]
fn a_city_without_fire_state_hashes_as_before() {
    let mut w = town(60);
    let mut mixed = 0;
    w.fire.hash_into(w.cfg.seed, &mut |_| mixed += 1);
    assert_eq!(mixed, 0, "nothing mixed while fire state is untouched");
    assert!(w.apply(Command::SetFires { on: false }));
    w.fire.hash_into(w.cfg.seed, &mut |_| mixed += 1);
    assert!(mixed > 0);
}
