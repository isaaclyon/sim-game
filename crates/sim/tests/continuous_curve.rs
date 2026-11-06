use sim::guide::{ANGLE, LENGTH, draft};
use sim::rng::Rng;
use sim::road::{RoadClass, RoadNetwork};
use sim::{Command, Config, World};

const CONTINUE: u8 = 8;

fn world() -> World {
    World::new(Config::default(), RoadNetwork::default(), Rng::new(1))
}
fn curve(w: &mut World, a: (f32, f32), c: (f32, f32), b: (f32, f32)) {
    assert!(w.apply(Command::BuildCurve {
        ax: a.0,
        ay: a.1,
        cx: c.0,
        cy: c.1,
        bx: b.0,
        by: b.1,
        class: RoadClass::Street
    }));
}
fn road(w: &mut World, a: (f32, f32), b: (f32, f32)) {
    assert!(w.apply(Command::BuildRoad {
        ax: a.0,
        ay: a.1,
        bx: b.0,
        by: b.1,
        class: RoadClass::Street
    }));
}
fn close(a: (f32, f32), b: (f32, f32)) {
    assert!((a.0 - b.0).hypot(a.1 - b.1) < 0.05, "{a:?} != {b:?}");
}

#[test]
fn five_pieces_follow_each_previous_end_tangent() {
    let mut w = world();
    let mut control = (50.0, 0.0);
    let mut a = (100.0, 50.0);
    curve(&mut w, (0.0, 0.0), control, a);
    for end in [
        (160.0, 120.0),
        (210.0, 200.0),
        (250.0, 290.0),
        (280.0, 390.0),
        (300.0, 500.0),
    ] {
        let raw = [
            a.0,
            a.1,
            CONTINUE as f32,
            end.0,
            end.1,
            0.0,
            control.0,
            control.1,
            0.0,
        ];
        let d = draft(
            &w.roads,
            &[
                (a.0, a.1, CONTINUE),
                (end.0, end.1, 0),
                (control.0, control.1, 0),
            ],
        );
        assert_eq!(d.points.len(), 3);
        let tangent = (a.0 - control.0, a.1 - control.1);
        let len = tangent.0.hypot(tangent.1);
        let t = (tangent.0 / len, tangent.1 / len);
        let projection = (end.0 - a.0) * t.0 + (end.1 - a.1) * t.1;
        close(
            d.points[1],
            (a.0 + t.0 * projection / 2.0, a.1 + t.1 * projection / 2.0),
        );
        assert!(w.apply(w.draft_command(&raw, RoadClass::Street).unwrap()));
        control = d.points[1];
        a = end;
    }
}

#[test]
fn invalid_sources_and_backwards_are_refused() {
    let mut w = world();
    curve(&mut w, (0.0, 0.0), (50.0, 0.0), (100.0, 50.0));
    for raw in [
        [100.0, 50.0, 8.0, 40.0, 0.0, 0.0, 50.0, 0.0, 0.0],
        [400.0, 400.0, 8.0, 500.0, 500.0, 0.0, 350.0, 350.0, 0.0],
        [100.0, 50.0, 8.0, 160.0, 120.0, 0.0, 50.0, 50.0, 0.0],
        [100.0, 50.0, 8.0, 160.0, 120.0, 0.0, 100.0, 50.0, 0.0],
        [100.0, 50.0, 8.0, 160.0, 120.0, 0.0, 150.0, 100.0, 0.0],
        [100.0, 50.0, 8.0, 160.0, 120.0, 0.0, f32::NAN, 0.0, 0.0],
    ] {
        assert!(w.draft_command(&raw, RoadClass::Street).is_none());
        assert!(w.draft_cost(&raw, RoadClass::Street).is_none());
        let mut out = Vec::new();
        w.write_road_draft(&raw, true, RoadClass::Street, &mut out);
        assert_eq!(out.len(), 5);
        assert_eq!(out[0], 0.0);
    }
    assert!(
        w.draft_command(&[100.0, 50.0, 8.0, 160.0, 120.0, 0.0], RoadClass::Street)
            .is_none()
    );
}

#[test]
fn preview_cost_and_build_use_same_snapped_curve() {
    let mut w = world();
    curve(&mut w, (0.0, 0.0), (50.0, 0.0), (100.0, 50.0));
    let raw = [
        102.0,
        51.0,
        8.0,
        163.0,
        109.0,
        (ANGLE | LENGTH) as f32,
        50.0,
        0.0,
        0.0,
    ];
    let mut out = Vec::new();
    w.write_road_draft(&raw, true, RoadClass::Street, &mut out);
    assert_eq!(out[0], 3.0);
    let cmd = w.draft_command(&raw, RoadClass::Street).unwrap();
    let Command::BuildCurve {
        ax,
        ay,
        cx,
        cy,
        bx,
        by,
        ..
    } = cmd
    else {
        panic!("not a curve")
    };
    assert_eq!(&out[5..11], &[ax, ay, cx, cy, bx, by]);
    close(
        (cx - ax, cy - ay),
        ((bx - ax + by - ay) / 4.0, (bx - ax + by - ay) / 4.0),
    );
    let cost = w.draft_cost(&raw, RoadClass::Street).unwrap();
    let before = w.money().balance;
    assert!(w.apply(cmd));
    assert_eq!(before - w.money().balance, cost);
    assert!(w.roads.nearest_node(bx, by, 0.05).is_some());
}

#[test]
fn split_and_newer_junction_road_preserve_source_tangent() {
    let mut w = world();
    curve(&mut w, (0.0, 0.0), (50.0, 0.0), (100.0, 50.0));
    road(&mut w, (50.0, -100.0), (50.0, 100.0));
    road(&mut w, (100.0, 50.0), (100.0, 150.0));
    let d = draft(
        &w.roads,
        &[(100.0, 50.0, CONTINUE), (160.0, 120.0, 0), (50.0, 0.0, 0)],
    );
    assert_eq!(d.points.len(), 3);
    close(d.points[1], (132.5, 82.5));
    let raw = [100.0, 50.0, 8.0, 160.0, 120.0, 0.0, 50.0, 0.0, 0.0];
    assert!(w.apply(w.draft_command(&raw, RoadClass::Street).unwrap()));
}

#[test]
fn nearby_junctions_snap_before_the_control_is_derived() {
    let mut w = world();
    curve(&mut w, (0.0, 0.0), (50.0, 0.0), (100.0, 50.0));
    road(&mut w, (160.0, 120.0), (220.0, 120.0));
    let d = draft(
        &w.roads,
        &[(103.0, 52.0, CONTINUE), (156.0, 116.0, 0), (50.0, 0.0, 0)],
    );
    assert_eq!(d.points.len(), 3);
    close(d.points[0], (100.0, 50.0));
    close(d.points[1], (132.5, 82.5));
    close(d.points[2], (160.0, 120.0));
}

#[test]
fn stale_source_after_deletion_is_refused_even_if_another_road_meets_start() {
    let mut w = world();
    curve(&mut w, (0.0, 0.0), (50.0, 0.0), (100.0, 50.0));
    road(&mut w, (100.0, 50.0), (100.0, 150.0));
    assert!(w.apply(Command::DeleteRoad { x: 50.0, y: 12.5 }));
    let raw = [100.0, 50.0, 8.0, 160.0, 120.0, 0.0, 50.0, 0.0, 0.0];
    assert!(w.draft_command(&raw, RoadClass::Street).is_none());
}
