//! A* over the lane graph. Cost is free-flow travel time, so routes prefer fast avenues.

use std::cmp::Ordering;
use std::collections::BinaryHeap;

use crate::road::{AVENUE_SPEED, LaneId, NodeId, RoadNetwork};

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
    pub fn find(
        &mut self,
        net: &RoadNetwork,
        from: NodeId,
        to: NodeId,
        out: &mut Vec<LaneId>,
    ) -> bool {
        out.clear();
        let n = net.node_count();
        if self.stamp.len() < n {
            self.g.resize(n, 0.0);
            self.via.resize(n, 0);
            self.stamp.resize(n, 0);
        }
        self.generation = self.generation.wrapping_add(1);
        if self.generation == 0 {
            self.stamp.fill(0);
            self.generation = 1;
        }

        let (tx, ty) = (net.node_x[to as usize], net.node_y[to as usize]);
        // Straight-line distance at the fastest speed limit never overestimates, so A* stays optimal.
        let h = |node: NodeId| {
            let i = node as usize;
            (net.node_x[i] - tx).hypot(net.node_y[i] - ty) / AVENUE_SPEED
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
                let ng = g + net.lane_len[l] / net.lane_speed[l];
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
        assert!(pf.find(&net, n0, n2, &mut path));
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
        assert!(!Pathfinder::default().find(&net, a, c, &mut path));
    }
}
