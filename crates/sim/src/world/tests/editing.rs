use super::*;
use crate::snap;

#[test]
fn painting_zones_is_hashed_and_survives_road_edits() {
    let mut world = roads(&[(0.0, 0.0, 100.0, 0.0), (0.0, 100.0, 100.0, 100.0)]);
    let before = world.state_hash();
    let paint = |zone| Command::PaintZone {
        ax: 0.0,
        ay: 0.0,
        bx: 100.0,
        by: 100.0,
        zone,
    };
    assert!(world.apply(paint(Zone::ResidentialHigh)));
    assert_ne!(world.state_hash(), before);
    assert!(!world.apply(paint(Zone::ResidentialHigh)));
    // Zones may cover roads; building or bulldozing a road leaves them alone.
    assert!(build(&mut world, 50.0, 0.0, 50.0, 100.0));
    assert!(world.apply(Command::DeleteRoad { x: 20.0, y: 0.0 }));
    assert_eq!(world.zones.zone_at(50.0, 0.0), Some(Zone::ResidentialHigh));
    assert_eq!(world.zones.zone_at(20.0, 50.0), Some(Zone::ResidentialHigh));
    assert!(world.apply(paint(Zone::None)));
    assert_eq!(world.zones.zone_at(20.0, 50.0), Some(Zone::None));
}

#[test]
fn bulldoze_keeps_ids_and_removes_routing_and_geometry() {
    let mut world = roads(&[(0.0, 0.0, 100.0, 0.0), (0.0, 100.0, 100.0, 100.0)]);
    let before = world.state_hash();
    let mut preview = Vec::new();
    world.write_bulldoze_preview(50.0, 4.0, &mut preview);
    assert_eq!(preview, [0.0, 0.0, 100.0, 0.0]);
    assert_eq!(world.state_hash(), before);
    assert!(world.apply(Command::DeleteRoad { x: 50.0, y: 4.0 }));
    assert_ne!(world.state_hash(), before);
    let version = world.roads.version;
    assert!(!world.apply(Command::DeleteRoad { x: 50.0, y: 4.0 }));
    assert!(!world.apply(Command::DeleteRoad {
        x: f32::NAN,
        y: 0.0
    }));
    assert_eq!(world.roads.version, version);
    assert_eq!(world.roads.road_count(), 1);
    assert_eq!(world.roads.lane_count(), 4);
    assert_eq!(world.roads.road_ends(1), (2, 3));
    assert!(world.roads.out_lanes(0).is_empty());
    assert!(world.roads.out_lanes(1).is_empty());
    let mut path = Vec::new();
    assert!(!Pathfinder::default().find(&world.roads, &[], 0, 1, &mut path));
    world.write_road_segments(&mut preview);
    assert_eq!(preview, [0.0, 100.0, 100.0, 100.0, 0.0]);
    // Deleted geometry neither blocks rebuilding nor gets split by a crossing.
    assert!(build(&mut world, 50.0, -100.0, 50.0, 50.0));
    assert_eq!(world.roads.road_count(), 2);
    assert!(build(&mut world, 0.0, 0.0, 100.0, 0.0));
    assert!(world.roads.active_roads().all(|r| r != 0));
}

#[test]
fn bulldoze_parks_affected_routes_and_updates_signals_while_paused() {
    let mut world = crossroads(RoadClass::Main);
    let on_deleted = wait_at_line(&mut world, 1, 2, 0) as usize;
    let using_later = wait_at_line(&mut world, 3, 0, 0) as usize;
    let unaffected = wait_at_line(&mut world, 5, 6, 0) as usize;
    let entering = world.agents.push(0) as usize;
    world.agents.path[entering] = vec![0];
    world.agents.state[entering] = AgentState::Entering;
    world.entering.push(entering as AgentId);
    assert!(world.apply(Command::DeleteRoad { x: 50.0, y: 0.0 }));
    assert_eq!(world.stats().despawned_by_edit, 3);
    for a in [on_deleted, using_later, entering] {
        assert_eq!(world.agents.state[a], AgentState::Parked);
        assert!(
            world
                .traffic
                .queues
                .iter()
                .all(|q| !q.contains(&(a as AgentId)))
        );
    }
    assert!(world.entering.is_empty());
    assert_eq!(world.agents.state[unaffected], AgentState::Driving);
    let mut signals = Vec::new();
    world.write_signal_snapshot(&mut signals);
    assert!(signals.is_empty(), "three arms no longer need signals");
    for _ in 0..600 {
        world.tick();
    }
    assert!(world.stats().trips_completed > 0);
}

#[test]
fn bulldoze_picking_has_bounded_reach_and_stable_ties() {
    let mut world = roads(&[(0.0, 0.0, 100.0, 0.0), (0.0, 30.0, 100.0, 30.0)]);
    assert_eq!(snap::bulldoze_target(&world.roads, 50.0, 15.0), Some(0));
    assert_eq!(snap::bulldoze_target(&world.roads, -16.0, 0.0), None);
    assert_eq!(
        snap::bulldoze_target(&world.roads, 50.0, f32::INFINITY),
        None
    );
    assert!(world.roads.remove_road(0));
    let version = world.roads.version;
    assert!(!world.roads.remove_road(0));
    assert!(!world.roads.remove_road(u32::MAX));
    assert_eq!(world.roads.version, version);
    assert_eq!(snap::bulldoze_target(&world.roads, 50.0, 15.0), Some(1));
    let mid = world.roads.add_node(50.0, 30.0);
    let rest = world.roads.split_road(1, mid);
    assert_eq!(
        rest, 2,
        "splitting uses the next slot, not the active count"
    );
    assert_eq!(world.roads.road_ends(rest), (mid, 3));
}

#[test]
fn bulldoze_every_road_is_safe_and_deterministic() {
    let run = || {
        let mut world = roads(&[(0.0, 0.0, 100.0, 0.0)]);
        world.apply(Command::SpawnAgents { count: 50 });
        for _ in 0..300 {
            world.tick();
        }
        assert!(world.apply(Command::DeleteRoad { x: 50.0, y: 0.0 }));
        for _ in 0..600 {
            world.tick();
        }
        assert_eq!(world.stats().driving, 0);
        assert!(!world.apply(Command::SpawnAgents { count: 1 }));
        assert!(build(&mut world, 0.0, 0.0, 100.0, 0.0));
        assert_eq!(world.roads.active_roads().collect::<Vec<_>>(), [1]);
        for _ in 0..600 {
            world.tick();
        }
        assert!(world.stats().trips_completed > 0);
        world.state_hash()
    };
    assert_eq!(run(), run());
}

#[test]
fn build_road_snaps_to_existing_nodes() {
    let mut world = roads(&[(0.0, 0.0, 100.0, 0.0), (0.0, 200.0, 100.0, 200.0)]);
    assert!(build(&mut world, 3.0, 0.0, 100.0, 197.0));
    assert_eq!(world.roads.node_count(), 4, "both ends should snap");
    assert_eq!(world.roads.road_count(), 3);
    assert!(!build(&mut world, 0.0, 0.0, 100.0, 200.0), "duplicate");
}

#[test]
fn main_roads_keep_their_class_when_split() {
    let mut world = roads(&[]);
    assert!(world.apply(Command::BuildRoad {
        ax: -100.0,
        ay: 0.0,
        bx: 100.0,
        by: 0.0,
        class: RoadClass::Main,
    }));
    assert!(build(&mut world, 0.0, -100.0, 0.0, 100.0));
    let r = &world.roads;
    let classes: Vec<_> = r.active_roads().map(|road| r.road_class(road)).collect();
    assert_eq!(
        classes,
        [
            RoadClass::Main,
            RoadClass::Main,
            RoadClass::Street,
            RoadClass::Street
        ]
    );
    assert_eq!(r.lane_speed[0], crate::road::MAIN_SPEED);
    let mut segments = Vec::new();
    world.write_road_segments(&mut segments);
    let snapshot: Vec<f32> = segments.chunks(5).map(|s| s[4]).collect();
    assert_eq!(snapshot, [1.0, 1.0, 0.0, 0.0], "class per segment");
    // Following an existing road keeps it as it is.
    assert!(!world.apply(Command::BuildRoad {
        ax: -100.0,
        ay: 0.0,
        bx: 0.0,
        by: 0.0,
        class: RoadClass::Street,
    }));
    assert_eq!(world.roads.road_class(0), RoadClass::Main);
}

#[test]
fn crossing_roads_meet_at_a_new_junction() {
    let mut world = roads(&[(-100.0, 0.0, 100.0, 0.0)]);
    assert!(build(&mut world, 0.0, -100.0, 0.0, 100.0));
    assert_eq!(world.roads.node_count(), 5);
    assert_eq!(world.roads.road_count(), 4);
    let centre = node_at(&world, 0.0, 0.0);
    assert_eq!(world.roads.out_lanes(centre).len(), 4);
    // The old road's ends are now reachable from the new road's ends.
    let (west, north) = (node_at(&world, -100.0, 0.0), node_at(&world, 0.0, 100.0));
    let mut path = Vec::new();
    assert!(Pathfinder::default().find(&world.roads, &[], west, north, &mut path));
    assert_eq!(path.len(), 2);
}

#[test]
fn road_ending_beside_a_road_makes_a_t_junction() {
    let mut world = roads(&[(-100.0, 0.0, 100.0, 0.0)]);
    assert!(build(&mut world, 0.0, -100.0, 0.0, -6.0));
    assert_eq!(world.roads.node_count(), 4);
    assert_eq!(world.roads.road_count(), 3);
    assert_eq!(world.roads.out_lanes(node_at(&world, 0.0, 0.0)).len(), 3);
}

#[test]
fn crossing_near_a_junction_joins_it() {
    let mut world = roads(&[(0.0, 0.0, 100.0, 0.0)]);
    assert!(build(&mut world, 5.0, -50.0, 5.0, 50.0));
    assert_eq!(world.roads.node_count(), 4, "no node 5 m from the end");
    assert_eq!(world.roads.road_count(), 3);
    assert_eq!(world.roads.out_lanes(0).len(), 3);
}

#[test]
fn road_passing_through_a_junction_does_not_run_along_its_roads() {
    // Roads east and north-east from the origin. The new road crosses the east road 12 m
    // out, so it joins the origin, then would cross the north-east road 39 m out.
    let mut net = RoadNetwork::default();
    let (c, e, ne) = (
        net.add_node(0.0, 0.0),
        net.add_node(100.0, 0.0),
        net.add_node(60.0, 100.0),
    );
    net.add_road(c, e, RoadClass::Street);
    net.add_road(c, ne, RoadClass::Street);
    let mut world = World::new(Config::default(), net, Rng::new(1));
    let mut out = Vec::new();
    world.write_road_preview(0.0, -50.0, 36.0, 100.0, &mut out);
    let joins: Vec<f32> = out.chunks(3).map(|s| s[2]).collect();
    assert_eq!(joins, [0.0, 1.0, 0.0], "through the origin, no split");
    assert!(build(&mut world, 0.0, -50.0, 36.0, 100.0));
    assert_eq!(world.roads.road_count(), 4);
    assert_eq!(world.roads.out_lanes(c).len(), 4);
}

#[test]
fn road_on_top_of_a_road_is_refused() {
    let mut world = roads(&[(-100.0, 0.0, 100.0, 0.0)]);
    assert!(!build(&mut world, -50.0, 1.0, 50.0, 1.0), "both ends on it");
    assert!(!build(&mut world, -50.0, 3.0, 150.0, 3.0), "runs along it");
    assert!(!build(&mut world, -100.0, 0.0, 100.0, 0.0), "duplicate");
    assert_eq!(world.roads.node_count(), 2);
    assert_eq!(world.roads.road_count(), 1);
}

#[test]
fn crossings_too_close_together_are_refused() {
    let mut world = roads(&[(-100.0, 0.0, 100.0, 0.0), (-100.0, 6.0, 100.0, 6.0)]);
    assert!(!build(&mut world, 0.0, -100.0, 0.0, 100.0));
    assert_eq!(world.roads.road_count(), 2);
}

#[test]
fn splitting_a_road_removes_cars_that_use_it() {
    // West road (lanes 0 east, 1 west) and east road (lanes 2 east, 3 west), meeting at x=0.
    let mut net = RoadNetwork::default();
    let (w, c, e) = (
        net.add_node(-200.0, 0.0),
        net.add_node(0.0, 0.0),
        net.add_node(200.0, 0.0),
    );
    net.add_road(w, c, RoadClass::Street);
    net.add_road(c, e, RoadClass::Street);
    let mut world = World::new(Config::default(), net, Rng::new(1));
    let drive = |world: &mut World, path: Vec<LaneId>| {
        let agent = world.agents.push(world.roads.lane_from[path[0] as usize]);
        let a = agent as usize;
        world.agents.dest[a] = world.roads.lane_to[*path.last().unwrap() as usize];
        world.agents.path[a] = path.clone();
        world.traffic.enter(path[0], agent, &mut world.agents);
        agent as usize
    };
    let on_west = drive(&mut world, vec![0, 2]);
    let heading_west = drive(&mut world, vec![3, 1]);
    let east_only = drive(&mut world, vec![2]);
    assert!(build(&mut world, -100.0, -100.0, -100.0, 100.0));
    let s = world.stats();
    assert_eq!(s.despawned_by_edit, 2, "{s:?}");
    assert_eq!(world.agents.state[on_west], AgentState::Parked);
    assert_eq!(world.agents.state[heading_west], AgentState::Parked);
    assert_eq!(world.agents.state[east_only], AgentState::Driving);
    assert!(
        world
            .traffic
            .queues
            .iter()
            .all(|q| !q.contains(&(on_west as AgentId)))
    );
    for _ in 0..600 {
        world.tick();
    }
    assert!(world.stats().trips_completed >= 1);
}

#[test]
fn hover_shows_where_a_road_would_start() {
    let world = roads(&[(-100.0, 0.0, 100.0, 0.0)]);
    let mut out = Vec::new();
    world.write_snap_preview(0.0, 6.0, &mut out);
    assert_eq!(out, [0.0, 0.0, 2.0], "on the road, splitting it");
    world.write_snap_preview(-95.0, 4.0, &mut out);
    assert_eq!(out, [-100.0, 0.0, 1.0], "at its end");
    world.write_snap_preview(0.0, 50.0, &mut out);
    assert_eq!(out, [0.0, 50.0, 0.0], "a new end");
}

#[test]
fn preview_shows_what_gets_built() {
    let world = roads(&[(-100.0, 0.0, 100.0, 0.0)]);
    let mut out = Vec::new();
    world.write_road_preview(0.0, -100.0, 0.0, 100.0, &mut out);
    assert_eq!(
        out,
        [0.0, -100.0, 0.0, 0.0, 0.0, 2.0, 0.0, 100.0, 0.0],
        "[x, y, join] per stop: new end, split, new end"
    );
    world.write_road_preview(-50.0, 1.0, 50.0, 1.0, &mut out);
    assert!(out.is_empty(), "refused");
    let before = world.roads.version;
    world.write_road_preview(0.0, -100.0, 0.0, 100.0, &mut out);
    assert_eq!(world.roads.version, before, "preview changes nothing");
}
