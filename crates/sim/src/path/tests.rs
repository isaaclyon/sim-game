use super::*;

#[test]
fn prefers_faster_route() {
    // 0 ── 1 ── 2 on streets, plus a longer but faster main road 0 ── 3 ── 2.
    let mut net = RoadNetwork::default();
    let n0 = net.add_node(0.0, 0.0);
    let n1 = net.add_node(100.0, 0.0);
    let n2 = net.add_node(200.0, 0.0);
    let n3 = net.add_node(100.0, 40.0);
    net.add_road(n0, n1, RoadClass::Street);
    net.add_road(n1, n2, RoadClass::Street);
    net.add_road(n0, n3, RoadClass::Main);
    net.add_road(n3, n2, RoadClass::Main);
    net.rebuild_adjacency();

    let mut pf = Pathfinder::default();
    let mut path = Vec::new();
    // Route noise is small: no driver's taste outweighs a road that is clearly faster.
    for driver in [None].into_iter().chain((0..40).map(Some)) {
        assert!(pf.find(&net, &[], driver, n0, n2, &mut path));
        let visited: Vec<_> = path.iter().map(|&l| net.lane_to[l as usize]).collect();
        assert_eq!(visited, vec![n3, n2], "driver {driver:?}");
    }
}

#[test]
fn unreachable_returns_false() {
    let mut net = RoadNetwork::default();
    let a = net.add_node(0.0, 0.0);
    let b = net.add_node(10.0, 0.0);
    let c = net.add_node(50.0, 0.0);
    net.add_road(a, b, RoadClass::Street);
    net.rebuild_adjacency();
    let mut path = Vec::new();
    assert!(!Pathfinder::default().find(&net, &[], None, a, c, &mut path));
}

/// Street grid 100 m apart, `cols` wide and `rows` high, with row `main_row` (if any) a
/// main road. Node id is `row * cols + col`.
fn town(cols: u32, rows: u32, main_row: Option<u32>) -> RoadNetwork {
    let mut net = RoadNetwork::default();
    for r in 0..rows {
        for c in 0..cols {
            net.add_node(c as f32 * 100.0, r as f32 * 100.0);
        }
    }
    for r in 0..rows {
        for c in 0..cols {
            let n = r * cols + c;
            if c + 1 < cols {
                let class = if main_row == Some(r) {
                    RoadClass::Main
                } else {
                    RoadClass::Street
                };
                net.add_road(n, n + 1, class);
            }
            if r + 1 < rows {
                net.add_road(n, n + cols, RoadClass::Street);
            }
        }
    }
    net.rebuild_adjacency();
    net
}

#[test]
fn drivers_spread_over_equally_good_routes() {
    // Corner to corner across a street grid: many routes tie on free-flow time.
    let net = town(6, 6, None);
    let mut pf = Pathfinder::default();
    let mut path = Vec::new();
    let mut routes = Vec::new();
    for driver in 0..40 {
        assert!(pf.find(&net, &[], Some(driver), 0, 35, &mut path));
        assert_eq!(path.len(), 10, "noise never adds a detour on a plain grid");
        let again = path.clone();
        assert!(pf.find(&net, &[], Some(driver), 0, 35, &mut path));
        assert_eq!(path, again, "a driver keeps their route");
        routes.push(again);
    }
    routes.sort();
    routes.dedup();
    assert!(routes.len() >= 10, "{} distinct routes", routes.len());
}

fn classes(net: &RoadNetwork, path: &[LaneId]) -> Vec<RoadClass> {
    path.iter().map(|&l| net.lane_class(l)).collect()
}

#[test]
fn long_trips_ride_the_main_road() {
    // 2 km along the bottom row. Straight on by street is quicker in free-flow time than
    // the main road 400 m away, but drivers on long trips head for the main road anyway.
    let net = town(21, 5, Some(4));
    let mut pf = Pathfinder::default();
    let mut path = Vec::new();
    assert!(pf.find(&net, &[], None, 0, 20, &mut path));
    let cls = classes(&net, &path);
    assert_eq!(cls[0], RoadClass::Street, "a local street to reach it");
    assert_eq!(
        *cls.last().unwrap(),
        RoadClass::Street,
        "leaves it near the end"
    );
    let on_main = cls.iter().filter(|&&c| c == RoadClass::Main).count();
    assert!(on_main >= 10, "rides it for the middle: {cls:?}");
    // Off the main road only near either end of the trip.
    let (start, end) = (0.0, 2000.0);
    for &lane in &path {
        let n = net.lane_from[lane as usize] as usize;
        let x = net.node_x[n];
        let near = (x - start).abs() <= MAIN_ROAD_REACH || (x - end).abs() <= MAIN_ROAD_REACH;
        assert!(near || net.lane_class(lane) == RoadClass::Main, "{x} m");
    }
}

#[test]
fn short_trips_stay_on_local_streets() {
    let net = town(21, 5, Some(4));
    let mut path = Vec::new();
    assert!(Pathfinder::default().find(&net, &[], None, 0, 4, &mut path));
    assert_eq!(path.len(), 4);
    assert!(classes(&net, &path).iter().all(|&c| c == RoadClass::Street));
}

#[test]
fn trips_far_from_any_main_road_still_find_a_route() {
    // The main road is 1.2 km away, out of reach of both ends, so the main-road search
    // fails and the trip falls back to streets.
    let net = town(21, 13, Some(12));
    let mut path = Vec::new();
    assert!(Pathfinder::default().find(&net, &[], None, 0, 20, &mut path));
    assert_eq!(path.len(), 20);
    assert!(classes(&net, &path).iter().all(|&c| c == RoadClass::Street));
}

/// 3x3 street grid, 100 m apart. Node id is `row * 3 + col`.
fn grid() -> RoadNetwork {
    let mut net = RoadNetwork::default();
    for r in 0..3 {
        for c in 0..3 {
            net.add_node(c as f32 * 100.0, r as f32 * 100.0);
        }
    }
    for r in 0..3 {
        for c in 0..3 {
            let n = r * 3 + c;
            if c < 2 {
                net.add_road(n, n + 1, RoadClass::Street);
            }
            if r < 2 {
                net.add_road(n, n + 3, RoadClass::Street);
            }
        }
    }
    net.rebuild_adjacency();
    net
}

#[test]
fn rejoin_goes_around_the_block() {
    let net = grid();
    let mut pf = Pathfinder::default();
    let mut route = Vec::new();
    assert!(pf.find(&net, &[], None, 0, 2, &mut route));
    assert_eq!(route.len(), 2, "straight along the bottom row");
    let blocked = route[0];

    let mut detour = Vec::new();
    let j = pf
        .rejoin(&net, &[], &route, |_| true, 64, &mut detour)
        .expect("a way around exists");
    assert!(!detour.contains(&blocked));
    assert_eq!(net.lane_from[detour[0] as usize], 0);
    for w in detour.windows(2) {
        assert_eq!(net.lane_to[w[0] as usize], net.lane_from[w[1] as usize]);
    }
    // The detour ends where the kept part of the route starts.
    let end = net.lane_to[*detour.last().unwrap() as usize];
    assert_eq!(end, net.lane_to[route[j] as usize]);
    // Up, along, down: two lanes more than the blocked route.
    assert_eq!(detour.len() + route.len() - j - 1, 4);
}

#[test]
fn rejoin_respects_its_limits() {
    let net = grid();
    let mut pf = Pathfinder::default();
    let mut route = Vec::new();
    assert!(pf.find(&net, &[], None, 0, 2, &mut route));
    let mut detour = Vec::new();
    assert_eq!(pf.rejoin(&net, &[], &route, |_| true, 1, &mut detour), None);
    assert_eq!(
        pf.rejoin(&net, &[], &route, |_| false, 64, &mut detour),
        None
    );
}
