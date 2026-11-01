use sim::rng::Rng;
use sim::road::{RoadNetwork, STREET_SPEED};
use sim::{Command, Config, World};

fn empty() -> World {
    World::new(Config::default(), RoadNetwork::default(), Rng::new(1))
}
fn curve() -> Command {
    Command::BuildCurve {
        ax: 0.0,
        ay: 0.0,
        cx: 100.0,
        cy: 100.0,
        bx: 200.0,
        by: 0.0,
    }
}
fn close(a: (f32, f32), b: (f32, f32)) {
    assert!((a.0 - b.0).hypot(a.1 - b.1) < 0.02, "{a:?} != {b:?}");
}

#[test]
fn bulldoze_highlights_and_removes_the_whole_curve_not_its_chord() {
    let mut w = empty();
    assert!(w.apply(curve()));
    let hash = w.state_hash();
    let mut preview = Vec::new();
    w.write_bulldoze_preview(100.0, 0.0, &mut preview);
    assert!(preview.is_empty(), "the endpoint chord is not a road");
    w.write_bulldoze_preview(100.0, 50.0, &mut preview);
    let mut segments = Vec::new();
    w.write_road_segments(&mut segments);
    assert_eq!(preview.len() / 2, segments.len() / 5 + 1);
    for (p, s) in preview.windows(4).step_by(2).zip(segments.chunks_exact(5)) {
        assert_eq!(p, &s[..4]);
    }
    assert_eq!(w.state_hash(), hash, "hover does not change the world");
    assert!(!w.apply(Command::DeleteRoad { x: 100.0, y: 0.0 }));
    assert!(w.apply(Command::DeleteRoad { x: 100.0, y: 50.0 }));
    assert_eq!(w.roads.road_count(), 0);
    assert_eq!(w.roads.lane_count(), 2);
    assert_ne!(w.state_hash(), hash);
    w.write_road_segments(&mut segments);
    assert!(segments.is_empty());
    w.write_bulldoze_preview(100.0, 50.0, &mut preview);
    assert!(preview.is_empty());
    assert!(w.apply(curve()), "deleted curves do not block rebuilding");
    assert_eq!(w.roads.active_roads().collect::<Vec<_>>(), [1]);
}

#[test]
fn curves_after_deleted_slots_still_render_snap_and_split() {
    let mut w = empty();
    assert!(w.apply(Command::BuildRoad {
        ax: -500.0,
        ay: -500.0,
        bx: -300.0,
        by: -500.0
    }));
    assert!(w.apply(curve()));
    assert!(w.apply(Command::DeleteRoad {
        x: -400.0,
        y: -500.0
    }));
    let mut segments = Vec::new();
    w.write_road_segments(&mut segments);
    assert!(segments.len() > 5);
    assert!(segments.chunks_exact(5).all(|s| s[0] >= 0.0 && s[1] >= 0.0));
    let mut preview = Vec::new();
    w.write_snap_preview(100.0, 50.0, &mut preview);
    assert_eq!(preview, [100.0, 50.0, 2.0]);
    assert!(w.apply(Command::BuildRoad {
        ax: 100.0,
        ay: -100.0,
        bx: 100.0,
        by: 50.0
    }));
    let n = w.roads.nearest_node(100.0, 50.0, 0.02).unwrap();
    assert_eq!(w.roads.out_lanes(n).len(), 3);
    assert_eq!(w.roads.active_roads().collect::<Vec<_>>(), [1, 2, 3]);
    close(
        w.roads.lane_point(2, w.roads.lane_len[2], 0.0),
        (100.0, 50.0),
    );
    close(w.roads.lane_point(4, 0.0, 0.0), (100.0, 50.0));
}

#[test]
fn extending_a_rebuilt_road_follows_the_active_curve() {
    let mut w = empty();
    assert!(w.apply(curve()));
    assert!(w.apply(Command::DeleteRoad { x: 100.0, y: 50.0 }));
    assert!(w.apply(Command::BuildCurve {
        ax: 0.0,
        ay: 0.0,
        cx: 100.0,
        cy: -100.0,
        bx: 200.0,
        by: 0.0
    }));
    let mut preview = Vec::new();
    w.write_road_preview(0.0, 0.0, 300.0, 0.0, &mut preview);
    assert!(!preview.is_empty());
    assert!(preview.chunks_exact(3).all(|p| p[1] <= 0.0));
    assert!(preview.chunks_exact(3).any(|p| p[1] < -40.0));
    assert!(w.apply(Command::BuildRoad {
        ax: 0.0,
        ay: 0.0,
        bx: 300.0,
        by: 0.0
    }));
    assert_eq!(w.roads.road_count(), 2);
}

#[test]
fn painted_zones_survive_curve_builds_splits_and_deletion() {
    let mut w = empty();
    assert!(w.apply(Command::PaintZone {
        ax: -40.0,
        ay: -40.0,
        bx: 240.0,
        by: 100.0,
        zone: sim::Zone::ResidentialHigh,
    }));
    let cells = w.zones.cells.clone();
    let layout = w.zones.layout();
    let version = w.zones.version;
    assert!(w.apply(curve()));
    assert!(w.apply(Command::BuildRoad {
        ax: 100.0,
        ay: -100.0,
        bx: 100.0,
        by: 100.0,
    }));
    assert!(w.apply(Command::DeleteRoad { x: 50.0, y: 37.5 }));
    assert_eq!(w.zones.cells, cells);
    assert_eq!(w.zones.layout(), layout);
    assert_eq!(w.zones.version, version);
}

#[test]
fn curve_queries_use_distance_tangents_and_reverse_lanes() {
    let mut net = RoadNetwork::default();
    let a = net.add_node(0.0, 0.0);
    let b = net.add_node(200.0, 0.0);
    assert!(net.add_curve(a, b, (100.0, 100.0), STREET_SPEED));
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
    for (s, p) in segments.chunks_exact(5).zip(preview.windows(6).step_by(3)) {
        close((s[0], s[1]), (p[0], p[1]));
        close((s[2], s[3]), (p[3], p[4]));
    }
}

#[test]
fn crossing_and_t_junction_preserve_curve_shape() {
    for end in [50.0, 100.0] {
        let mut w = empty();
        assert!(w.apply(curve()));
        assert!(w.apply(Command::BuildRoad {
            ax: 100.0,
            ay: -100.0,
            bx: 100.0,
            by: end
        }));
        let n = w.roads.nearest_node(100.0, 50.0, 0.02).unwrap();
        assert_eq!(w.roads.out_lanes(n).len(), if end == 50.0 { 3 } else { 4 });
        let len = w.roads.lane_len[0];
        close(w.roads.lane_point(0, len, 0.0), (100.0, 50.0));
        close(w.roads.lane_point(0, len / 2.0, 0.0), (45.24, 35.01));
    }
}

#[test]
fn curve_crosses_existing_straight_road() {
    let mut w = empty();
    assert!(w.apply(Command::BuildRoad {
        ax: 100.0,
        ay: -100.0,
        bx: 100.0,
        by: 100.0
    }));
    assert!(w.apply(curve()));
    let n = w.roads.nearest_node(100.0, 50.0, 0.02).unwrap();
    assert_eq!(w.roads.out_lanes(n).len(), 4);
    assert_eq!(w.roads.road_count(), 4);
}

#[test]
fn curves_cross_each_other_and_preview_marks_only_real_junctions() {
    let mut w = empty();
    assert!(w.apply(curve()));
    let mut preview = Vec::new();
    w.write_curve_preview(100.0, -100.0, 200.0, 0.0, 100.0, 100.0, &mut preview);
    assert!(!preview.is_empty());
    assert_eq!(preview.chunks_exact(3).filter(|s| s[2] == 2.0).count(), 1);
    assert!(w.apply(Command::BuildCurve {
        ax: 100.0,
        ay: -100.0,
        cx: 200.0,
        cy: 0.0,
        bx: 100.0,
        by: 100.0
    }));
    assert_eq!(w.roads.road_count(), 4);
    let split = preview.chunks_exact(3).find(|s| s[2] == 2.0).unwrap();
    let n = w.roads.nearest_node(split[0], split[1], 0.02).unwrap();
    assert_eq!(w.roads.out_lanes(n).len(), 4);
}

#[test]
fn curve_junction_turns_use_local_endpoint_tangents() {
    use sim::junction::{Junctions, Turn};
    let mut net = RoadNetwork::default();
    let a = net.add_node(0.0, 0.0);
    let b = net.add_node(200.0, 0.0);
    let c = net.add_node(300.0, -100.0);
    net.add_curve(a, b, (100.0, 100.0), STREET_SPEED);
    net.add_curve(b, c, (250.0, -50.0), STREET_SPEED);
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
        assert!(snapshot.chunks_exact(4).any(|s| s[3] >= 0.0));
        for (i, s) in snapshot
            .chunks_exact(4)
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
            by: 100.0
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
    net.add_curve(a, b, (100.0, 500.0), STREET_SPEED);
    net.add_road(a, c, STREET_SPEED);
    net.add_road(c, b, STREET_SPEED);
    net.rebuild_adjacency();
    let mut path = Vec::new();
    assert!(sim::path::Pathfinder::default().find(&net, &[], a, b, &mut path));
    assert_eq!(path, vec![2, 4]);
    assert!(net.bounds()[3] >= 250.0);
}

#[test]
fn nonuniform_parameter_does_not_change_distance_travelled() {
    let mut net = RoadNetwork::default();
    let a = net.add_node(0.0, 0.0);
    let b = net.add_node(200.0, 0.0);
    net.add_curve(a, b, (10.0, 0.0), STREET_SPEED);
    for d in 0..=200 {
        close(net.lane_point(0, d as f32, 2.0), (d as f32, -2.0));
    }
}

#[test]
fn curve_cannot_join_then_recross_the_same_road() {
    let mut w = empty();
    w.apply(Command::BuildRoad {
        ax: -100.0,
        ay: 0.0,
        bx: 300.0,
        by: 0.0,
    });
    let before = w.roads.version;
    assert!(!w.apply(Command::BuildCurve {
        ax: 0.0,
        ay: 0.0,
        cx: 100.0,
        cy: 100.0,
        bx: 200.0,
        by: -100.0
    }));
    assert_eq!(w.roads.version, before);
}

#[test]
fn a_second_connection_to_the_same_curve_cannot_silently_lose_its_crossing() {
    // The graph permits one road per node pair. These builds would create a second road
    // between the curve's start and a new split, so refuse rather than drop the split/leg.
    for (bx, by) in [(200.0, 100.0), (100.0, 100.0)] {
        let mut w = empty();
        assert!(w.apply(Command::BuildCurve {
            ax: 0.0,
            ay: 0.0,
            cx: 100.0,
            cy: 200.0,
            bx: 200.0,
            by: 0.0
        }));
        let version = w.roads.version;
        assert!(!w.apply(Command::BuildRoad {
            ax: 0.0,
            ay: 0.0,
            bx,
            by
        }));
        assert_eq!(w.roads.version, version);
    }
}

#[test]
fn curve_t_junctions_work_far_from_the_origin() {
    for offset in [0.0, 10000.0, 20000.0] {
        for k in 2..19 {
            let mut w = empty();
            assert!(w.apply(Command::BuildCurve {
                ax: offset,
                ay: offset,
                cx: offset + 100.0,
                cy: offset + 100.0,
                bx: offset + 200.0,
                by: offset
            }));
            let len = w.roads.lane_len[0];
            let p = w.roads.lane_point(0, len * k as f32 / 20.0, 0.0);
            assert!(
                w.apply(Command::BuildRoad {
                    ax: p.0,
                    ay: p.1 - 100.0,
                    bx: p.0,
                    by: p.1
                }),
                "offset {offset}, step {k}"
            );
        }
    }
}

#[test]
fn short_t_junctions_on_large_city_coordinates_are_not_duplicate_crossings() {
    let mut w = empty();
    let angle: f32 = 0.248;
    let (x, y) = (angle.cos(), angle.sin());
    assert!(w.apply(Command::BuildRoad {
        ax: 10000.0,
        ay: 10000.0,
        bx: 10000.0 + 400.0 * x,
        by: 10000.0 + 400.0 * y
    }));
    let (sx, sy) = (10000.0 + 177.7 * x, 10000.0 + 177.7 * y);
    for command in [
        Command::BuildRoad {
            ax: sx,
            ay: sy,
            bx: sx - 25.0 * y,
            by: sy + 25.0 * x,
        },
        Command::BuildRoad {
            bx: sx,
            by: sy,
            ax: sx - 25.0 * y,
            ay: sy + 25.0 * x,
        },
        Command::BuildCurve {
            ax: sx,
            ay: sy,
            cx: sx - 12.0 * y + 5.0 * x,
            cy: sy + 12.0 * x + 5.0 * y,
            bx: sx - 25.0 * y,
            by: sy + 25.0 * x,
        },
    ] {
        let mut copy = World::new(Config::default(), w.roads.clone(), Rng::new(1));
        assert!(copy.apply(command.clone()), "{command:?}");
        assert_eq!(copy.roads.road_count(), 3);
    }
}

#[test]
fn refused_curves_are_atomic() {
    let mut w = empty();
    for c in [f32::NAN, f32::INFINITY] {
        assert!(!w.apply(Command::BuildCurve {
            ax: 0.0,
            ay: 0.0,
            cx: c,
            cy: 50.0,
            bx: 200.0,
            by: 0.0
        }));
    }
    assert!(!w.apply(Command::BuildCurve {
        ax: 0.0,
        ay: 0.0,
        cx: 100.0,
        cy: 0.0,
        bx: 0.0,
        by: 0.0
    }));
    assert_eq!(w.roads.node_count(), 0);
    assert!(w.apply(curve()));
    let version = w.roads.version;
    assert!(!w.apply(curve()));
    assert_eq!(w.roads.version, version);
}
