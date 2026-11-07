//! Coverage (ADR 0053): road distance from each station's junction, which buildings that
//! covers, and the land value covered buildings gain. Derived, never saved or hashed.

use std::cmp::Reverse;
use std::collections::BinaryHeap;

use super::{COVER_M, LAND_BOOST, STATION_HALF};
use crate::building::BuildingId;
use crate::frontage::frontage;
use crate::pollution::Sources;
use crate::road::{NodeId, RoadNetwork};
use crate::world::World;

/// A station joins the road its front faces within this many metres of its front edge.
const STATION_ROAD_REACH: f32 = 20.0;

#[derive(Default)]
pub(super) struct Cover {
    /// Roads and fire versions `dist` was computed for.
    dist_key: Option<(u32, u32)>,
    /// Per station ID: its junction, if it stands beside a road.
    pub(super) node: Vec<Option<NodeId>>,
    /// Per station ID, per node: metres along roads from the station's junction.
    pub(super) dist: Vec<Vec<f32>>,
    covered_key: Option<(u32, u32, u32, u32)>,
    /// Per building ID: within `COVER_M` of some station.
    pub(super) covered: Vec<bool>,
}

/// Shortest driving distance from `from` to every node, following lane directions.
fn distances(roads: &RoadNetwork, from: NodeId) -> Vec<f32> {
    let mut dist = vec![f32::INFINITY; roads.node_count()];
    let mut heap = BinaryHeap::new();
    dist[from as usize] = 0.0;
    heap.push(Reverse((0f32.to_bits(), from)));
    while let Some(Reverse((bits, n))) = heap.pop() {
        let d = f32::from_bits(bits);
        if d > dist[n as usize] || d > COVER_M {
            continue;
        }
        for &lane in roads.out_lanes(n) {
            let to = roads.lane_to[lane as usize];
            let next = d + roads.lane_len[lane as usize];
            if next < dist[to as usize] {
                dist[to as usize] = next;
                // Non-negative floats order like their bits.
                heap.push(Reverse((next.to_bits(), to)));
            }
        }
    }
    dist
}

impl World {
    /// The junction of the road station `s` faces: the nearer end of that road.
    fn station_node(&self, s: u32) -> Option<NodeId> {
        let at = self.fire.footprint(s);
        let front = (
            at.centre.0 + at.front.0 * STATION_HALF,
            at.centre.1 + at.front.1 * STATION_HALF,
        );
        let f = frontage(&self.roads, front, STATION_ROAD_REACH)?;
        let lane = f.road as usize * 2;
        let (a, z) = (self.roads.lane_from[lane], self.roads.lane_to[lane]);
        let far = |n: NodeId| {
            let n = n as usize;
            (self.roads.node_x[n] - f.on.0).hypot(self.roads.node_y[n] - f.on.1)
        };
        Some(if far(a) <= far(z) { a } else { z })
    }

    /// Recomputes distances when roads or stations changed, and coverage when they, lots or
    /// buildings did.
    pub(super) fn ensure_cover(&mut self) {
        let dist_key = (self.roads.version, self.fire.version);
        if self.fire.cover.dist_key != Some(dist_key) {
            self.roads.rebuild_adjacency();
            let stations = self.fire.station_active.len() as u32;
            let node: Vec<_> = (0..stations)
                .map(|s| {
                    self.fire.station_active[s as usize]
                        .then(|| self.station_node(s))
                        .flatten()
                })
                .collect();
            let dist = node
                .iter()
                .map(|n| n.map_or_else(Vec::new, |n| distances(&self.roads, n)))
                .collect();
            let c = &mut self.fire.cover;
            (c.node, c.dist, c.dist_key) = (node, dist, Some(dist_key));
            c.covered_key = None;
        }
        let key = (
            self.roads.version,
            self.fire.version,
            self.lots.version,
            self.buildings.version,
        );
        if self.fire.cover.covered_key == Some(key) {
            return;
        }
        let covered = (0..self.buildings.active.len() as BuildingId)
            .map(|b| self.nearest_station(b).is_some())
            .collect();
        let c = &mut self.fire.cover;
        (c.covered, c.covered_key) = (covered, Some(key));
    }

    /// The covering station nearest building `b` by road, with its distance, among stations
    /// `ok` accepts. Ties keep the lower station ID.
    pub(super) fn nearest_station_where(
        &self,
        b: BuildingId,
        ok: impl Fn(u32) -> bool,
    ) -> Option<(u32, f32)> {
        let node = self.building_node(b)? as usize;
        let c = &self.fire.cover;
        let mut best: Option<(u32, f32)> = None;
        for (s, dist) in c.dist.iter().enumerate() {
            let d = dist.get(node).copied().unwrap_or(f32::INFINITY);
            if d <= COVER_M && ok(s as u32) && best.is_none_or(|(_, old)| d < old) {
                best = Some((s as u32, d));
            }
        }
        best
    }

    fn nearest_station(&self, b: BuildingId) -> Option<(u32, f32)> {
        self.nearest_station_where(b, |_| true)
    }

    /// Whether building `b` is covered, as of the last `ensure_cover`.
    pub fn building_covered(&self, b: BuildingId) -> bool {
        self.fire.cover.covered.get(b as usize) == Some(&true)
    }

    /// Adds the land value boost of covered buildings to the cells of their lots.
    pub(in crate::world) fn fire_boost(&self, src: &mut Sources) {
        let cols = self.zones.cols;
        for b in self.buildings.active_buildings() {
            if !self.building_covered(b) {
                continue;
            }
            let lot = self.buildings.lot[b as usize];
            for &cell in self.lots.cells(lot) {
                let c = self.pollution.cell_of_zone_cell(cell, cols);
                src.boost[c] = LAND_BOOST;
            }
        }
    }

    /// Whether road `r` has an end within reach of some station.
    pub(super) fn road_covered(&self, r: u32) -> bool {
        let lane = r as usize * 2;
        let ends = [self.roads.lane_from[lane], self.roads.lane_to[lane]];
        self.fire.cover.dist.iter().any(|dist| {
            ends.iter()
                .any(|&n| dist.get(n as usize).is_some_and(|&d| d <= COVER_M))
        })
    }
}
