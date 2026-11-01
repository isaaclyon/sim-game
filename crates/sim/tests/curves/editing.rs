//! Bulldozing, rebuilding, zoning and refused builds around curves.

use sim::Command;
use sim::road::RoadClass;

use super::{close, curve, empty};

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
    for (p, s) in preview
        .windows(4)
        .step_by(2)
        .zip(segments.as_chunks::<5>().0.iter())
    {
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
        by: -500.0,
        class: RoadClass::Street
    }));
    assert!(w.apply(curve()));
    assert!(w.apply(Command::DeleteRoad {
        x: -400.0,
        y: -500.0
    }));
    let mut segments = Vec::new();
    w.write_road_segments(&mut segments);
    assert!(segments.len() > 5);
    assert!(
        segments
            .as_chunks::<5>()
            .0
            .iter()
            .all(|s| s[0] >= 0.0 && s[1] >= 0.0)
    );
    let mut preview = Vec::new();
    w.write_snap_preview(100.0, 50.0, &mut preview);
    assert_eq!(preview, [100.0, 50.0, 2.0]);
    assert!(w.apply(Command::BuildRoad {
        ax: 100.0,
        ay: -100.0,
        bx: 100.0,
        by: 50.0,
        class: RoadClass::Street
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
        by: 0.0,
        class: RoadClass::Street
    }));
    let mut preview = Vec::new();
    w.write_road_preview(0.0, 0.0, 300.0, 0.0, &mut preview);
    assert!(!preview.is_empty());
    assert!(preview.as_chunks::<3>().0.iter().all(|p| p[1] <= 0.0));
    assert!(preview.as_chunks::<3>().0.iter().any(|p| p[1] < -40.0));
    assert!(w.apply(Command::BuildRoad {
        ax: 0.0,
        ay: 0.0,
        bx: 300.0,
        by: 0.0,
        class: RoadClass::Street
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
        class: RoadClass::Street
    }));
    assert!(w.apply(Command::DeleteRoad { x: 50.0, y: 37.5 }));
    assert_eq!(w.zones.cells, cells);
    assert_eq!(w.zones.layout(), layout);
    assert_eq!(w.zones.version, version);
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
            by: 0.0,
            class: RoadClass::Street
        }));
    }
    assert!(!w.apply(Command::BuildCurve {
        ax: 0.0,
        ay: 0.0,
        cx: 100.0,
        cy: 0.0,
        bx: 0.0,
        by: 0.0,
        class: RoadClass::Street
    }));
    assert_eq!(w.roads.node_count(), 0);
    assert!(w.apply(curve()));
    let version = w.roads.version;
    assert!(!w.apply(curve()));
    assert_eq!(w.roads.version, version);
}
