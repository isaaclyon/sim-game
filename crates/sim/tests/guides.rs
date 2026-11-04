//! Drawing guides (ADR 0036): angle, guide-line and length snapping of clicked points.

use sim::guide::{ALL, ANGLE, GUIDES, LENGTH, angle_of, draft};
use sim::rng::Rng;
use sim::road::{RoadClass, RoadNetwork};
use sim::{Command, Config, World};

fn town(roads: &[(f32, f32, f32, f32)]) -> World {
    let mut w = World::new(Config::default(), RoadNetwork::default(), Rng::new(1));
    for &(ax, ay, bx, by) in roads {
        assert!(w.apply(Command::BuildRoad {
            ax,
            ay,
            bx,
            by,
            class: RoadClass::Street,
        }));
    }
    w
}
fn close(a: (f32, f32), b: (f32, f32)) {
    assert!((a.0 - b.0).hypot(a.1 - b.1) < 0.05, "{a:?} != {b:?}");
}

#[test]
fn angles_snap_square_to_the_road_the_start_joins() {
    // A road along a 30° bearing; a new road leaving its end almost square to it.
    let (c, s) = (0.866_025_4, 0.5);
    let w = town(&[(0.0, 0.0, 100.0 * c, 100.0 * s)]);
    let end = (100.0 * c, 100.0 * s);
    let cursor = (end.0 - 58.0 * s + 3.0, end.1 + 58.0 * c);
    let d = draft(
        &w.roads,
        &[(end.0, end.1, ALL), (cursor.0, cursor.1, ANGLE)],
    );
    let v = (d.points[1].0 - end.0, d.points[1].1 - end.1);
    assert!(
        (v.0 * c + v.1 * s).abs() < 0.01,
        "square to the road: {v:?}"
    );
    assert!((d.angle - 90.0).abs() < 0.1, "readout {}", d.angle);
    assert_eq!(d.lines.len(), 1, "the angle line is shown");
    // Without the angle flag the point stays where the cursor is.
    let free = draft(&w.roads, &[(end.0, end.1, ALL), (cursor.0, cursor.1, 0)]);
    close(free.points[1], cursor);
}

#[test]
fn empty_land_snaps_to_map_axes_and_whole_cells() {
    let w = town(&[]);
    let d = draft(&w.roads, &[(0.0, 0.0, ALL), (61.0, 2.0, ANGLE | LENGTH)]);
    close(d.points[1], (64.0, 0.0));
    assert!((d.length - 64.0).abs() < 0.01);
    assert!(d.angle.is_nan(), "no road to measure from");
    let off = draft(&w.roads, &[(0.0, 0.0, ALL), (61.0, 30.0, ANGLE | LENGTH)]);
    assert!(
        (off.points[1].1 / off.points[1].0 - 30.0 / 61.0).abs() < 0.01,
        "far from any step, the angle is kept"
    );
}

#[test]
fn guide_lines_align_new_ends_with_nearby_road_ends() {
    // A dead end at (0, 100). Drawing from (200, 0) towards (3, 40) lands on its extension,
    // the line x = 0.
    let w = town(&[(0.0, 200.0, 0.0, 100.0)]);
    let d = draft(&w.roads, &[(200.0, 0.0, ALL), (3.0, 40.0, GUIDES)]);
    close(d.points[1], (0.0, 40.0));
    assert_eq!(d.lines.len(), 1);
    close((d.lines[0][0], d.lines[0][1]), (0.0, 100.0));
    // Angle and guide together: the axis from the start meets the extension at (0, 0).
    let both = draft(&w.roads, &[(200.0, 0.0, ALL), (4.0, 5.0, ANGLE | GUIDES)]);
    close(both.points[1], (0.0, 0.0));
    assert_eq!(both.lines.len(), 2);
}

#[test]
fn junctions_win_and_angle_lines_cross_roads_where_they_meet() {
    let w = town(&[(0.0, 0.0, 200.0, 0.0), (200.0, 0.0, 200.0, 200.0)]);
    // Near the junction: the junction, no guides.
    let d = draft(&w.roads, &[(100.0, 120.0, ALL), (196.0, 8.0, ALL)]);
    close(d.points[1], (200.0, 0.0));
    assert!(d.lines.is_empty());
    // Heading straight down from (100, 120) onto the first road, a little off: the point lands
    // where the vertical meets the road, not at the nearest point to the cursor.
    let hit = draft(&w.roads, &[(100.0, 120.0, ALL), (104.0, 10.0, ANGLE)]);
    close(hit.points[1], (100.0, 0.0));
}

#[test]
fn the_world_previews_and_builds_the_same_guided_road() {
    let mut w = town(&[(0.0, 0.0, 100.0, 0.0)]);
    let raw = [100.0, 0.0, ALL as f32, 103.0, 61.0, ALL as f32];
    let mut out = Vec::new();
    w.write_road_draft(&raw, true, &mut out);
    let (n, m, k) = (out[0] as usize, out[1] as usize, out[4] as usize);
    assert_eq!(n, 2);
    let end = (out[5 + 2], out[5 + 3]);
    close(end, (100.0, 64.0));
    let stops = &out[5 + 2 * n + 4 * m + 6 * k..];
    close((stops[stops.len() - 3], stops[stops.len() - 2]), end);
    let cmd = w.draft_command(&raw, RoadClass::Street).expect("buildable");
    assert!(w.apply(cmd));
    assert!(w.roads.nearest_node(100.0, 64.0, 0.05).is_some());
    // Guide-only answers for a curve's control point plan no road.
    w.write_road_draft(&raw[..6], false, &mut out);
    assert_eq!(out.len(), 5 + 4 + 4 * out[1] as usize + 6 * out[4] as usize);
}

#[test]
fn a_square_end_on_another_road_keeps_its_angle() {
    // From a junction to a parallel road 104 m away, aimed 3° off square, as in a town block.
    for tilt in [0.0, 12.0] {
        let w = town(&[
            (0.0, -200.0, 0.0, 200.0),
            (-100.0, 0.0, 0.0, 0.0),
            // Drawn in two pieces along one line: no junction is left where they meet.
            (104.0, -200.0, 104.0 + tilt * 0.525, 10.0),
            (104.0 + tilt * 0.525, 10.0, 104.0 + tilt, 200.0),
        ]);
        let cursor = (104.0 + tilt / 2.0, 5.5);
        let d = draft(&w.roads, &[(0.0, 0.0, ALL), (cursor.0, cursor.1, ALL)]);
        let end = d.points[1];
        assert!(
            end.1.abs() < 0.05,
            "tilt {tilt}: square to the start, got {end:?}"
        );
        assert!(
            (d.angle - 90.0).abs() < 0.1,
            "tilt {tilt}: readout {}",
            d.angle
        );
    }
}

#[test]
fn the_readout_measures_from_the_road_the_snap_lined_up_with() {
    // A junction of an east road and a road 130° clockwise from it. A new road square to the
    // east road is 40° from the other, but the readout says 90°.
    let (c, s) = (-0.642_787_6_f32, -0.766_044_4_f32);
    let w = town(&[(0.0, 0.0, 100.0, 0.0), (0.0, 0.0, 100.0 * c, 100.0 * s)]);
    let d = draft(&w.roads, &[(0.0, 0.0, ALL), (2.0, -80.0, ANGLE)]);
    close(d.points[1], (0.0, -80.0));
    assert!((d.angle - 90.0).abs() < 0.1, "readout {}", d.angle);
}

#[test]
fn angle_marks_sit_where_the_road_joins_roads_at_either_end() {
    // From the middle of one street square across to a parallel one: a T at both ends.
    let w = town(&[(0.0, -100.0, 0.0, 100.0), (80.0, -100.0, 80.0, 100.0)]);
    let d = draft(&w.roads, &[(0.0, 0.0, ALL), (78.0, 2.0, ALL)]);
    close(d.points[1], (80.0, 0.0));
    let (start, end) = (d.start_mark.expect("start"), d.end_mark.expect("end"));
    close((start[0], start[1]), (0.0, 0.0));
    close((end[0], end[1]), (80.0, 0.0));
    for m in [start, end] {
        assert!((angle_of(m) - 90.0).abs() < 0.01, "{m:?}");
    }
    // Each mark's new-road direction points along the road, away from its junction.
    close((start[4], start[5]), (1.0, 0.0));
    close((end[4], end[5]), (-1.0, 0.0));
    // A start on open ground joins nothing: no mark there.
    let free = draft(&w.roads, &[(40.0, 60.0, 0), (40.0, 90.0, 0)]);
    assert!(free.start_mark.is_none() && free.end_mark.is_none());
}
