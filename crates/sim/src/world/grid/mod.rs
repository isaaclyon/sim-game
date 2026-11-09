//! The grid tool (ADR 0049): one command lays a lattice of blocks. Each lattice edge is a
//! candidate straight road, planned in a fixed order like a hand-drawn road against the
//! network plus the edges already accepted. Planning works on a scratch copy of the roads, so
//! previews, quotes and builds share it and change nothing until a build applies the plans.

mod draft;
mod sliver;

use sliver::Fit;

pub use draft::GridPreview;

use std::collections::BTreeSet;

use super::money;
use crate::era::Era;
use crate::geometry::{Point, Shape};
use crate::road::{NodeId, RoadClass, RoadNetwork};
use crate::snap::{self, Join, Plan};

/// Most blocks along each axis: 840 edges at 20 × 20.
pub const MAX_BLOCKS: i32 = 20;
/// Cell sizes the sim accepts, metres.
pub const MIN_CELL: f32 = 40.0;
pub const MAX_CELL: f32 = 200.0;

/// A lattice: point (i, j) is `origin + i·cell·axis + j·cell·side`, where `side` is `axis`
/// turned 90° counter-clockwise. Signed counts pick the side.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Lattice {
    pub origin: Point,
    pub axis: Point,
    pub cell: f32,
    pub cols: i32,
    pub rows: i32,
}

/// What a grid would build, for previews and the build itself.
pub(crate) struct GridPlan {
    /// Accepted edges' plans in build order; each was planned with the ones before it built.
    pub plans: Vec<Plan>,
    /// Dollars for every accepted edge's new legs.
    pub cost: i64,
    /// `[ax, ay, a_join, bx, by, b_join]` per new leg; joins as `snap` stops in previews:
    /// 0 new end, 1 existing node, 2 splits a road.
    pub roads: Vec<[f32; 6]>,
    /// `[ax, ay, bx, by]` per lattice edge left out.
    pub dropped: Vec<[f32; 4]>,
}

/// One lattice edge: its ends, where they sit in the lattice, and its direction.
struct Edge {
    a: Point,
    b: Point,
    ends: [usize; 2],
    along_axis: bool,
}

impl Lattice {
    /// Whether the command's numbers make a grid the sim builds.
    pub fn valid(&self) -> bool {
        let finite = [
            self.origin.0,
            self.origin.1,
            self.axis.0,
            self.axis.1,
            self.cell,
        ]
        .iter()
        .all(|v| v.is_finite());
        let unit = self.axis.0 * self.axis.0 + self.axis.1 * self.axis.1;
        let blocks = |n: i32| n != 0 && n.abs() <= MAX_BLOCKS;
        finite
            && (unit - 1.0).abs() < 1e-3
            && (MIN_CELL..=MAX_CELL).contains(&self.cell)
            && blocks(self.cols)
            && blocks(self.rows)
    }

    /// Lattice point (i, j), with multiply and add only so it is the same on every target.
    fn point(&self, i: i32, j: i32) -> Point {
        let (u, v) = (i as f32 * self.cell, j as f32 * self.cell);
        let (ax, ay) = self.axis;
        (
            self.origin.0 + u * ax - v * ay,
            self.origin.1 + u * ay + v * ax,
        )
    }

    /// Edges in planning order: along the axis row by row, then across it column by column,
    /// each by increasing index.
    fn edges(&self) -> Vec<Edge> {
        let (i0, i1) = (self.cols.min(0), self.cols.max(0));
        let (j0, j1) = (self.rows.min(0), self.rows.max(0));
        let width = (i1 - i0 + 1) as usize;
        let index = |i: i32, j: i32| (i - i0) as usize + (j - j0) as usize * width;
        let mut out = Vec::new();
        let edge = |i: i32, j: i32, di: i32, dj: i32| Edge {
            a: self.point(i, j),
            b: self.point(i + di, j + dj),
            ends: [index(i, j), index(i + di, j + dj)],
            along_axis: di == 1,
        };
        for j in j0..=j1 {
            for i in i0..i1 {
                out.push(edge(i, j, 1, 0));
            }
        }
        for i in i0..=i1 {
            for j in j0..j1 {
                out.push(edge(i, j, 0, 1));
            }
        }
        out
    }
}

/// Plans `lattice` as `class` against `net` (ADR 0049). Changes nothing.
pub(crate) fn plan(net: &RoadNetwork, lattice: &Lattice, class: RoadClass, era: Era) -> GridPlan {
    let edges = lattice.edges();
    // 1. Edges with existing roads alongside give way to them.
    let fits: Vec<Fit> = edges.iter().map(|e| sliver::fit(net, e.a, e.b)).collect();
    let points = edges.iter().flat_map(|e| e.ends).max().map_or(0, |n| n + 1);
    let mut at_point: Vec<Vec<usize>> = vec![Vec::new(); points];
    for (k, e) in edges.iter().enumerate() {
        for end in e.ends {
            at_point[end].push(k);
        }
    }
    let mut out = GridPlan {
        plans: Vec::new(),
        cost: 0,
        roads: Vec::new(),
        dropped: Vec::new(),
    };
    let mut scratch = net.clone();
    let old_nodes = net.node_count() as NodeId;
    let mut split_nodes = BTreeSet::new();
    for (k, e) in edges.iter().enumerate() {
        let (mut ends, moved) = match &fits[k] {
            Fit::Drop(_) => {
                out.dropped.push([e.a.0, e.a.1, e.b.0, e.b.1]);
                continue;
            }
            Fit::Keep => ([e.a, e.b], [false; 2]),
            &Fit::Trim(a, b) => ([a, b], [a != e.a, b != e.b]),
        };
        // 2. An end on a left-out cross edge's point moves along this edge to its roads.
        let dir = (e.b.0 - e.a.0, e.b.1 - e.a.1);
        for i in 0..2 {
            if moved[i] {
                continue;
            }
            let roads = at_point[e.ends[i]]
                .iter()
                .filter(|&&o| edges[o].along_axis != e.along_axis)
                .flat_map(|&o| match &fits[o] {
                    Fit::Drop(roads) => roads.as_slice(),
                    _ => &[],
                })
                .copied();
            let Some(p) = sliver::meet(net, roads, ends[i], dir, 2.0 * sliver::SLIVER) else {
                continue;
            };
            // Kept only if the moved end does not run alongside a road itself.
            let mut moved_to = ends;
            moved_to[i] = p;
            let clear = match sliver::fit(net, moved_to[0], moved_to[1]) {
                Fit::Keep => true,
                Fit::Trim(a, b) => [a, b][i] == p,
                Fit::Drop(_) => false,
            };
            if clear {
                ends = moved_to;
            }
        }
        // 3. Planned like a hand-drawn road; followed roads keep their class.
        let shape = Shape {
            a: ends[0],
            b: ends[1],
            control: None,
        };
        let Some(plan) = snap::plan(&scratch, shape, None) else {
            out.dropped.push([e.a.0, e.a.1, e.b.0, e.b.1]);
            continue;
        };
        // 4. Refused if any new leg would still run alongside a road: no slivers, ever.
        let new_legs: Vec<_> = plan
            .legs
            .iter()
            .zip(plan.stops.windows(2))
            .filter(|(_, pair)| match (pair[0].join, pair[1].join) {
                (Join::Node(m), Join::Node(n)) => !scratch.has_road(m, n),
                _ => true,
            })
            .map(|(leg, pair)| (*leg, pair[0].join, pair[1].join))
            .collect();
        if new_legs
            .iter()
            .any(|(leg, _, _)| sliver::makes_sliver(&scratch, leg.a, leg.b))
        {
            out.dropped.push([e.a.0, e.a.1, e.b.0, e.b.1]);
            continue;
        }
        out.cost += money::plan_cost(&scratch, &plan, class, era);
        let code = |join: Join| match join {
            Join::New => 0.0,
            Join::Split(_) => 2.0,
            Join::Node(n) if split_nodes.contains(&n) => 2.0,
            Join::Node(n) if n < old_nodes => 1.0,
            Join::Node(_) => 0.0,
        };
        for (leg, a, b) in new_legs {
            out.roads
                .push([leg.a.0, leg.a.1, code(a), leg.b.0, leg.b.1, code(b)]);
        }
        let nodes = snap::apply(&mut scratch, &plan, class, |_, _| {});
        for (stop, node) in plan.stops.iter().zip(nodes) {
            if matches!(stop.join, Join::Split(_)) {
                split_nodes.insert(node);
            }
        }
        out.plans.push(plan);
    }
    out
}
