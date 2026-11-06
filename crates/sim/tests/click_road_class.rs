//! Clicking a road to change its class (ADR 0050): the road nearest the click takes the class
//! in place, under ADR 0048's rules for cost, lanes and junctions.
use sim::rng::Rng;
use sim::road::{MAIN_SPEED, RoadClass, RoadNetwork, STREET_SPEED};
use sim::world::START_FUNDS;
use sim::{Command, Config, World};

fn click(w: &mut World, x: f32, y: f32, class: RoadClass) -> bool {
    w.apply(Command::SetRoadClass { x, y, class })
}

/// A world on `segments`, each `(ax, ay, bx, by, class)` with nodes shared where ends meet.
fn world(segments: &[(f32, f32, f32, f32, RoadClass)]) -> World {
    let mut net = RoadNetwork::default();
    let node = |net: &mut RoadNetwork, x: f32, y: f32| {
        net.nearest_node(x, y, 0.5)
            .unwrap_or_else(|| net.add_node(x, y))
    };
    for &(ax, ay, bx, by, class) in segments {
        let (a, b) = (node(&mut net, ax, ay), node(&mut net, bx, by));
        assert!(net.add_road(a, b, class));
    }
    net.rebuild_adjacency();
    World::new(Config::default(), net, Rng::new(1))
}

fn classes(w: &World) -> Vec<RoadClass> {
    w.roads
        .active_roads()
        .map(|r| w.roads.road_class(r))
        .collect()
}

fn preview(w: &World, x: f32, y: f32, class: RoadClass) -> Vec<f32> {
    let mut out = Vec::new();
    w.write_class_preview(x, y, class, &mut out);
    out
}

#[test]
fn clicking_a_street_with_main_upgrades_it_in_place_for_the_difference() {
    let mut w = world(&[(0.0, 0.0, 200.0, 0.0, RoadClass::Street)]);
    // $25/m for main less the $10/m the street already cost.
    assert_eq!(w.class_cost(100.0, 6.0, RoadClass::Main), Some(3_000));
    assert_eq!(
        preview(&w, 100.0, 6.0, RoadClass::Main),
        [0.0, 0.0, 200.0, 0.0],
        "the preview is the road's centreline, as for the bulldozer"
    );
    let (lanes, nodes) = (w.roads.lane_count(), w.roads.node_count());

    assert!(click(&mut w, 100.0, 6.0, RoadClass::Main));
    assert_eq!(classes(&w), [RoadClass::Main]);
    assert_eq!(w.roads.road_count(), 1, "same road ID");
    assert_eq!(w.roads.road_ends(0), (0, 1), "same ends");
    assert_eq!((w.roads.lane_count(), w.roads.node_count()), (lanes, nodes));
    assert_eq!(w.roads.lane_speed[..2], [MAIN_SPEED, MAIN_SPEED]);
    assert_eq!(w.stats().money.balance, START_FUNDS - 3_000);
    assert_eq!(w.stats().money.upkeep, 50, "main road upkeep");
}

#[test]
fn clicking_a_road_with_its_own_class_or_empty_land_changes_nothing() {
    let mut w = world(&[(0.0, 0.0, 100.0, 0.0, RoadClass::Main)]);
    let hash = w.state_hash();
    assert_eq!(w.class_cost(50.0, 0.0, RoadClass::Main), None);
    assert!(preview(&w, 50.0, 0.0, RoadClass::Main).is_empty());
    assert!(!click(&mut w, 50.0, 0.0, RoadClass::Main));
    assert_eq!(w.class_cost(50.0, 40.0, RoadClass::Street), None);
    assert!(preview(&w, 50.0, 40.0, RoadClass::Street).is_empty());
    assert!(
        !click(&mut w, 50.0, 40.0, RoadClass::Street),
        "out of reach"
    );
    assert!(!click(&mut w, f32::NAN, 0.0, RoadClass::Street));
    assert_eq!(w.state_hash(), hash);
}

#[test]
fn downgrading_is_free_and_alleys_become_streets_for_the_difference() {
    let mut w = world(&[
        (0.0, 0.0, 200.0, 0.0, RoadClass::Main),
        (0.0, 100.0, 100.0, 100.0, RoadClass::Alley),
    ]);
    assert_eq!(w.class_cost(100.0, 0.0, RoadClass::Street), Some(0));
    assert!(click(&mut w, 100.0, 0.0, RoadClass::Street));
    assert_eq!(w.roads.lane_speed[..2], [STREET_SPEED, STREET_SPEED]);
    assert_eq!(w.stats().money.balance, START_FUNDS, "no refund, no charge");
    assert_eq!(w.class_cost(50.0, 100.0, RoadClass::Street), Some(500));
    assert!(click(&mut w, 50.0, 100.0, RoadClass::Street));
    assert_eq!(classes(&w), [RoadClass::Street, RoadClass::Street]);
    assert_eq!(w.stats().money.balance, START_FUNDS - 500);
}

#[test]
fn a_click_the_city_cannot_afford_is_refused() {
    let mut w = world(&[(0.0, 0.0, 4_000.0, 0.0, RoadClass::Street)]);
    let hash = w.state_hash();
    // 4 km at $15/m is $60,000, more than a fresh city has.
    assert_eq!(w.class_cost(2_000.0, 0.0, RoadClass::Main), Some(60_000));
    assert!(!w.can_afford(60_000));
    assert!(!click(&mut w, 2_000.0, 0.0, RoadClass::Main));
    assert_eq!(classes(&w), [RoadClass::Street]);
    assert_eq!(w.state_hash(), hash);
}

#[test]
fn a_main_road_arm_turns_a_crossing_into_signals() {
    let s = RoadClass::Street;
    let mut w = world(&[
        (0.0, 0.0, 100.0, 0.0, s),
        (100.0, 0.0, 200.0, 0.0, s),
        (100.0, -100.0, 100.0, 0.0, s),
        (100.0, 0.0, 100.0, 100.0, s),
    ]);
    let mut signals = Vec::new();
    w.write_signal_snapshot(&mut signals);
    assert!(signals.is_empty(), "streets meet with right of way");
    assert!(click(&mut w, 50.0, 0.0, RoadClass::Main));
    w.write_signal_snapshot(&mut signals);
    assert!(!signals.is_empty(), "junction controls were rebuilt");
}

#[test]
fn the_nearest_road_changes_and_its_neighbours_keep_their_class() {
    let s = RoadClass::Street;
    let mut w = world(&[
        (0.0, 0.0, 100.0, 0.0, s),
        (100.0, 0.0, 200.0, 0.0, s),
        (100.0, 0.0, 100.0, 100.0, s),
    ]);
    assert!(click(&mut w, 104.0, 60.0, RoadClass::Main));
    assert_eq!(classes(&w), [s, s, RoadClass::Main]);
}

#[test]
fn cars_keep_driving_when_their_road_is_clicked_to_main() {
    let mut w = World::generate(Config::default(), 8, 8);
    w.apply(Command::SpawnAgents { count: 500 });
    for _ in 0..600 {
        w.tick();
    }
    assert!(w.stats().driving > 50, "cars are on the roads");
    let streets: Vec<_> = w
        .roads
        .active_roads()
        .filter(|&r| w.roads.road_class(r) == RoadClass::Street)
        .collect();
    let (lanes, roads) = (w.roads.lane_count(), w.roads.road_count());
    let despawned = w.stats().despawned_by_edit;
    let mut upgraded = 0;
    for r in streets {
        let (a, b) = w.roads.road_ends(r);
        let (a, b) = (a as usize, b as usize);
        let x = (w.roads.node_x[a] + w.roads.node_x[b]) / 2.0;
        let y = (w.roads.node_y[a] + w.roads.node_y[b]) / 2.0;
        if click(&mut w, x, y, RoadClass::Main) {
            upgraded += 1;
            assert_eq!(w.roads.road_class(r), RoadClass::Main, "road {r} in place");
        }
    }
    assert!(upgraded > 10, "upgraded {upgraded} streets");
    assert_eq!(w.roads.lane_count(), lanes, "no road was rebuilt");
    assert_eq!(w.roads.road_count(), roads, "no road was added");
    assert_eq!(w.stats().despawned_by_edit, despawned, "no car was removed");
    for _ in 0..600 {
        w.tick();
    }
}

#[test]
fn a_click_replays_like_any_command() {
    let mut a = world(&[(0.0, 0.0, 200.0, 0.0, RoadClass::Street)]);
    let mut b = world(&[(0.0, 0.0, 200.0, 0.0, RoadClass::Street)]);
    assert!(click(&mut a, 100.0, 0.0, RoadClass::Main));
    assert!(click(&mut b, 100.0, 0.0, RoadClass::Main));
    assert_eq!(a.state_hash(), b.state_hash());
    let mut c = World::load(&a.save().unwrap()).unwrap();
    assert_eq!(c.state_hash(), a.state_hash());
    assert!(click(&mut c, 100.0, 0.0, RoadClass::Street));
}
