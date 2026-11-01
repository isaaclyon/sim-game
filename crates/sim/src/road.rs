//! Road graph.
//!
//! Two-way roads are stored as pairs of directed lanes: road `k` owns lane `2k` (a→b) and lane
//! `2k + 1` (b→a). Lanes are append-only, so a `LaneId` stays valid for the life of the world.
//! Centrelines are straight or quadratic Bézier curves; geometry queries use travel distance.

use std::collections::BTreeSet;

use crate::geometry::{Point, Shape};
use crate::rng::Rng;

pub type NodeId = u32;
pub type LaneId = u32;
/// Index of a two-way road; it owns lanes `2k` and `2k + 1`.
pub type RoadId = u32;

/// Street speed limit, m/s (50 km/h).
pub const STREET_SPEED: f32 = 13.9;
/// Main road speed limit, m/s (72 km/h). Also the fastest limit in the game, which the A*
/// heuristic relies on.
pub const MAIN_SPEED: f32 = 20.0;

/// What a road is for. Local streets reach places; main roads carry the middle of long trips,
/// are faster, and win right of way and signals at junctions (ADR 0017).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[repr(u8)]
pub enum RoadClass {
    #[default]
    Street = 0,
    Main = 1,
    /// Laid by the sim to reach deep lots (ADR 0017).
    Alley = 2,
}

impl RoadClass {
    pub fn from_u8(v: u8) -> Option<Self> {
        match v {
            0 => Some(Self::Street),
            1 => Some(Self::Main),
            2 => Some(Self::Alley),
            _ => None,
        }
    }

    pub fn speed(self) -> f32 {
        match self {
            Self::Street => STREET_SPEED,
            Self::Main => MAIN_SPEED,
            Self::Alley => ALLEY_SPEED,
        }
    }
}
/// Alley speed limit, m/s (30 km/h). The sim lays alleys to reach deep lots (ADR 0016).
pub const ALLEY_SPEED: f32 = 8.3;

#[derive(Default, Clone)]
pub struct RoadNetwork {
    pub node_x: Vec<f32>,
    pub node_y: Vec<f32>,
    pub lane_from: Vec<NodeId>,
    pub lane_to: Vec<NodeId>,
    pub lane_len: Vec<f32>,
    pub lane_speed: Vec<f32>,
    // Per road. Straight roads need no distance table.
    class: Vec<RoadClass>,
    control: Vec<Option<Point>>,
    lengths: Vec<Vec<f32>>,
    // CSR adjacency: lanes leaving node n are out_lanes[out_start[n]..out_start[n + 1]].
    out_start: Vec<u32>,
    out_lanes: Vec<LaneId>,
    roads: BTreeSet<(NodeId, NodeId)>,
    road_active: Vec<bool>,
    /// Whether any active road is a main road; refreshed with the adjacency.
    has_main: bool,
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
    pub fn add_road(&mut self, a: NodeId, b: NodeId, class: RoadClass) -> bool {
        self.add_shape(a, b, None, class)
    }

    /// Adds a quadratic Bézier road with one control point. Refuses invalid geometry.
    pub fn add_curve(&mut self, a: NodeId, b: NodeId, control: Point, class: RoadClass) -> bool {
        self.add_shape(a, b, Some(control), class)
    }

    pub(crate) fn add_shape(
        &mut self,
        a: NodeId,
        b: NodeId,
        control: Option<Point>,
        class: RoadClass,
    ) -> bool {
        let shape = Shape {
            a: (self.node_x[a as usize], self.node_y[a as usize]),
            b: (self.node_x[b as usize], self.node_y[b as usize]),
            control,
        };
        if !shape.valid() || a == b || !self.roads.insert((a.min(b), a.max(b))) {
            return false;
        }
        let lengths = if control.is_some() {
            shape.lengths()
        } else {
            Vec::new()
        };
        let len = lengths
            .last()
            .copied()
            .unwrap_or_else(|| self.distance(a, b));
        self.class.push(class);
        self.control.push(control);
        self.lengths.push(lengths);
        self.road_active.push(true);
        for (from, to) in [(a, b), (b, a)] {
            self.lane_from.push(from);
            self.lane_to.push(to);
            self.lane_len.push(len);
            self.lane_speed.push(class.speed());
        }
        self.adjacency_dirty = true;
        self.version += 1;
        true
    }

    pub fn road_class(&self, road: RoadId) -> RoadClass {
        self.class[road as usize]
    }

    pub fn lane_class(&self, lane: LaneId) -> RoadClass {
        self.class[lane as usize / 2]
    }

    /// Whether any active road is a main road.
    pub fn has_main_roads(&self) -> bool {
        debug_assert!(
            !self.adjacency_dirty,
            "call rebuild_adjacency after editing roads"
        );
        self.has_main
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
    /// `mid`, and a new road of the same class runs on from `mid` to the old far end. Returns
    /// the new road. Ids stay valid, but the old road's lanes are shorter and lead somewhere
    /// else, so cars and routes on them are the caller's to deal with.
    pub fn split_road(&mut self, road: RoadId, mid: NodeId) -> RoadId {
        let shape = self.shape(road);
        let t = shape.nearest((self.node_x[mid as usize], self.node_y[mid as usize]));
        let left = shape.section(0.0, t);
        let right = shape.section(t, 1.0);
        let (a, b) = self.road_ends(road);
        let (fwd, back) = (road as usize * 2, road as usize * 2 + 1);
        self.roads.remove(&(a.min(b), a.max(b)));
        self.roads.insert((a.min(mid), a.max(mid)));
        self.lane_to[fwd] = mid;
        self.lane_from[back] = mid;
        self.control[road as usize] = left.control;
        self.lengths[road as usize] = if left.control.is_some() {
            self.shape(road).lengths()
        } else {
            Vec::new()
        };
        let len = self.lengths[road as usize]
            .last()
            .copied()
            .unwrap_or_else(|| self.distance(a, mid));
        self.lane_len[fwd] = len;
        self.lane_len[back] = len;
        let rest = self.road_active.len() as RoadId;
        let added = self.add_shape(mid, b, right.control, self.class[road as usize]);
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
        let main = self
            .active_roads()
            .any(|r| self.class[r as usize] == RoadClass::Main);
        self.has_main = main;
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
        if self.control[l / 2].is_some() {
            let shape = self.shape(lane / 2);
            let t = self.parameter(lane, dist);
            let p = shape.point(t);
            let (dx, dy) = shape.tangent(t);
            let len = dx.hypot(dy).max(1e-6);
            let side = if lane.is_multiple_of(2) { side } else { -side };
            return (p.0 + dy / len * side, p.1 - dx / len * side);
        }
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

    /// Direction of travel `dist` metres along `lane`, radians from +x.
    pub fn lane_heading(&self, lane: LaneId, dist: f32) -> f32 {
        let l = lane as usize;
        if self.control[l / 2].is_some() {
            let (x, y) = self.shape(lane / 2).tangent(self.parameter(lane, dist));
            return if lane.is_multiple_of(2) {
                y.atan2(x)
            } else {
                (-y).atan2(-x)
            };
        }
        let (a, b) = (self.lane_from[l] as usize, self.lane_to[l] as usize);
        (self.node_y[b] - self.node_y[a]).atan2(self.node_x[b] - self.node_x[a])
    }

    pub(crate) fn shape(&self, road: RoadId) -> Shape {
        let (a, b) = self.road_ends(road);
        Shape {
            a: (self.node_x[a as usize], self.node_y[a as usize]),
            b: (self.node_x[b as usize], self.node_y[b as usize]),
            control: self.control[road as usize],
        }
    }

    fn parameter(&self, lane: LaneId, dist: f32) -> f32 {
        let len = self.lane_len[lane as usize];
        let d = dist.clamp(0.0, len);
        let d = if lane.is_multiple_of(2) { d } else { len - d };
        let table = &self.lengths[lane as usize / 2];
        let i = table.partition_point(|&v| v < d).clamp(1, table.len() - 1);
        let f = (d - table[i - 1]) / (table[i] - table[i - 1]).max(1e-6);
        (i as f32 - 1.0 + f) / (table.len() - 1) as f32
    }

    /// `[min_x, min_y, max_x, max_y]` including each curve's control hull.
    pub fn bounds(&self) -> [f32; 4] {
        let mut b = [f32::MAX, f32::MAX, f32::MIN, f32::MIN];
        for i in 0..self.node_count() {
            b[0] = b[0].min(self.node_x[i]);
            b[1] = b[1].min(self.node_y[i]);
            b[2] = b[2].max(self.node_x[i]);
            b[3] = b[3].max(self.node_y[i]);
        }
        for &(x, y) in self.control.iter().flatten() {
            b[0] = b[0].min(x);
            b[1] = b[1].min(y);
            b[2] = b[2].max(x);
            b[3] = b[3].max(y);
        }
        b
    }

    /// A perfect `cols` × `rows` lattice of junctions `spacing` apart: every street present,
    /// no jitter, no avenues. The cleanest case for traffic and lots.
    pub fn generate_lattice(cols: u32, rows: u32, spacing: f32) -> Self {
        let mut net = Self::default();
        for r in 0..rows {
            for c in 0..cols {
                net.add_node(c as f32 * spacing, r as f32 * spacing);
            }
        }
        for r in 0..rows {
            for c in 0..cols {
                let id = r * cols + c;
                if c + 1 < cols {
                    net.add_road(id, id + 1, RoadClass::Street);
                }
                if r + 1 < rows {
                    net.add_road(id, id + cols, RoadClass::Street);
                }
            }
        }
        net.rebuild_adjacency();
        net
    }

    /// Jittered grid with some streets removed, so it reads as a town rather than a lattice.
    /// Every fifth row and column is a main road (an avenue), which long trips ride.
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
        let class = |i: u32| {
            if is_avenue(i) {
                RoadClass::Main
            } else {
                RoadClass::Street
            }
        };
        for r in 0..rows {
            for c in 0..cols {
                if c + 1 < cols && (is_avenue(r) || rng.unit() > DROP_CHANCE) {
                    net.add_road(id(c, r), id(c + 1, r), class(r));
                }
                if r + 1 < rows && (is_avenue(c) || rng.unit() > DROP_CHANCE) {
                    net.add_road(id(c, r), id(c, r + 1), class(c));
                }
            }
        }
        net.rebuild_adjacency();
        net
    }
}
