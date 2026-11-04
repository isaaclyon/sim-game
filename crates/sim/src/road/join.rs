//! Straight joins (ADR 0040): two straight roads of one class meeting end to end in a line, with
//! no other road at that node, become one road. Otherwise the node would be a junction nobody
//! can see, and it would pull new roads off the line they were drawn on.

use super::{LaneId, NodeId, RoadId, RoadNetwork};

/// Roads closer to one line than this (sine of the bend, about 0.06°) join.
const STRAIGHT_SIN: f32 = 1e-3;

/// One join: `kept` now also runs where `removed` did. `lanes` maps each lane of `removed` to
/// the lane of `kept` that now leaves or arrives at the far end the same way.
pub(crate) struct Joined {
    pub kept: RoadId,
    pub removed: RoadId,
    pub lanes: [(LaneId, LaneId); 2],
}

impl RoadNetwork {
    /// Nodes where exactly two active roads meet, ascending; `join_straight` decides the rest.
    pub(crate) fn two_road_nodes(&self) -> Vec<NodeId> {
        let mut count = vec![0u8; self.node_count()];
        for road in self.active_roads() {
            let (a, b) = self.road_ends(road);
            for n in [a, b] {
                count[n as usize] = count[n as usize].saturating_add(1);
            }
        }
        (0..self.node_count() as NodeId)
            .filter(|&n| count[n as usize] == 2)
            .collect()
    }

    /// Joins the two roads at `n` into the lower-numbered one if they are straight, of one
    /// class and in line. The other is removed and `n` is left unused. Cars on either road are
    /// the caller's to deal with, as for `split_road`.
    pub(crate) fn join_straight(&mut self, n: NodeId) -> Option<Joined> {
        let at: Vec<RoadId> = self
            .active_roads()
            .filter(|&r| {
                let (a, b) = self.road_ends(r);
                a == n || b == n
            })
            .collect();
        let &[kept, removed] = &at[..] else {
            return None;
        };
        if self.control[kept as usize].is_some()
            || self.control[removed as usize].is_some()
            || self.class[kept as usize] != self.class[removed as usize]
        {
            return None;
        }
        let far = |r: RoadId| {
            let (a, b) = self.road_ends(r);
            if a == n { b } else { a }
        };
        let (p, q) = (far(kept), far(removed));
        let pos = |m: NodeId| (self.node_x[m as usize], self.node_y[m as usize]);
        let (o, u, v) = (pos(n), pos(p), pos(q));
        let (u, v) = ((u.0 - o.0, u.1 - o.1), (v.0 - o.0, v.1 - o.1));
        let scale = u.0.hypot(u.1) * v.0.hypot(v.1);
        if p == q
            || self.has_road(p, q)
            || u.0 * v.0 + u.1 * v.1 >= 0.0
            || (u.0 * v.1 - u.1 * v.0).abs() > STRAIGHT_SIN * scale
        {
            return None;
        }
        let gone_in = self.lane_into(removed, q);
        self.remove_road(removed);
        let (fwd, back) = (2 * kept as usize, 2 * kept as usize + 1);
        if self.lane_from[fwd] == n {
            self.lane_from[fwd] = q;
            self.lane_to[back] = q;
        } else {
            self.lane_to[fwd] = q;
            self.lane_from[back] = q;
        }
        self.roads.remove(&(p.min(n), p.max(n)));
        self.roads.insert((p.min(q), p.max(q)));
        let len = self.distance(p, q);
        self.lane_len[fwd] = len;
        self.lane_len[back] = len;
        self.adjacency_dirty = true;
        self.version += 1;
        let kept_in = self.lane_into(kept, q);
        Some(Joined {
            kept,
            removed,
            lanes: [(gone_in, kept_in), (gone_in ^ 1, kept_in ^ 1)],
        })
    }

    /// The nearest node within `max_dist` that an active road still uses. Joins and bulldozing
    /// leave unused nodes behind, as IDs are append-only; those must not pull new roads.
    pub fn nearest_junction(&self, x: f32, y: f32, max_dist: f32) -> Option<NodeId> {
        let mut best = None;
        let mut best_d2 = max_dist * max_dist;
        for i in 0..self.node_count() {
            let (dx, dy) = (self.node_x[i] - x, self.node_y[i] - y);
            let d2 = dx * dx + dy * dy;
            // The road index is checked only for nodes that would win, so this stays cheap.
            if d2 <= best_d2 && self.node_used(i as NodeId) {
                best_d2 = d2;
                best = Some(i as NodeId);
            }
        }
        best
    }

    /// The lane of `road` that arrives at its end `m`.
    fn lane_into(&self, road: RoadId, m: NodeId) -> LaneId {
        if self.lane_to[2 * road as usize] == m {
            2 * road
        } else {
            2 * road + 1
        }
    }

    /// Whether any active road ends at `n`. Valid before the adjacency is rebuilt.
    fn node_used(&self, n: NodeId) -> bool {
        self.roads.range((n, 0)..=(n, NodeId::MAX)).next().is_some()
            || self.roads.iter().any(|&(_, b)| b == n)
    }
}
