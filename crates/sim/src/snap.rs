//! Road snapping: where a drawn road joins the network.
//!
//! `plan_road` turns the two points a player clicked into the stops the new road passes
//! through, in order from the first point. A stop is an existing node, a point where the new
//! road splits an existing one (a crossing or a T-junction), or a new free end. Planning only
//! reads the network, so the build preview and `Command::BuildRoad` share it and the preview
//! always shows what gets built.

use crate::road::{NodeId, RoadId, RoadNetwork};

/// Clicks this close to a node join it; clicks this close to a road join it at a T-junction.
/// A crossing this close to the end of the crossed road joins that end instead of splitting
/// it, so a split never leaves a piece shorter than this.
pub const SNAP_RADIUS: f32 = 15.0;
/// Shortest road piece a build may create, metres. Shorter pieces leave no room for the
/// junctions at either end.
pub const MIN_ROAD_LEN: f32 = 10.0;
/// A new road piece within this distance of an existing road, and nearly parallel to it,
/// would be drawn on top of it, metres.
const OVERLAP_DIST: f32 = 8.0;
/// Sine of the largest angle at which two roads count as parallel (about 11.5°).
const PARALLEL_SIN: f32 = 0.2;

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Join {
    /// A new node.
    New,
    /// An existing node.
    Node(NodeId),
    /// A new node that splits this road in two.
    Split(RoadId),
}

#[derive(Clone, Copy, Debug)]
pub struct Stop {
    pub x: f32,
    pub y: f32,
    pub join: Join,
}

/// The stops a road drawn from `(ax, ay)` to `(bx, by)` passes through, or `None` if it can't
/// be built: it would run on top of a road, cross one road twice, loop back through a node,
/// leave a piece shorter than `MIN_ROAD_LEN`, or add nothing new. Between two stops that
/// already have a road, the new road follows it.
pub fn plan_road(net: &RoadNetwork, ax: f32, ay: f32, bx: f32, by: f32) -> Option<Vec<Stop>> {
    let a = snap_end(net, ax, ay);
    let b = snap_end(net, bx, by);
    let mut stops = vec![(0.0, a)];
    for road in 0..net.road_count() as RoadId {
        if a.join == Join::Split(road) || b.join == Join::Split(road) {
            continue; // An end already joins it there.
        }
        let (p, q) = ends(net, road);
        if let Some((t, u)) = intersect((a.x, a.y), (b.x, b.y), p, q) {
            stops.push((t, on_road(net, road, u)));
        }
    }
    stops.push((1.0, b));
    // Stable, and roads were visited in id order, so ties resolve the same way every time.
    stops.sort_by(|s, t| s.0.total_cmp(&t.0));
    let mut stops: Vec<Stop> = stops.into_iter().map(|(_, s)| s).collect();
    // A crossing at a node the road already passes through, such as an end, is the same stop.
    stops.dedup_by(|s, t| matches!((s.join, t.join), (Join::Node(m), Join::Node(n)) if m == n));
    // A crossing snapped to a node a little off the drawn line. If the line then also crosses
    // a road leaving that node, splitting it would lay the new road along it, so pass through
    // the node instead.
    while let Some(i) = (0..stops.len()).find(|&i| {
        let Join::Split(road) = stops[i].join else {
            return false;
        };
        let (a, b) = net.road_ends(road);
        let next_to = |j: Option<usize>| {
            j.and_then(|j| stops.get(j))
                .is_some_and(|s| s.join == Join::Node(a) || s.join == Join::Node(b))
        };
        next_to(i.checked_sub(1)) || next_to(Some(i + 1))
    }) {
        stops.remove(i);
    }

    for (i, s) in stops.iter().enumerate() {
        if s.join != Join::New && stops[..i].iter().any(|t| t.join == s.join) {
            return None;
        }
    }
    let mut new_legs = 0;
    for leg in stops.windows(2) {
        if let (Join::Node(m), Join::Node(n)) = (leg[0].join, leg[1].join)
            && net.has_road(m, n)
        {
            continue; // The road follows one that is already there.
        }
        let (p, q) = ((leg[0].x, leg[0].y), (leg[1].x, leg[1].y));
        if dist(p, q) < MIN_ROAD_LEN || overlaps_any(net, p, q) {
            return None;
        }
        new_legs += 1;
    }
    (new_legs > 0).then_some(stops)
}

/// Where a road end clicked at `(x, y)` goes: the nearest node in reach, else the nearest road
/// in reach (a T-junction), else a new node.
pub fn snap_end(net: &RoadNetwork, x: f32, y: f32) -> Stop {
    if let Some(n) = net.nearest_node(x, y, SNAP_RADIUS) {
        return node_stop(net, n);
    }
    let mut best = None;
    let mut best_d = SNAP_RADIUS;
    for road in 0..net.road_count() as RoadId {
        let (p, q) = ends(net, road);
        let u = project(p, q, (x, y));
        let d = dist(lerp(p, q, u), (x, y));
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

/// The point a fraction `u` along `road`: one of its ends if that is within `SNAP_RADIUS`,
/// otherwise a split.
fn on_road(net: &RoadNetwork, road: RoadId, u: f32) -> Stop {
    let (p, q) = ends(net, road);
    let at = lerp(p, q, u);
    let (dp, dq) = (dist(at, p), dist(at, q));
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

/// True if a road from `p` to `q` would run along an existing road, nearly parallel and less
/// than `OVERLAP_DIST` away over some stretch.
fn overlaps_any(net: &RoadNetwork, p: (f32, f32), q: (f32, f32)) -> bool {
    let len = dist(p, q);
    let d = ((q.0 - p.0) / len, (q.1 - p.1) / len);
    (0..net.road_count() as RoadId).any(|road| {
        let (r0, r1) = ends(net, road);
        let r_len = dist(r0, r1).max(1e-3);
        let r_dir = ((r1.0 - r0.0) / r_len, (r1.1 - r0.1) / r_len);
        if cross(d, r_dir).abs() >= PARALLEL_SIN {
            return false;
        }
        // The stretch of the new piece that lies alongside the road, by projection.
        let s0 = dot((r0.0 - p.0, r0.1 - p.1), d);
        let s1 = dot((r1.0 - p.0, r1.1 - p.1), d);
        let (lo, hi) = (s0.min(s1).max(0.0), s0.max(s1).min(len));
        if hi - lo < 1.0 {
            return false; // End to end, or not alongside at all.
        }
        let mid = (p.0 + d.0 * (lo + hi) / 2.0, p.1 + d.1 * (lo + hi) / 2.0);
        dist(mid, lerp(r0, r1, project(r0, r1, mid))) < OVERLAP_DIST
    })
}

fn ends(net: &RoadNetwork, road: RoadId) -> ((f32, f32), (f32, f32)) {
    let (a, b) = net.road_ends(road);
    let (a, b) = (a as usize, b as usize);
    (
        (net.node_x[a], net.node_y[a]),
        (net.node_x[b], net.node_y[b]),
    )
}

/// Where segments `p0 → p1` and `q0 → q1` cross, as fractions `(t, u)` along each. `None` if
/// they don't, or are parallel.
fn intersect(p0: (f32, f32), p1: (f32, f32), q0: (f32, f32), q1: (f32, f32)) -> Option<(f32, f32)> {
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

/// Fraction along `p → q` of the point on it nearest `x`, clamped to the segment.
fn project(p: (f32, f32), q: (f32, f32), x: (f32, f32)) -> f32 {
    let d = (q.0 - p.0, q.1 - p.1);
    let len2 = dot(d, d).max(1e-6);
    (dot((x.0 - p.0, x.1 - p.1), d) / len2).clamp(0.0, 1.0)
}

fn lerp(p: (f32, f32), q: (f32, f32), t: f32) -> (f32, f32) {
    (p.0 + (q.0 - p.0) * t, p.1 + (q.1 - p.1) * t)
}

fn dist(p: (f32, f32), q: (f32, f32)) -> f32 {
    (q.0 - p.0).hypot(q.1 - p.1)
}

fn dot(a: (f32, f32), b: (f32, f32)) -> f32 {
    a.0 * b.0 + a.1 * b.1
}

fn cross(a: (f32, f32), b: (f32, f32)) -> f32 {
    a.0 * b.1 - a.1 * b.0
}
