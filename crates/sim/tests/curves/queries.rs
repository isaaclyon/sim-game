//! Curve geometry queries, previews, junction turns and traffic on curves.

use sim::Command;
use sim::road::{RoadClass, RoadNetwork};

use super::{close, curve, empty};

#[test]
fn curve_queries_use_distance_tangents_and_reverse_lanes() {
    let mut net = RoadNetwork::default();
    let a = net.add_node(0.0, 0.0);
    let b = net.add_node(200.0, 0.0);
    assert!(net.add_curve(a, b, (100.0, 100.0), RoadClass::Street));
    let len = net.lane_len[0];
    assert!((len - 229.5587).abs() < 0.02);
    close(net.lane_point(0, len / 2.0, 0.0), (100.0, 50.0));
    close(net.lane_point(1, len / 2.0, 2.0), (100.0, 52.0));
    assert!((net.lane_heading(0, 0.0) - std::f32::consts::FRAC_PI_4).abs() < 0.001);
    assert!((net.lane_heading(0, len) + std::f32::consts::FRAC_PI_4).abs() < 0.001);
    for i in 0..=20 {
        let d = len * i as f32 / 20.0;
        close(net.lane_point(0, d, 0.0), net.lane_point(1, len - d, 0.0));
    }
    close(net.lane_point(0, -10.0, 0.0), (0.0, 0.0));
    close(net.lane_point(0, len + 10.0, 0.0), (200.0, 0.0));
}

#[test]
fn preview_matches_built_curve_without_mutating_world() {
    let mut w = empty();
    let mut preview = Vec::new();
    w.write_curve_preview(0.0, 0.0, 100.0, 100.0, 200.0, 0.0, &mut preview);
    assert_eq!(w.roads.road_count(), 0);
    assert!(preview.len() > 6);
    assert!(w.apply(curve()));
    assert_eq!(w.roads.road_count(), 1);
    let mut segments = Vec::new();
    w.write_road_segments(&mut segments);
    assert_eq!(segments.len() / 5, preview.len() / 3 - 1);
    for (s, p) in segments
        .as_chunks::<5>()
        .0
        .iter()
        .zip(preview.windows(6).step_by(3))
    {
        close((s[0], s[1]), (p[0], p[1]));
        close((s[2], s[3]), (p[3], p[4]));
    }
}

#[test]
fn curve_junction_turns_use_local_endpoint_tangents() {
    use sim::junction::{Junctions, Turn};
    let mut net = RoadNetwork::default();
    let a = net.add_node(0.0, 0.0);
    let b = net.add_node(200.0, 0.0);
    let c = net.add_node(300.0, -100.0);
    net.add_curve(a, b, (100.0, 100.0), RoadClass::Street);
    net.add_curve(b, c, (250.0, -50.0), RoadClass::Street);
    net.rebuild_adjacency();
    let mut junctions = Junctions::default();
    junctions.sync(&net);
    assert_eq!(junctions.turn(0, 2), Turn::Straight);
    assert_eq!(junctions.turn(3, 1), Turn::Straight);
}

#[test]
fn curve_traffic_replays_and_survives_a_split() {
    fn run() -> (u64, u64) {
        let mut w = empty();
        assert!(w.apply(curve()));
        assert!(w.apply(Command::SpawnAgents { count: 100 }));
        for _ in 0..1000 {
            w.tick();
        }
        let mut snapshot = Vec::new();
        w.write_agent_snapshot(&mut snapshot);
        assert!(
            snapshot
                .as_chunks::<{ sim::world::SNAPSHOT_STRIDE }>()
                .0
                .iter()
                .any(|s| s[3] >= 0.0)
        );
        for (i, s) in snapshot
            .as_chunks::<{ sim::world::SNAPSHOT_STRIDE }>()
            .0
            .iter()
            .enumerate()
            .filter(|(_, s)| s[3] >= 0.0)
        {
            let lane = w.agents.lane[i];
            let pos = w.agents.pos[i];
            close((s[0], s[1]), w.roads.lane_point(lane, pos, 2.0));
            assert_eq!(s[2], w.roads.lane_heading(lane, pos));
        }
        assert!(w.apply(Command::BuildRoad {
            ax: 100.0,
            ay: -100.0,
            bx: 100.0,
            by: 100.0,
            class: RoadClass::Street
        }));
        assert!(w.stats().despawned_by_edit > 0);
        for _ in 0..3000 {
            w.tick();
        }
        assert!(w.stats().trips_completed > 100);
        (w.state_hash(), w.stats().trips_completed)
    }
    assert_eq!(run(), run());
}

#[test]
fn routes_use_curve_length_and_bounds_include_the_bend() {
    let mut net = RoadNetwork::default();
    let a = net.add_node(0.0, 0.0);
    let b = net.add_node(200.0, 0.0);
    let c = net.add_node(100.0, -100.0);
    net.add_curve(a, b, (100.0, 500.0), RoadClass::Street);
    net.add_road(a, c, RoadClass::Street);
    net.add_road(c, b, RoadClass::Street);
    net.rebuild_adjacency();
    let mut path = Vec::new();
    assert!(sim::path::Pathfinder::default().find(&net, &[], None, a, b, &mut path));
    assert_eq!(path, vec![2, 4]);
    assert!(net.bounds()[3] >= 250.0);
}

#[test]
fn nonuniform_parameter_does_not_change_distance_travelled() {
    let mut net = RoadNetwork::default();
    let a = net.add_node(0.0, 0.0);
    let b = net.add_node(200.0, 0.0);
    net.add_curve(a, b, (10.0, 0.0), RoadClass::Street);
    for d in 0..=200 {
        close(net.lane_point(0, d as f32, 2.0), (d as f32, -2.0));
    }
}
