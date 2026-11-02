//! Detours: a bounded local search around a blocked lane that rejoins the old route.

use super::{Costs, Open, Pathfinder};
use crate::road::{LaneId, MAIN_SPEED, NodeId, RoadNetwork};

impl Pathfinder {
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
            (net.node_x[i] - tx).hypot(net.node_y[i] - ty) / MAIN_SPEED
        };
        // Detours react to a blocked lane with plain travel times, not the driver's taste.
        let costs = Costs {
            net,
            delay,
            driver: None,
            walk: false,
        };
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
            self.rest[j] = self.rest[j + 1] + costs.lane(route[j + 1]);
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
                let ng = g + costs.lane(lane);
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
}
