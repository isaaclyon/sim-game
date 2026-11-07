//! Exact placement for undo and redo (ADR 0051): a recorded road goes back exactly where it
//! was. Nothing snaps or pulls; anything that would move it refuses the plan instead.

use super::{Join, MIN_ROAD_LEN, Plan, Stop, intersections, node_stop, on_road, overlaps_any};
use crate::geometry::{Point, Shape, dist};
use crate::road::RoadNetwork;

/// Positions closer than this are the same place, metres.
pub(crate) const EXACT_EPS: f32 = 0.01;

/// Plans `shape` with its ends and crossings where they are: each end on the node at that
/// exact position, else splitting the road through it, else a new node; every road it crosses
/// split there. None if the shape is invalid, a crossing would move onto a road's end, a leg
/// would run along a road or be too short, or it would cross a road twice.
pub(crate) fn exact_plan(net: &RoadNetwork, shape: Shape) -> Option<Plan> {
    if !shape.valid() {
        return None;
    }
    let mut stops = vec![
        (0.0, exact_end(net, shape.a)?),
        (1.0, exact_end(net, shape.b)?),
    ];
    let on_shape = |p: Point| {
        let t = shape.nearest(p);
        (dist(shape.point(t), p) < EXACT_EPS).then_some(t)
    };
    let at_end = |p: Point| dist(p, shape.a) < EXACT_EPS || dist(p, shape.b) < EXACT_EPS;
    for road in net.active_roads() {
        for (_, u) in intersections(shape, net.shape(road)) {
            let s = on_road(net, road, u);
            if at_end((s.x, s.y)) {
                continue;
            }
            stops.push((on_shape((s.x, s.y))?, s));
        }
        // Road ends touching the shape are junctions even where the chords just miss them.
        let (a, b) = net.road_ends(road);
        for n in [a, b] {
            let p = (net.node_x[n as usize], net.node_y[n as usize]);
            if let Some(t) = on_shape(p).filter(|_| !at_end(p)) {
                stops.push((t, node_stop(net, n)));
            }
        }
    }
    stops.sort_by(|s, t| s.0.total_cmp(&t.0));
    stops.dedup_by(|s, t| s.1.join == t.1.join && matches!(s.1.join, Join::Node(_)));
    for (i, s) in stops.iter().enumerate() {
        if s.1.join != Join::New && stops[..i].iter().any(|t| t.1.join == s.1.join) {
            return None;
        }
    }
    let mut legs = Vec::with_capacity(stops.len() - 1);
    for pair in stops.windows(2) {
        match (pair[0].1.join, pair[1].1.join) {
            (Join::Node(m), Join::Node(n)) if net.has_road(m, n) => return None,
            (Join::Node(n), Join::Split(r)) | (Join::Split(r), Join::Node(n)) => {
                let (a, b) = net.road_ends(r);
                if n == a || n == b {
                    return None;
                }
            }
            _ => {}
        }
        let mut leg = shape.section(pair[0].0, pair[1].0);
        leg.a = (pair[0].1.x, pair[0].1.y);
        leg.b = (pair[1].1.x, pair[1].1.y);
        if !leg.valid()
            || leg.lengths().last().copied().unwrap_or(0.0) < MIN_ROAD_LEN
            || overlaps_any(net, leg)
        {
            return None;
        }
        legs.push(leg);
    }
    Some(Plan {
        stops: stops.into_iter().map(|s| s.1).collect(),
        legs,
        reclass: Vec::new(),
    })
}

/// The node at `p`, kept unused nodes included; else a split of the road through `p`, unless
/// that would move it onto the road's end; else a new node.
fn exact_end(net: &RoadNetwork, p: Point) -> Option<Stop> {
    if let Some(n) = net.nearest_node(p.0, p.1, EXACT_EPS) {
        return Some(node_stop(net, n));
    }
    for road in net.active_roads() {
        let shape = net.shape(road);
        let u = shape.nearest(p);
        if dist(shape.point(u), p) < EXACT_EPS {
            let s = on_road(net, road, u);
            return matches!(s.join, Join::Split(_)).then_some(Stop {
                x: p.0,
                y: p.1,
                join: s.join,
            });
        }
    }
    Some(Stop {
        x: p.0,
        y: p.1,
        join: Join::New,
    })
}
