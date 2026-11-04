//! Straight joins (ADR 0040): two straight roads of one class meeting end to end in a line
//! become one road, so no hidden junction is left to pull new roads off their line.
use sim::rng::Rng;
use sim::road::{RoadClass, RoadNetwork};
use sim::snap::{Join, snap_end};
use sim::{Command, Config, World, Zone};

fn road(w: &mut World, ax: f32, ay: f32, bx: f32, by: f32, class: RoadClass) -> bool {
    w.apply(Command::BuildRoad {
        ax,
        ay,
        bx,
        by,
        class,
    })
}

fn empty() -> World {
    World::new(
        Config {
            ignore_demand: true,
            ..Config::default()
        },
        RoadNetwork::default(),
        Rng::new(1),
    )
}

#[test]
fn a_street_drawn_in_pieces_is_one_road() {
    let mut w = empty();
    for x in [0.0, 100.0, 200.0] {
        assert!(road(&mut w, x, 0.0, x + 100.0, 0.0, RoadClass::Street));
    }
    assert_eq!(w.roads.road_count(), 1);
    // Where the pieces met there is nothing left to snap to but the road itself.
    let stop = snap_end(&w.roads, 102.0, 4.0);
    assert!(matches!(stop.join, Join::Split(_)), "{:?}", stop.join);
    // The joined road still carries traffic end to end.
    let lane = w.roads.active_roads().next().unwrap() * 2;
    assert!((w.roads.lane_len[lane as usize] - 300.0).abs() < 0.01);
}

#[test]
fn bends_mixed_classes_and_curves_keep_their_junction() {
    let mut w = empty();
    assert!(road(&mut w, 0.0, 0.0, 100.0, 0.0, RoadClass::Street));
    assert!(road(&mut w, 100.0, 0.0, 200.0, 10.0, RoadClass::Street));
    assert!(road(&mut w, 200.0, 10.0, 300.0, 20.0, RoadClass::Main));
    assert!(w.apply(Command::BuildCurve {
        ax: 300.0,
        ay: 20.0,
        cx: 400.0,
        cy: 30.0,
        bx: 450.0,
        by: 100.0,
        class: RoadClass::Main,
    }));
    assert_eq!(w.roads.road_count(), 4);
}

#[test]
fn bulldozing_a_side_street_joins_the_through_road_and_keeps_its_buildings() {
    let mut w = empty();
    // A block whose south side is crossed by a side street leaving southwards.
    for &(ax, ay, bx, by) in &[
        (0.0, 0.0, 160.0, 0.0),
        (160.0, 0.0, 160.0, 160.0),
        (160.0, 160.0, 0.0, 160.0),
        (0.0, 160.0, 0.0, 0.0),
        (80.0, 0.0, 80.0, -100.0),
    ] {
        assert!(road(&mut w, ax, ay, bx, by, RoadClass::Street));
    }
    assert_eq!(
        w.roads.road_count(),
        6,
        "the side street splits the south side"
    );
    assert!(w.apply(Command::FillBlock {
        x: 80.0,
        y: 80.0,
        zone: Zone::ResidentialLow,
    }));
    for _ in 0..1200 {
        w.tick();
    }
    let built: Vec<u32> = w.buildings.active_buildings().collect();
    assert!(built.len() > 4);
    // Zoning may have added alleys, so count the change.
    let roads = w.roads.road_count();
    assert!(w.apply(Command::DeleteRoad { x: 80.0, y: -60.0 }));
    assert_eq!(
        w.roads.road_count(),
        roads - 2,
        "the south side is one road again"
    );
    assert!(matches!(snap_end(&w.roads, 80.0, 3.0).join, Join::Split(_)));
    let after: Vec<u32> = w.buildings.active_buildings().collect();
    assert_eq!(after, built, "buildings along the joined road stay");
    for b in after {
        let lot = w.buildings.lot[b as usize] as usize;
        assert!(w.lots.active[lot]);
        assert!(
            w.roads.is_road_active(w.lots.road[lot]),
            "building {b} fronts a live road"
        );
    }
}
