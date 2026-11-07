//! Road upgrades (ADR 0048): drawing a road of another class over existing roads changes
//! their class in place, and pays the difference when the new class costs more.
use sim::rng::Rng;
use sim::road::{MAIN_SPEED, RoadClass, RoadNetwork, STREET_SPEED};
use sim::world::START_FUNDS;
use sim::{Command, Config, World};

fn road(w: &mut World, ax: f32, ay: f32, bx: f32, by: f32, class: RoadClass) -> bool {
    w.apply(Command::BuildRoad {
        ax,
        ay,
        bx,
        by,
        class,
    })
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

/// Raw `[x, y, snap]` draft points with every snap off (ADR 0038).
fn raw(points: &[(f32, f32)]) -> Vec<f32> {
    points.iter().flat_map(|&(x, y)| [x, y, 0.0]).collect()
}

/// The `stops` part of `write_road_draft`'s answer.
fn preview_stops(w: &World, points: &[(f32, f32)], class: RoadClass) -> Vec<f32> {
    let mut out = Vec::new();
    w.write_road_draft(&raw(points), true, class, &mut out);
    let (n, m, k, r) = (
        out[0] as usize,
        out[1] as usize,
        out[4] as usize,
        out[5] as usize,
    );
    out[6 + 2 * n + 4 * m + 6 * k + r..].to_vec()
}

/// The `reclass` part of `write_road_draft`'s answer: `[x, y]` centreline samples per road
/// the draft would change to the drawn class.
fn preview_reclass(w: &World, points: &[(f32, f32)], class: RoadClass) -> Vec<Vec<(f32, f32)>> {
    let mut out = Vec::new();
    w.write_road_draft(&raw(points), true, class, &mut out);
    let (n, m, k, r) = (
        out[0] as usize,
        out[1] as usize,
        out[4] as usize,
        out[5] as usize,
    );
    let at = 6 + 2 * n + 4 * m + 6 * k;
    let mut section = &out[at..at + r];
    let mut roads = Vec::new();
    while let [count, rest @ ..] = section {
        let (samples, next) = rest.split_at(2 * *count as usize);
        roads.push(samples.chunks(2).map(|p| (p[0], p[1])).collect());
        section = next;
    }
    roads
}

#[test]
fn the_preview_lists_the_roads_a_draft_would_change() {
    let w = world(&[(0.0, 0.0, 200.0, 0.0, RoadClass::Main)]);
    // A street drawn along the main road would quietly turn it into a street.
    let along = preview_reclass(&w, &[(0.0, 0.0), (200.0, 0.0)], RoadClass::Street);
    assert_eq!(along, [vec![(0.0, 0.0), (200.0, 0.0)]]);
    // Crossing it adds a road and changes no class; open ground changes nothing either.
    assert!(preview_reclass(&w, &[(100.0, -80.0), (100.0, 80.0)], RoadClass::Street).is_empty());
    assert!(preview_reclass(&w, &[(0.0, 300.0), (200.0, 300.0)], RoadClass::Street).is_empty());
    // A main road drawn on past it upgrades nothing, since the road is already main.
    assert!(preview_reclass(&w, &[(0.0, 0.0), (300.0, 0.0)], RoadClass::Main).is_empty());
}

#[test]
fn drawing_a_main_road_over_a_street_upgrades_it_for_the_difference() {
    let mut w = world(&[(0.0, 0.0, 200.0, 0.0, RoadClass::Street)]);
    let ends = [(0.0, 0.0), (200.0, 0.0)];
    assert!(preview_stops(&w, &ends, RoadClass::Street).is_empty());
    assert!(!preview_stops(&w, &ends, RoadClass::Main).is_empty());
    assert_eq!(w.draft_cost(&raw(&ends), RoadClass::Street), None);
    // $25/m for main less the $10/m the street already cost.
    assert_eq!(w.draft_cost(&raw(&ends), RoadClass::Main), Some(3_000));

    assert!(!road(&mut w, 0.0, 0.0, 200.0, 0.0, RoadClass::Street));
    assert!(
        !road(&mut w, 60.0, 0.0, 200.0, 0.0, RoadClass::Main),
        "only whole roads change class"
    );
    assert!(road(&mut w, 0.0, 0.0, 200.0, 0.0, RoadClass::Main));
    assert_eq!(classes(&w), [RoadClass::Main]);
    assert_eq!(w.roads.road_count(), 1, "upgraded in place");
    assert_eq!(w.roads.lane_speed[..2], [MAIN_SPEED, MAIN_SPEED]);
    assert_eq!(w.stats().money.balance, START_FUNDS - 3_000);
    assert_eq!(w.stats().money.upkeep, 50, "main road upkeep");
    let mut segments = Vec::new();
    w.write_road_segments(&mut segments);
    assert_eq!(segments[4], 1.0, "the snapshot draws it as a main road");
}

#[test]
fn downgrading_is_free_and_lowers_upkeep() {
    let mut w = world(&[(0.0, 0.0, 200.0, 0.0, RoadClass::Main)]);
    assert_eq!(
        w.draft_cost(&raw(&[(200.0, 0.0), (0.0, 0.0)]), RoadClass::Street),
        Some(0)
    );
    assert!(road(&mut w, 200.0, 0.0, 0.0, 0.0, RoadClass::Street));
    assert_eq!(classes(&w), [RoadClass::Street]);
    assert_eq!(w.roads.lane_speed[..2], [STREET_SPEED, STREET_SPEED]);
    assert_eq!(w.stats().money.balance, START_FUNDS);
    assert_eq!(w.stats().money.upkeep, 20);
}

#[test]
fn a_drag_through_a_junction_upgrades_every_road_it_follows() {
    let s = RoadClass::Street;
    let mut w = world(&[
        (0.0, 0.0, 100.0, 0.0, s),
        (100.0, 0.0, 200.0, 0.0, s),
        (100.0, 0.0, 100.0, 100.0, s),
    ]);
    assert!(road(&mut w, 0.0, 0.0, 200.0, 0.0, RoadClass::Main));
    assert_eq!(classes(&w), [RoadClass::Main, RoadClass::Main, s]);
}

#[test]
fn a_road_that_follows_a_street_and_runs_on_upgrades_the_street() {
    let mut w = world(&[(0.0, 0.0, 100.0, 0.0, RoadClass::Street)]);
    // The old street ($1,500 to upgrade), then 100 m of new main road ($2,500).
    assert_eq!(
        w.draft_cost(&raw(&[(0.0, 0.0), (200.0, 0.0)]), RoadClass::Main),
        Some(4_000)
    );
    assert!(road(&mut w, 0.0, 0.0, 200.0, 0.0, RoadClass::Main));
    assert_eq!(w.stats().money.balance, START_FUNDS - 4_000);
    // Now one class and in line, the two pieces join (ADR 0040).
    assert_eq!(classes(&w), [RoadClass::Main]);
}

#[test]
fn a_straight_line_between_a_curved_roads_ends_upgrades_the_curve() {
    let mut w = world(&[]);
    assert!(w.apply(Command::BuildCurve {
        ax: 0.0,
        ay: 0.0,
        cx: 100.0,
        cy: 80.0,
        bx: 200.0,
        by: 0.0,
        class: RoadClass::Street,
        chain: false,
    }));
    let roads = w.roads.road_count();
    assert!(road(&mut w, 0.0, 0.0, 200.0, 0.0, RoadClass::Main));
    assert_eq!(w.roads.road_count(), roads, "no straight road was added");
    assert!(classes(&w).iter().all(|&c| c == RoadClass::Main));
}

#[test]
fn a_street_drawn_over_an_alley_turns_it_into_a_street() {
    let mut w = world(&[(0.0, 0.0, 100.0, 0.0, RoadClass::Alley)]);
    assert_eq!(
        w.draft_cost(&raw(&[(0.0, 0.0), (100.0, 0.0)]), RoadClass::Street),
        Some(500)
    );
    assert!(road(&mut w, 0.0, 0.0, 100.0, 0.0, RoadClass::Street));
    assert_eq!(classes(&w), [RoadClass::Street]);
}

#[test]
fn an_upgrade_the_city_cannot_afford_is_refused() {
    let mut w = world(&[(0.0, 0.0, 4_000.0, 0.0, RoadClass::Street)]);
    w.add_funds_for_bench(50_000 - START_FUNDS);
    // 4 km at $15/m is $60,000, more than the city's $50,000.
    assert!(!road(&mut w, 0.0, 0.0, 4_000.0, 0.0, RoadClass::Main));
    assert_eq!(classes(&w), [RoadClass::Street]);
    assert_eq!(w.stats().money.balance, 50_000);
}

#[test]
fn cars_keep_driving_when_their_road_is_upgraded() {
    let mut w = World::generate(Config::default(), 8, 8);
    w.apply(Command::SpawnAgents { count: 500 });
    for _ in 0..600 {
        w.tick();
    }
    let streets: Vec<_> = w
        .roads
        .active_roads()
        .filter(|&r| w.roads.road_class(r) == RoadClass::Street)
        .collect();
    let lanes = w.roads.lane_count();
    let despawned = w.stats().despawned_by_edit;
    let mut upgraded = 0;
    for r in streets {
        let (a, b) = w.roads.road_ends(r);
        let (a, b) = (a as usize, b as usize);
        let (ax, ay, bx, by) = (
            w.roads.node_x[a],
            w.roads.node_y[a],
            w.roads.node_x[b],
            w.roads.node_y[b],
        );
        if road(&mut w, ax, ay, bx, by, RoadClass::Main) {
            upgraded += 1;
        }
    }
    assert!(upgraded > 10, "upgraded {upgraded} streets");
    assert_eq!(w.roads.lane_count(), lanes, "no road was rebuilt");
    assert_eq!(w.stats().despawned_by_edit, despawned, "no car was removed");
    for _ in 0..600 {
        w.tick();
    }
}

#[test]
fn drawing_over_a_road_of_the_same_class_changes_nothing() {
    let mut w = world(&[(0.0, 0.0, 100.0, 0.0, RoadClass::Main)]);
    let hash = w.state_hash();
    assert!(!road(&mut w, 0.0, 0.0, 100.0, 0.0, RoadClass::Main));
    assert_eq!(w.state_hash(), hash);
}
