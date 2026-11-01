//! Road graph.
//!
//! Two-way roads are stored as pairs of directed lanes: road `k` owns lane `2k` (a→b) and lane
//! `2k + 1` (b→a). Lanes are append-only, so a `LaneId` stays valid for the life of the world.
//! Roads are straight segments for now; curves will replace `lane_point` without changing ids.

use std::collections::BTreeSet;

use crate::rng::Rng;

pub type NodeId = u32;
pub type LaneId = u32;
/// Index of a two-way road; it owns lanes `2k` and `2k + 1`.
pub type RoadId = u32;

/// Street speed limit, m/s (50 km/h).
pub const STREET_SPEED: f32 = 13.9;
/// Avenue speed limit, m/s (72 km/h). Also the fastest limit in the game, which the A*
/// heuristic relies on.
pub const AVENUE_SPEED: f32 = 20.0;

#[derive(Default, Clone)]
pub struct RoadNetwork {
    pub node_x: Vec<f32>,
    pub node_y: Vec<f32>,
    pub lane_from: Vec<NodeId>,
    pub lane_to: Vec<NodeId>,
    pub lane_len: Vec<f32>,
    pub lane_speed: Vec<f32>,
    // CSR adjacency: lanes leaving node n are out_lanes[out_start[n]..out_start[n + 1]].
    out_start: Vec<u32>,
    out_lanes: Vec<LaneId>,
    roads: BTreeSet<(NodeId, NodeId)>,
    road_active: Vec<bool>,
    adjacency_dirty: bool,
    /// Bumped on every topology change so the renderer knows when to re-upload roads.
    pub version: u32,
}

impl RoadNetwork {
    pub fn node_count(&self) -> usize {
        self.node_x.len()
    }

    pub fn lane_count(&self) -> usize {
        self.lane_from.len()
    }

    /// Number of active roads, not allocated ID slots.
    pub fn road_count(&self) -> usize {
        self.roads.len()
    }

    pub fn active_roads(&self) -> impl Iterator<Item = RoadId> + '_ {
        self.road_active
            .iter()
            .enumerate()
            .filter_map(|(r, &active)| active.then_some(r as RoadId))
    }

    /// Tombstones both lanes without reusing their IDs or discarding their geometry.
    pub fn remove_road(&mut self, road: RoadId) -> bool {
        if self.road_active.get(road as usize) != Some(&true) {
            return false;
        }
        self.road_active[road as usize] = false;
        let (a, b) = self.road_ends(road);
        self.roads.remove(&(a.min(b), a.max(b)));
        self.adjacency_dirty = true;
        self.version += 1;
        true
    }

    pub fn add_node(&mut self, x: f32, y: f32) -> NodeId {
        self.node_x.push(x);
        self.node_y.push(y);
        self.adjacency_dirty = true;
        self.version += 1;
        (self.node_x.len() - 1) as NodeId
    }

    /// Linear scan. Fine for click handling; swap in a spatial hash once it shows up in profiles.
    pub fn nearest_node(&self, x: f32, y: f32, max_dist: f32) -> Option<NodeId> {
        let mut best = None;
        let mut best_d2 = max_dist * max_dist;
        for i in 0..self.node_count() {
            let (dx, dy) = (self.node_x[i] - x, self.node_y[i] - y);
            let d2 = dx * dx + dy * dy;
            if d2 <= best_d2 {
                best_d2 = d2;
                best = Some(i as NodeId);
            }
        }
        best
    }

    /// Adds a two-way road. Returns false for self-loops and duplicates.
    pub fn add_road(&mut self, a: NodeId, b: NodeId, speed: f32) -> bool {
        if a == b || !self.roads.insert((a.min(b), a.max(b))) {
            return false;
        }
        self.road_active.push(true);
        let len = self.distance(a, b);
        for (from, to) in [(a, b), (b, a)] {
            self.lane_from.push(from);
            self.lane_to.push(to);
            self.lane_len.push(len);
            self.lane_speed.push(speed);
        }
        self.adjacency_dirty = true;
        self.version += 1;
        true
    }

    pub fn has_road(&self, a: NodeId, b: NodeId) -> bool {
        self.roads.contains(&(a.min(b), a.max(b)))
    }

    /// The nodes road `road` runs between, in the direction of its lane `2k`.
    pub fn road_ends(&self, road: RoadId) -> (NodeId, NodeId) {
        let lane = road as usize * 2;
        (self.lane_from[lane], self.lane_to[lane])
    }

    /// Splits `road` at `mid`, a new node on it. The road keeps its id and lanes but now ends at
    /// `mid`, and a new road with the same speed runs on from `mid` to the old far end. Returns
    /// the new road. Ids stay valid, but the old road's lanes are shorter and lead somewhere
    /// else, so cars and routes on them are the caller's to deal with.
    pub fn split_road(&mut self, road: RoadId, mid: NodeId) -> RoadId {
        let (a, b) = self.road_ends(road);
        let (fwd, back) = (road as usize * 2, road as usize * 2 + 1);
        self.roads.remove(&(a.min(b), a.max(b)));
        self.roads.insert((a.min(mid), a.max(mid)));
        self.lane_to[fwd] = mid;
        self.lane_from[back] = mid;
        let len = self.distance(a, mid);
        self.lane_len[fwd] = len;
        self.lane_len[back] = len;
        let rest = self.road_active.len() as RoadId;
        let added = self.add_road(mid, b, self.lane_speed[fwd]);
        debug_assert!(added, "mid is a new node");
        rest
    }

    pub fn distance(&self, a: NodeId, b: NodeId) -> f32 {
        let (a, b) = (a as usize, b as usize);
        (self.node_x[b] - self.node_x[a]).hypot(self.node_y[b] - self.node_y[a])
    }

    /// Rebuilds the CSR adjacency if roads changed since the last call. Cheap no-op otherwise.
    pub fn rebuild_adjacency(&mut self) {
        if !self.adjacency_dirty {
            return;
        }
        let n = self.node_count();
        let mut start = vec![0u32; n + 1];
        for (lane, &from) in self.lane_from.iter().enumerate() {
            if self.road_active[lane / 2] {
                start[from as usize + 1] += 1;
            }
        }
        for i in 0..n {
            start[i + 1] += start[i];
        }
        let mut fill = start.clone();
        let mut out = vec![0; self.road_count() * 2];
        for (lane, &from) in self.lane_from.iter().enumerate() {
            if !self.road_active[lane / 2] {
                continue;
            }
            let slot = &mut fill[from as usize];
            out[*slot as usize] = lane as LaneId;
            *slot += 1;
        }
        self.out_start = start;
        self.out_lanes = out;
        self.adjacency_dirty = false;
    }

    pub fn out_lanes(&self, node: NodeId) -> &[LaneId] {
        debug_assert!(
            !self.adjacency_dirty,
            "call rebuild_adjacency after editing roads"
        );
        let n = node as usize;
        &self.out_lanes[self.out_start[n] as usize..self.out_start[n + 1] as usize]
    }

    /// World position `dist` metres along `lane`, shifted `side` metres to the right of travel.
    pub fn lane_point(&self, lane: LaneId, dist: f32, side: f32) -> (f32, f32) {
        let l = lane as usize;
        let (a, b) = (self.lane_from[l] as usize, self.lane_to[l] as usize);
        let (dx, dy) = (
            self.node_x[b] - self.node_x[a],
            self.node_y[b] - self.node_y[a],
        );
        let len = self.lane_len[l].max(1e-3);
        let t = (dist / len).clamp(0.0, 1.0);
        let (nx, ny) = (dx / len, dy / len);
        // (ny, -nx) is the right-hand normal of the travel direction.
        (
            self.node_x[a] + dx * t + ny * side,
            self.node_y[a] + dy * t - nx * side,
        )
    }

    /// Direction of travel along `lane`, radians from +x.
    pub fn lane_heading(&self, lane: LaneId) -> f32 {
        let l = lane as usize;
        let (a, b) = (self.lane_from[l] as usize, self.lane_to[l] as usize);
        (self.node_y[b] - self.node_y[a]).atan2(self.node_x[b] - self.node_x[a])
    }

    /// `[min_x, min_y, max_x, max_y]` over all nodes.
    pub fn bounds(&self) -> [f32; 4] {
        let mut b = [f32::MAX, f32::MAX, f32::MIN, f32::MIN];
        for i in 0..self.node_count() {
            b[0] = b[0].min(self.node_x[i]);
            b[1] = b[1].min(self.node_y[i]);
            b[2] = b[2].max(self.node_x[i]);
            b[3] = b[3].max(self.node_y[i]);
        }
        b
    }

    /// Jittered grid with some streets removed, so it reads as a town rather than a lattice.
    /// Every fifth row and column is an avenue, which gives pathfinding something to prefer.
    pub fn generate_grid(cols: u32, rows: u32, spacing: f32, rng: &mut Rng) -> Self {
        const DROP_CHANCE: f32 = 0.12;
        let mut net = Self::default();
        let jitter = spacing * 0.25;
        for r in 0..rows {
            for c in 0..cols {
                let x = c as f32 * spacing + rng.range(-jitter, jitter);
                let y = r as f32 * spacing + rng.range(-jitter, jitter);
                net.add_node(x, y);
            }
        }
        let id = |c: u32, r: u32| r * cols + c;
        let is_avenue = |i: u32| i.is_multiple_of(5);
        for r in 0..rows {
            for c in 0..cols {
                if c + 1 < cols && (is_avenue(r) || rng.unit() > DROP_CHANCE) {
                    let speed = if is_avenue(r) {
                        AVENUE_SPEED
                    } else {
                        STREET_SPEED
                    };
                    net.add_road(id(c, r), id(c + 1, r), speed);
                }
                if r + 1 < rows && (is_avenue(c) || rng.unit() > DROP_CHANCE) {
                    let speed = if is_avenue(c) {
                        AVENUE_SPEED
                    } else {
                        STREET_SPEED
                    };
                    net.add_road(id(c, r), id(c, r + 1), speed);
                }
            }
        }
        net.rebuild_adjacency();
        net
    }
}
