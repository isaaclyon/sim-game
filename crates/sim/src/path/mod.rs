//! A* over the lane graph. Cost is free-flow travel time, raised by a small per-driver
//! amount (ADR 0024), plus a per-lane congestion delay, so routes prefer fast main roads until
//! they back up. Long trips search only main roads in their middle (ADR 0017).

use std::cmp::Ordering;
use std::collections::BinaryHeap;

use crate::road::{LaneId, MAIN_SPEED, NodeId, RoadClass, RoadNetwork};

mod rejoin;

/// Weighted A*: routes cost at most this much more than optimal.
const HEURISTIC_WEIGHT: f32 = 1.5;
/// Straight-line distance from a trip's start or end within which it may use local streets.
/// Farther from both, the search follows main roads only, the way a driver heads for the
/// nearest main road, rides it, and leaves it near the destination (ADR 0017).
pub const MAIN_ROAD_REACH: f32 = 500.0;
/// Most a driver's taste raises a lane's free-flow time, as a fraction. Each driver sees each
/// lane as up to this much slower, fixed per driver and lane, so drivers split over routes
/// that cost about the same instead of all taking the single cheapest (ADR 0024).
pub const ROUTE_NOISE: f32 = 0.2;

/// What one search pays per lane.
struct Costs<'a> {
    net: &'a RoadNetwork,
    delay: &'a [f32],
    driver: Option<u64>,
}

impl Costs<'_> {
    /// Seconds to drive `lane`: free-flow time scaled by the driver's taste, plus congestion.
    fn lane(&self, lane: LaneId) -> f32 {
        let l = lane as usize;
        let mut time = self.net.lane_len[l] / self.net.lane_speed[l];
        if let Some(driver) = self.driver {
            // Top 24 bits as a uniform [0, 1), as `Rng::unit` does.
            let unit = (crate::rng::mix(driver, u64::from(lane)) >> 40) as f32 / (1 << 24) as f32;
            time *= 1.0 + ROUTE_NOISE * unit;
        }
        time + self.delay.get(l).unwrap_or(&0.0)
    }
}

/// Reusable search state. Buffers persist between searches and a generation stamp marks
/// which entries are current, so a search never pays to clear per-node arrays.
#[derive(Default)]
pub struct Pathfinder {
    g: Vec<f32>,
    via: Vec<LaneId>,
    stamp: Vec<u32>,
    generation: u32,
    open: BinaryHeap<Open>,
    /// Nodes expanded by the most recent search.
    pub last_expanded: u32,
    // `rejoin` only: the route position of each node on the old route, and suffix costs.
    on_route: Vec<u32>,
    route_stamp: Vec<u32>,
    rest: Vec<f32>,
}

#[derive(PartialEq)]
struct Open {
    f: f32,
    g: f32,
    node: NodeId,
}

impl Eq for Open {}

impl Ord for Open {
    fn cmp(&self, other: &Self) -> Ordering {
        // BinaryHeap is a max-heap, so compare reversed to pop the lowest f first.
        // Node id breaks ties so results never depend on heap internals.
        other
            .f
            .total_cmp(&self.f)
            .then_with(|| other.node.cmp(&self.node))
    }
}

impl PartialOrd for Open {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Pathfinder {
    /// Writes the lanes from `from` to `to` into `out`. Returns false if `to` is unreachable.
    /// `delay` is extra seconds per lane (congestion); lanes past its end cost nothing extra.
    /// `driver` seeds the driver's route taste (`ROUTE_NOISE`); `None` uses plain travel times.
    ///
    /// Farther than `MAIN_ROAD_REACH` from both ends, only main roads are searched. If that
    /// finds no route (no main road near an end, or none linking them), the search runs again
    /// over every road, so a reachable destination is always found.
    pub fn find(
        &mut self,
        net: &RoadNetwork,
        delay: &[f32],
        driver: Option<u64>,
        from: NodeId,
        to: NodeId,
        out: &mut Vec<LaneId>,
    ) -> bool {
        self.last_expanded = 0;
        let costs = Costs { net, delay, driver };
        (net.has_main_roads() && self.search(&costs, from, to, MAIN_ROAD_REACH, out))
            || self.search(&costs, from, to, f32::INFINITY, out)
    }

    /// A* from `from` to `to` that leaves nodes farther than `reach` from both ends only by
    /// main roads. Adds its expanded nodes to `last_expanded`.
    fn search(
        &mut self,
        costs: &Costs,
        from: NodeId,
        to: NodeId,
        reach: f32,
        out: &mut Vec<LaneId>,
    ) -> bool {
        out.clear();
        let net = costs.net;
        self.begin(net.node_count());

        let (fx, fy) = (net.node_x[from as usize], net.node_y[from as usize]);
        let (tx, ty) = (net.node_x[to as usize], net.node_y[to as usize]);
        // Straight-line distance at the fastest speed limit never overestimates (delays and
        // route noise never lower a cost). Inflating it trades up to HEURISTIC_WEIGHT× route
        // cost for far fewer expansions, which matters once congestion delays make the plain
        // bound loose.
        let h = |node: NodeId| {
            let i = node as usize;
            (net.node_x[i] - tx).hypot(net.node_y[i] - ty) / MAIN_SPEED * HEURISTIC_WEIGHT
        };
        let reach2 = reach * reach;
        let local = |node: NodeId| {
            let (x, y) = (net.node_x[node as usize], net.node_y[node as usize]);
            let d2 = |px: f32, py: f32| (x - px) * (x - px) + (y - py) * (y - py);
            d2(fx, fy) <= reach2 || d2(tx, ty) <= reach2
        };

        self.open.clear();
        self.visit(from, 0.0, LaneId::MAX);
        self.open.push(Open {
            f: h(from),
            g: 0.0,
            node: from,
        });

        while let Some(Open { g, node, .. }) = self.open.pop() {
            if node == to {
                let mut cur = to;
                while cur != from {
                    let lane = self.via[cur as usize];
                    out.push(lane);
                    cur = net.lane_from[lane as usize];
                }
                out.reverse();
                return true;
            }
            if g > self.g[node as usize] {
                continue; // stale heap entry; a cheaper route to this node was found later
            }
            self.last_expanded += 1;
            let main_only = !local(node);
            for &lane in net.out_lanes(node) {
                if main_only && net.lane_class(lane) != RoadClass::Main {
                    continue;
                }
                let l = lane as usize;
                let next = net.lane_to[l];
                let ng = g + costs.lane(lane);
                let ni = next as usize;
                if self.stamp[ni] != self.generation || ng < self.g[ni] {
                    self.visit(next, ng, lane);
                    self.open.push(Open {
                        f: ng + h(next),
                        g: ng,
                        node: next,
                    });
                }
            }
        }
        false
    }

    /// Starts a new search: sizes the scratch buffers and bumps the generation stamp.
    fn begin(&mut self, n: usize) {
        if self.stamp.len() < n {
            self.g.resize(n, 0.0);
            self.via.resize(n, 0);
            self.stamp.resize(n, 0);
        }
        self.generation = self.generation.wrapping_add(1);
        if self.generation == 0 {
            self.stamp.fill(0);
            self.route_stamp.fill(0);
            self.generation = 1;
        }
    }

    fn visit(&mut self, node: NodeId, g: f32, via: LaneId) {
        let i = node as usize;
        self.stamp[i] = self.generation;
        self.g[i] = g;
        self.via[i] = via;
    }
}

#[cfg(test)]
mod tests;
