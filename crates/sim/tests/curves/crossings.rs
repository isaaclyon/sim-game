//! Curves crossing and joining other roads.

use sim::rng::Rng;
use sim::road::RoadClass;
use sim::{Command, Config, World};

use super::{close, curve, empty};

#[test]
fn crossing_and_t_junction_preserve_curve_shape() {
    for end in [50.0, 100.0] {
        let mut w = empty();
        assert!(w.apply(curve()));
        assert!(w.apply(Command::BuildRoad {
            ax: 100.0,
            ay: -100.0,
            bx: 100.0,
            by: end,
            class: RoadClass::Street
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
        by: 100.0,
        class: RoadClass::Street
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
    assert_eq!(
        preview
            .as_chunks::<3>()
            .0
            .iter()
            .filter(|s| s[2] == 2.0)
            .count(),
        1
    );
    assert!(w.apply(Command::BuildCurve {
        ax: 100.0,
        ay: -100.0,
        cx: 200.0,
        cy: 0.0,
        bx: 100.0,
        by: 100.0,
        class: RoadClass::Street
    }));
    assert_eq!(w.roads.road_count(), 4);
    let split = preview
        .as_chunks::<3>()
        .0
        .iter()
        .find(|s| s[2] == 2.0)
        .unwrap();
    let n = w.roads.nearest_node(split[0], split[1], 0.02).unwrap();
    assert_eq!(w.roads.out_lanes(n).len(), 4);
}

#[test]
fn curve_cannot_join_then_recross_the_same_road() {
    let mut w = empty();
    w.apply(Command::BuildRoad {
        ax: -100.0,
        ay: 0.0,
        bx: 300.0,
        by: 0.0,
        class: RoadClass::Street,
    });
    let before = w.roads.version;
    assert!(!w.apply(Command::BuildCurve {
        ax: 0.0,
        ay: 0.0,
        cx: 100.0,
        cy: 100.0,
        bx: 200.0,
        by: -100.0,
        class: RoadClass::Street
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
            by: 0.0,
            class: RoadClass::Street
        }));
        let version = w.roads.version;
        assert!(!w.apply(Command::BuildRoad {
            ax: 0.0,
            ay: 0.0,
            bx,
            by,
            class: RoadClass::Street
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
                by: offset,
                class: RoadClass::Street
            }));
            let len = w.roads.lane_len[0];
            let p = w.roads.lane_point(0, len * k as f32 / 20.0, 0.0);
            assert!(
                w.apply(Command::BuildRoad {
                    ax: p.0,
                    ay: p.1 - 100.0,
                    bx: p.0,
                    by: p.1,
                    class: RoadClass::Street
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
        by: 10000.0 + 400.0 * y,
        class: RoadClass::Street
    }));
    let (sx, sy) = (10000.0 + 177.7 * x, 10000.0 + 177.7 * y);
    for command in [
        Command::BuildRoad {
            ax: sx,
            ay: sy,
            bx: sx - 25.0 * y,
            by: sy + 25.0 * x,
            class: RoadClass::Street,
        },
        Command::BuildRoad {
            bx: sx,
            by: sy,
            ax: sx - 25.0 * y,
            ay: sy + 25.0 * x,
            class: RoadClass::Street,
        },
        Command::BuildCurve {
            ax: sx,
            ay: sy,
            cx: sx - 12.0 * y + 5.0 * x,
            cy: sy + 12.0 * x + 5.0 * y,
            bx: sx - 25.0 * y,
            by: sy + 25.0 * x,
            class: RoadClass::Street,
        },
    ] {
        let mut copy = World::new(Config::default(), w.roads.clone(), Rng::new(1));
        assert!(copy.apply(command.clone()), "{command:?}");
        assert_eq!(copy.roads.road_count(), 3);
    }
}
