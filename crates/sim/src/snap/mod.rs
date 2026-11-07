//! Construction planning shared by previews and commands. Planning never mutates the graph.

mod exact;

use crate::geometry::{Point, Shape, dist, lerp, project};
use crate::road::{NodeId, RoadClass, RoadId, RoadNetwork};

pub(crate) use exact::{EXACT_EPS, exact_plan};

/// Clicks near a node or road join it. Splits near an end join that end instead.
pub const SNAP_RADIUS: f32 = 15.0;
/// Shorter pieces leave no room for junctions.
pub const MIN_ROAD_LEN: f32 = 10.0;
const OVERLAP_DIST: f32 = 8.0;
const PARALLEL_SIN: f32 = 0.2;
/// Position tolerance for refined intersections and already-snapped endpoints, metres.
const INTERSECTION_EPS: f32 = 0.1;

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Join {
    New,
    Node(NodeId),
    Split(RoadId),
}
#[derive(Clone, Copy, Debug)]
pub struct Stop {
    pub x: f32,
    pub y: f32,
    pub join: Join,
}

pub(crate) struct Plan {
    pub stops: Vec<Stop>,
    pub legs: Vec<Shape>,
    /// Followed roads whose class differs from the one being drawn; building changes them to
    /// it (ADR 0048).
    pub reclass: Vec<RoadId>,
}

/// Stops of a straight road, or None if it overlaps, loops, crosses one road twice,
/// leaves a short piece or adds nothing. Existing roads between stops are followed.
pub fn plan_road(net: &RoadNetwork, ax: f32, ay: f32, bx: f32, by: f32) -> Option<Vec<Stop>> {
    plan(
        net,
        Shape {
            a: (ax, ay),
            b: (bx, by),
            control: None,
        },
        None,
    )
    .map(|p| p.stops)
}

/// Plans `shape`. With `class`, followed roads of another class are listed for a class change
/// (ADR 0048) and count as something added; without, they are kept as they are.
pub(crate) fn plan(net: &RoadNetwork, mut shape: Shape, class: Option<RoadClass>) -> Option<Plan> {
    if !shape.valid() {
        return None;
    }
    let a = snap_end(net, shape.a.0, shape.a.1);
    let b = snap_end(net, shape.b.0, shape.b.1);
    shape.a = (a.x, a.y);
    shape.b = (b.x, b.y);
    if !shape.valid() {
        return None;
    }
    let mut stops = vec![(0.0, a)];
    for road in net.active_roads() {
        for (t, u) in intersections(shape, net.shape(road)) {
            // Parameter error scales with road length and coordinate magnitude. Compare
            // positions so float rounding cannot turn an endpoint into a second crossing.
            let at = shape.point(t);
            if (a.join == Join::Split(road) && dist(at, shape.a) < INTERSECTION_EPS)
                || (b.join == Join::Split(road) && dist(at, shape.b) < INTERSECTION_EPS)
            {
                continue;
            }
            stops.push((t, on_road(net, road, u)));
        }
    }
    stops.push((1.0, b));
    stops.sort_by(|s, t| s.0.total_cmp(&t.0));
    stops.dedup_by(|s, t| matches!((s.1.join,t.1.join),(Join::Node(m),Join::Node(n)) if m == n));
    // A crossing snapped to a node off the drawn line may also cross an adjacent arm.
    // Follow that node rather than laying another road along the arm.
    while let Some(i) = (0..stops.len()).find(|&i| {
        if shape.control.is_some() {
            return false;
        }
        let Join::Split(road) = stops[i].1.join else {
            return false;
        };
        // A straight road can leave a curve's endpoint and cross it later. That crossing
        // is real, not a near-junction artefact along a straight arm.
        if net.shape(road).control.is_some() {
            return false;
        }
        let (a, b) = net.road_ends(road);
        let next_to = |j: Option<usize>| {
            j.and_then(|j| stops.get(j))
                .is_some_and(|s| s.1.join == Join::Node(a) || s.1.join == Join::Node(b))
        };
        next_to(i.checked_sub(1)) || next_to(Some(i + 1))
    }) {
        stops.remove(i);
    }
    // A straight line can run along a road without crossing it. Its ends become stops, so the
    // line follows that road (ADR 0048) instead of being refused for overlapping it. Added
    // after the pass above, so a line starting part-way along a road is still refused.
    if shape.control.is_none() {
        for road in net.active_roads() {
            if !overlaps(shape, net.shape(road)) {
                continue;
            }
            let (ra, rb) = net.road_ends(road);
            for n in [ra, rb] {
                let at = (net.node_x[n as usize], net.node_y[n as usize]);
                let t = shape.nearest(at);
                if t > 0.0 && t < 1.0 && dist(shape.point(t), at) < OVERLAP_DIST {
                    stops.push((t, node_stop(net, n)));
                }
            }
        }
        stops.sort_by(|s, t| s.0.total_cmp(&t.0));
        stops.dedup_by(|s, t| s.1.join == t.1.join && matches!(s.1.join, Join::Node(_)));
    }
    for (i, s) in stops.iter().enumerate() {
        if s.1.join != Join::New && stops[..i].iter().any(|t| t.1.join == s.1.join) {
            return None;
        }
    }
    let mut legs = Vec::new();
    let mut reclass = Vec::new();
    let mut new_legs = 0;
    for pair in stops.windows(2) {
        // After splitting, these stops would already share a road. The graph cannot add a
        // parallel connection with the same node pair; do not silently discard that leg.
        let joins = (pair[0].1.join, pair[1].1.join);
        if let (Join::Node(n), Join::Split(r)) | (Join::Split(r), Join::Node(n)) = joins {
            let (a, b) = net.road_ends(r);
            if n == a || n == b {
                return None;
            }
        }
        let mut leg = shape.section(pair[0].0, pair[1].0);
        leg.a = (pair[0].1.x, pair[0].1.y);
        leg.b = (pair[1].1.x, pair[1].1.y);
        if let (Join::Node(m), Join::Node(n)) = (pair[0].1.join, pair[1].1.join)
            && net.has_road(m, n)
        {
            // Curves must not silently turn into an existing road with a different shape.
            if shape.control.is_some() {
                return None;
            }
            let lane = net
                .active_roads()
                .flat_map(|r| [r * 2, r * 2 + 1])
                .find(|&l| net.lane_from[l as usize] == m && net.lane_to[l as usize] == n)?;
            leg = net.shape(lane / 2);
            if class.is_some_and(|c| c != net.road_class(lane / 2)) {
                reclass.push(lane / 2);
            }
            if !lane.is_multiple_of(2) {
                std::mem::swap(&mut leg.a, &mut leg.b);
            }
        } else {
            if !leg.valid()
                || leg.lengths().last().copied().unwrap_or(0.0) < MIN_ROAD_LEN
                || overlaps_any(net, leg)
            {
                return None;
            }
            new_legs += 1;
        }
        legs.push(leg);
    }
    (new_legs > 0 || !reclass.is_empty()).then(|| Plan {
        stops: stops.into_iter().map(|s| s.1).collect(),
        legs,
        reclass,
    })
}

/// A plan that only follows `road` from end to end, giving it `class` in place (ADR 0050):
/// what drawing a road of that class exactly over it plans under ADR 0048. None if the road
/// already has that class.
pub(crate) fn reclass_plan(net: &RoadNetwork, road: RoadId, class: RoadClass) -> Option<Plan> {
    if net.road_class(road) == class {
        return None;
    }
    let (a, b) = net.road_ends(road);
    Some(Plan {
        stops: vec![node_stop(net, a), node_stop(net, b)],
        legs: vec![net.shape(road)],
        reclass: vec![road],
    })
}

/// Adds `plan`'s stops and legs to `net` as `class` and returns its nodes in stop order.
/// Followed legs add nothing and keep their class; `reclass` is the caller's. `split` hears of
/// each road split, with the road continuing it. The caller rebuilds the adjacency.
pub(crate) fn apply(
    net: &mut RoadNetwork,
    plan: &Plan,
    class: RoadClass,
    mut split: impl FnMut(RoadId, RoadId),
) -> Vec<NodeId> {
    let nodes: Vec<NodeId> = plan
        .stops
        .iter()
        .map(|s| match s.join {
            Join::Node(n) => n,
            Join::New => net.add_node(s.x, s.y),
            Join::Split(road) => {
                // The plan splits each road at most once, so `road` still has its old ends.
                let n = net.add_node(s.x, s.y);
                let rest = net.split_road(road, n);
                split(road, rest);
                n
            }
        })
        .collect();
    for (leg, shape) in nodes.windows(2).zip(&plan.legs) {
        // False where the plan follows an existing road, which is kept in place.
        net.add_shape(leg[0], leg[1], shape.control, class);
    }
    nodes
}

/// Nearest node in reach, otherwise nearest point on a road, otherwise a new node.
pub fn snap_end(net: &RoadNetwork, x: f32, y: f32) -> Stop {
    if let Some(n) = net.nearest_junction(x, y, SNAP_RADIUS) {
        return node_stop(net, n);
    }
    let mut best = None;
    let mut best_d = SNAP_RADIUS;
    for road in net.active_roads() {
        let shape = net.shape(road);
        let u = shape.nearest((x, y));
        let d = dist(shape.point(u), (x, y));
        if d < best_d {
            best_d = d;
            best = Some((road, u));
        }
    }
    match best {
        Some((road, u)) => on_road(net, road, u),
        None => Stop {
            x,
            y,
            join: Join::New,
        },
    }
}

/// Nearest active centreline in bulldoze reach. Ties keep the lowest road ID.
pub fn bulldoze_target(net: &RoadNetwork, x: f32, y: f32) -> Option<RoadId> {
    if !x.is_finite() || !y.is_finite() {
        return None;
    }
    let mut best = None;
    let mut distance = SNAP_RADIUS;
    for road in net.active_roads() {
        let shape = net.shape(road);
        let d = dist(shape.point(shape.nearest((x, y))), (x, y));
        if d < distance || (best.is_none() && d <= distance) {
            distance = d;
            best = Some(road);
        }
    }
    best
}

/// The point a fraction `u` along `road`: one of its ends if that is within `SNAP_RADIUS`,
/// otherwise a split.
fn on_road(net: &RoadNetwork, road: RoadId, u: f32) -> Stop {
    let shape = net.shape(road);
    let at = shape.point(u);
    let (dp, dq) = (dist(at, shape.a), dist(at, shape.b));
    if dp.min(dq) < SNAP_RADIUS {
        let (a, b) = net.road_ends(road);
        return node_stop(net, if dp <= dq { a } else { b });
    }
    Stop {
        x: at.0,
        y: at.1,
        join: Join::Split(road),
    }
}
fn node_stop(net: &RoadNetwork, n: NodeId) -> Stop {
    Stop {
        x: net.node_x[n as usize],
        y: net.node_y[n as usize],
        join: Join::Node(n),
    }
}

fn overlaps_any(net: &RoadNetwork, shape: Shape) -> bool {
    net.active_roads()
        .any(|road| overlaps(shape, net.shape(road)))
}
fn overlaps(shape: Shape, other: Shape) -> bool {
    shape
        .segments()
        .any(|(_, p, _, q)| other.segments().any(|(_, r0, _, r1)| overlap(p, q, r0, r1)))
}
fn overlap(p: Point, q: Point, r0: Point, r1: Point) -> bool {
    let len = dist(p, q).max(1e-3);
    let d = ((q.0 - p.0) / len, (q.1 - p.1) / len);
    let r_len = dist(r0, r1).max(1e-3);
    let rd = ((r1.0 - r0.0) / r_len, (r1.1 - r0.1) / r_len);
    if cross(d, rd).abs() >= PARALLEL_SIN {
        return false;
    }
    let s0 = dot((r0.0 - p.0, r0.1 - p.1), d);
    let s1 = dot((r1.0 - p.0, r1.1 - p.1), d);
    let (lo, hi) = (s0.min(s1).max(0.0), s0.max(s1).min(len));
    if hi - lo < 1.0 {
        return false;
    }
    let mid = (p.0 + d.0 * (lo + hi) / 2.0, p.1 + d.1 * (lo + hi) / 2.0);
    dist(mid, lerp(r0, r1, project(r0, r1, mid))) < OVERLAP_DIST
}

fn intersections(a: Shape, b: Shape) -> Vec<(f32, f32)> {
    let mut hits: Vec<(f32, f32)> = Vec::new();
    for (t0, p, t1, q) in a.segments() {
        for (u0, r, u1, s) in b.segments() {
            let Some((t, u)) = intersect(p, q, r, s) else {
                continue;
            };
            let (mut t, mut u) = (t0 + (t1 - t0) * t, u0 + (u1 - u0) * u);
            // Chords seed the intersection; Newton refinement puts the junction on both curves.
            for _ in 0..6 {
                let (p, q) = (a.point(t), b.point(u));
                let gap = (q.0 - p.0, q.1 - p.1);
                let (d, e) = (a.tangent(t), b.tangent(u));
                let denom = cross(d, e);
                if denom.abs() < 1e-6 {
                    break;
                }
                t = (t + cross(gap, e) / denom).clamp(0.0, 1.0);
                u = (u + cross(gap, d) / denom).clamp(0.0, 1.0);
            }
            if dist(a.point(t), b.point(u)) < INTERSECTION_EPS
                && !hits
                    .iter()
                    .any(|&(v, w)| (v - t).abs() < 1e-5 && (w - u).abs() < 1e-5)
            {
                hits.push((t, u));
            }
        }
    }
    hits
}
fn intersect(p0: Point, p1: Point, q0: Point, q1: Point) -> Option<(f32, f32)> {
    let r = (p1.0 - p0.0, p1.1 - p0.1);
    let s = (q1.0 - q0.0, q1.1 - q0.1);
    let denom = cross(r, s);
    if denom.abs() <= 1e-6 * dist(p0, p1) * dist(q0, q1) {
        return None;
    }
    let w = (q0.0 - p0.0, q0.1 - p0.1);
    let (t, u) = (cross(w, s) / denom, cross(w, r) / denom);
    ((0.0..=1.0).contains(&t) && (0.0..=1.0).contains(&u)).then_some((t, u))
}
fn dot(a: Point, b: Point) -> f32 {
    a.0 * b.0 + a.1 * b.1
}
fn cross(a: Point, b: Point) -> f32 {
    a.0 * b.1 - a.1 * b.0
}
