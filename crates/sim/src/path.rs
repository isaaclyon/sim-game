//! A* over the lane graph. Cost is free-flow travel time plus a per-lane congestion delay, so
//! routes prefer fast avenues until they back up.

use std::cmp::Ordering;
use std::collections::BinaryHeap;

use crate::road::{AVENUE_SPEED, LaneId, NodeId, RoadNetwork};

/// Weighted A*: routes cost at most this much more than optimal.
const HEURISTIC_WEIGHT: f32 = 1.5;

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
    pub fn find(
        &mut self,
        net: &RoadNetwork,
        delay: &[f32],
        from: NodeId,
        to: NodeId,
        out: &mut Vec<LaneId>,
    ) -> bool {
        out.clear();
        self.begin(net.node_count());

        let (tx, ty) = (net.node_x[to as usize], net.node_y[to as usize]);
        // Straight-line distance at the fastest speed limit never overestimates (delays are never
        // negative). Inflating it trades up to HEURISTIC_WEIGHT× route cost for far fewer
        // expansions, which matters once congestion delays make the plain bound loose.
        let h = |node: NodeId| {
            let i = node as usize;
            (net.node_x[i] - tx).hypot(net.node_y[i] - ty) / AVENUE_SPEED * HEURISTIC_WEIGHT
        };

        self.open.clear();
        self.last_expanded = 0;
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
            for &lane in net.out_lanes(node) {
                let l = lane as usize;
                let next = net.lane_to[l];
                let ng = g + net.lane_len[l] / net.lane_speed[l] + delay.get(l).unwrap_or(&0.0);
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

    /// Finds a short way around a blocked lane, the way a driver goes around the block.
    /// `route` is the rest of the trip and starts with the blocked lane, which leaves the node
    /// the search starts from. The search avoids that lane, takes only a first lane that
    /// `first_ok` accepts, and stops after `max_expanded` nodes.
    ///
    /// The result rejoins the route at the point that minimises detour cost plus the remaining
    /// route cost. The search is A* toward the trip's end, so it stops as soon as no unexplored
    /// node can beat the best rejoin found. On success the detour lanes are in `out`, and the
    /// return value `j` means the trip continues with `route[j + 1..]`.
    pub fn rejoin(
        &mut self,
        net: &RoadNetwork,
        delay: &[f32],
        route: &[LaneId],
        first_ok: impl Fn(LaneId) -> bool,
        max_expanded: u32,
        out: &mut Vec<LaneId>,
    ) -> Option<usize> {
        out.clear();
        let (&blocked, _) = route.split_first()?;
        let from = net.lane_from[blocked as usize];
        let dest = net.lane_to[route[route.len() - 1] as usize] as usize;
        let (tx, ty) = (net.node_x[dest], net.node_y[dest]);
        // Never overestimates the cost of reaching the trip's end, rejoin or not.
        let h = |node: NodeId| {
            let i = node as usize;
            (net.node_x[i] - tx).hypot(net.node_y[i] - ty) / AVENUE_SPEED
        };
        let cost = |l: usize| net.lane_len[l] / net.lane_speed[l] + delay.get(l).unwrap_or(&0.0);
        self.begin(net.node_count());
        if self.route_stamp.len() < net.node_count() {
            self.on_route.resize(net.node_count(), 0);
            self.route_stamp.resize(net.node_count(), 0);
        }
        // rest[j]: cost of route[j + 1..]. Later visits overwrite earlier ones, so a route
        // that loops through a node rejoins at its last visit and skips the loop.
        self.rest.clear();
        self.rest.resize(route.len(), 0.0);
        for j in (0..route.len() - 1).rev() {
            self.rest[j] = self.rest[j + 1] + cost(route[j + 1] as usize);
        }
        for (j, &lane) in route.iter().enumerate() {
            let node = net.lane_to[lane as usize] as usize;
            self.on_route[node] = j as u32;
            self.route_stamp[node] = self.generation;
        }

        self.open.clear();
        self.last_expanded = 0;
        self.visit(from, 0.0, LaneId::MAX);
        self.open.push(Open {
            f: h(from),
            g: 0.0,
            node: from,
        });
        let mut best: Option<(f32, NodeId)> = None;
        while let Some(Open { f, g, node }) = self.open.pop() {
            if best.is_some_and(|(total, _)| f >= total) || self.last_expanded >= max_expanded {
                break; // Nothing left can beat the best rejoin, or the budget is spent.
            }
            if g > self.g[node as usize] {
                continue;
            }
            self.last_expanded += 1;
            for &lane in net.out_lanes(node) {
                if lane == blocked || (node == from && !first_ok(lane)) {
                    continue;
                }
                let l = lane as usize;
                let next = net.lane_to[l];
                let ni = next as usize;
                let ng = g + cost(l);
                if self.stamp[ni] == self.generation && ng >= self.g[ni] {
                    continue;
                }
                self.visit(next, ng, lane);
                self.open.push(Open {
                    f: ng + h(next),
                    g: ng,
                    node: next,
                });
                if self.route_stamp[ni] == self.generation && next != from {
                    let total = ng + self.rest[self.on_route[ni] as usize];
                    if best.is_none_or(|(b, _)| total < b) {
                        best = Some((total, next));
                    }
                }
            }
        }

        let (_, end) = best?;
        let mut cur = end;
        while cur != from {
            let lane = self.via[cur as usize];
            out.push(lane);
            cur = net.lane_from[lane as usize];
        }
        out.reverse();
        Some(self.on_route[end as usize] as usize)
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
mod tests {
    use super::*;
    use crate::road::STREET_SPEED;

    #[test]
    fn prefers_faster_route() {
        // 0 ── 1 ── 2 on slow streets, plus a longer but faster detour 0 ── 3 ── 2.
        let mut net = RoadNetwork::default();
        let n0 = net.add_node(0.0, 0.0);
        let n1 = net.add_node(100.0, 0.0);
        let n2 = net.add_node(200.0, 0.0);
        let n3 = net.add_node(100.0, 40.0);
        net.add_road(n0, n1, STREET_SPEED / 2.0);
        net.add_road(n1, n2, STREET_SPEED / 2.0);
        net.add_road(n0, n3, AVENUE_SPEED);
        net.add_road(n3, n2, AVENUE_SPEED);
        net.rebuild_adjacency();

        let mut pf = Pathfinder::default();
        let mut path = Vec::new();
        assert!(pf.find(&net, &[], n0, n2, &mut path));
        let visited: Vec<_> = path.iter().map(|&l| net.lane_to[l as usize]).collect();
        assert_eq!(visited, vec![n3, n2]);
    }

    #[test]
    fn unreachable_returns_false() {
        let mut net = RoadNetwork::default();
        let a = net.add_node(0.0, 0.0);
        let b = net.add_node(10.0, 0.0);
        let c = net.add_node(50.0, 0.0);
        net.add_road(a, b, STREET_SPEED);
        net.rebuild_adjacency();
        let mut path = Vec::new();
        assert!(!Pathfinder::default().find(&net, &[], a, c, &mut path));
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
                    net.add_road(n, n + 1, STREET_SPEED);
                }
                if r < 2 {
                    net.add_road(n, n + 3, STREET_SPEED);
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
        assert!(pf.find(&net, &[], 0, 2, &mut route));
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
        assert!(pf.find(&net, &[], 0, 2, &mut route));
        let mut detour = Vec::new();
        assert_eq!(pf.rejoin(&net, &[], &route, |_| true, 1, &mut detour), None);
        assert_eq!(
            pf.rejoin(&net, &[], &route, |_| false, 64, &mut detour),
            None
        );
    }
}
