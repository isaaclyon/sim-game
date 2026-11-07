//! Power in the world (ADR 0041): placing and removing plants and lines, the land they take,
//! and which buildings are powered.
//!
//! Plants and lines are stored state. Everything else here is derived from stored state and
//! the tick, so it is neither saved nor hashed: grids from roads and power, each building's
//! power demand from its footprint, and the allocation. The allocation is recomputed whenever
//! its inputs change and kept current between ticks, so a loaded world reads the same.

mod alloc;
mod snapshot;
#[cfg(test)]
mod tests;

use super::World;
use super::money::plant_price;
use crate::block::Rect;
use crate::building::BuildingId;
use crate::frontage::frontage;
use crate::geometry::{Point, dist, project};
use crate::lot::LotId;
use crate::power::{
    COAL_SETBACK, COAL_SNAP, Footprint, LINE_REACH, PLANT_REACH, PlantKind, segment_hits_ring,
};
use crate::snap::SNAP_RADIUS;
use crate::zone::Zone;

pub use alloc::PowerStats;
pub(super) use alloc::{PowerCache, power_demand};
pub use snapshot::{PLANT_STRIDE, POWER_BUILDING_STRIDE};

/// A plant footprint keeps this far from any road centreline, metres.
const ROAD_CLEARANCE: f32 = 6.0;
/// A pole joining a road stands this far inside its kerb, metres.
const SIDEWALK_POLE: f32 = 1.0;

fn on_map(x: f32, y: f32) -> bool {
    x.is_finite() && y.is_finite() && x.abs() < 1e6 && y.abs() < 1e6
}

/// What a line end joins once snapped.
#[derive(Clone, Copy, PartialEq)]
enum End {
    Free,
    Joined,
}

impl World {
    /// Where a plant of `kind` asked for at the cursor would stand. Coal snaps beside the
    /// nearest road in reach (`roadside_spot`). Others stand where asked, facing +y. `None`
    /// off the map.
    pub fn plant_spot(&self, kind: PlantKind, x: f32, y: f32) -> Option<Footprint> {
        if !on_map(x, y) {
            return None;
        }
        let half = kind.half_size();
        if kind == PlantKind::Coal {
            return self.roadside_spot(half, x, y);
        }
        Some(Footprint {
            centre: (x, y),
            front: (0.0, 1.0),
            half,
        })
    }

    /// Where a roadside piece (coal, pump or tower; ADR 0042) asked for at the cursor would
    /// stand: its front just outside the kerb of the nearest road in reach and facing it,
    /// sliding along it with the cursor; where asked, facing +y, with no road in reach.
    pub(in crate::world) fn roadside_spot(&self, half: f32, x: f32, y: f32) -> Option<Footprint> {
        if !on_map(x, y) {
            return None;
        }
        Some(match frontage(&self.roads, (x, y), COAL_SNAP) {
            Some(f) => Footprint {
                centre: f.out(f.kerb + COAL_SETBACK + half),
                front: f.front,
                half,
            },
            None => Footprint {
                centre: (x, y),
                front: (0.0, 1.0),
                half,
            },
        })
    }

    /// Whether `plant_spot` is clear of roads and other plants, and for coal, beside a road.
    pub fn plant_fits(&self, kind: PlantKind, x: f32, y: f32) -> bool {
        self.plant_spot(kind, x, y)
            .is_some_and(|at| self.spot_fits(at, kind == PlantKind::Coal))
    }

    /// Whether a plant, pump or tower at `at` is clear of roads, plants, pumps and towers,
    /// and, when it must stand `beside_road`, within reach of one.
    pub(in crate::world) fn spot_fits(&self, at: Footprint, beside_road: bool) -> bool {
        let near_road = |margin| {
            self.roads.active_roads().any(|road| {
                self.roads
                    .shape(road)
                    .segments()
                    .any(|(_, a, _, b)| at.hits_segment(a, b, margin))
            })
        };
        let overlaps = self
            .power
            .active_plants()
            .map(|p| self.power.footprint(p))
            .chain(self.water.active_pieces().map(|p| self.water.footprint(p)))
            .any(|other| other.overlaps(at));
        !overlaps && !near_road(ROAD_CLEARANCE) && (!beside_road || near_road(PLANT_REACH))
    }

    /// Places a plant at its `plant_spot`, unzoning the land under it and removing the
    /// buildings it covers. Refused when the city cannot pay for it (ADR 0052).
    pub(super) fn place_plant(&mut self, kind: PlantKind, x: f32, y: f32) -> bool {
        let Some(at) = self.plant_spot(kind, x, y) else {
            return false;
        };
        let cost = plant_price(kind).0;
        if !self.spot_fits(at, kind == PlantKind::Coal) || !self.can_afford(cost) {
            return false;
        }
        self.treasury.balance -= cost;
        self.power.add_plant(kind, at);
        self.claim_land(at);
        true
    }

    /// Unzones the land under a new plant, pump or tower and removes the buildings it covers.
    pub(in crate::world) fn claim_land(&mut self, at: Footprint) {
        self.clear_buildings(|ring| at.hits_ring(ring));
        let bounds = at.bounds();
        let under: Vec<u32> = self
            .zones
            .cells_in(bounds)
            .filter(|&c| at.contains(self.zones.centre(c), 0.0))
            .collect();
        let changed = self.zones.set(&under, Zone::None);
        if !changed.is_empty() {
            self.after_zoning(bounds as Rect, &changed);
        }
    }

    /// Where a line end at `(x, y)` goes: onto a line end, onto the terminal of a plant it
    /// points into, onto the sidewalk of a road in reach on the side facing `toward` (the
    /// line's other end), or free.
    fn snap_line_end(&self, x: f32, y: f32, toward: Point) -> (Point, End) {
        let p = (x, y);
        let mut best: Option<(f32, Point)> = None;
        for l in self.power.active_lines() {
            let (a, b) = self.power.line(l);
            for end in [a, b] {
                let d = dist(end, p);
                if d <= LINE_REACH && best.is_none_or(|(old, _)| d < old) {
                    best = Some((d, end));
                }
            }
        }
        if let Some((_, end)) = best {
            return (end, End::Joined);
        }
        if let Some(plant) = self.power.plant_at(x, y, LINE_REACH) {
            return (self.power.terminal(plant), End::Joined);
        }
        match frontage(&self.roads, p, SNAP_RADIUS).map(|f| f.facing(toward)) {
            // On the sidewalk, inside the kerb, so the grid joins it to this road.
            Some(f) => (f.out(f.kerb - SIDEWALK_POLE), End::Joined),
            None => (p, End::Free),
        }
    }

    /// The line `BuildLine` would build: both ends snapped, or `None` if refused.
    fn plan_line(&self, ax: f32, ay: f32, bx: f32, by: f32) -> Option<[(Point, End); 2]> {
        if ![ax, ay, bx, by]
            .iter()
            .all(|v| v.is_finite() && v.abs() < 1e6)
        {
            return None;
        }
        let ends = [
            self.snap_line_end(ax, ay, (bx, by)),
            self.snap_line_end(bx, by, (ax, ay)),
        ];
        (dist(ends[0].0, ends[1].0) >= 4.0).then_some(ends)
    }

    /// Builds a transmission line, removing the buildings whose footprints it crosses.
    pub(super) fn build_line(&mut self, ax: f32, ay: f32, bx: f32, by: f32) -> bool {
        let Some([(a, _), (b, _)]) = self.plan_line(ax, ay, bx, by) else {
            return false;
        };
        if !self.power.add_line(a, b) {
            return false;
        }
        self.clear_buildings(|ring| segment_hits_ring(a, b, ring));
        true
    }

    /// The plant under the point, otherwise the nearest line within reach.
    fn power_target(&self, x: f32, y: f32) -> Option<Result<u32, u32>> {
        if !(x.is_finite() && y.is_finite()) {
            return None;
        }
        if let Some(p) = self.power.plant_at(x, y, 0.0) {
            return Some(Ok(p));
        }
        let mut best = None;
        let mut reach = LINE_REACH;
        for l in self.power.active_lines() {
            let (a, b) = self.power.line(l);
            let d = dist(crate::geometry::lerp(a, b, project(a, b, (x, y))), (x, y));
            if d < reach {
                reach = d;
                best = Some(Err(l));
            }
        }
        best
    }

    /// Removes the plant or line under the point. Zones a plant cleared stay cleared.
    pub(super) fn bulldoze_power(&mut self, x: f32, y: f32) -> bool {
        match self.power_target(x, y) {
            Some(Ok(p)) => self.power.remove_plant(p),
            Some(Err(l)) => self.power.remove_line(l),
            None => return false,
        }
        true
    }

    /// Removes every standing building whose footprint `hit` says is in the way, as a road
    /// edit does. The caller's command then reconciles closures and residents.
    pub(in crate::world) fn clear_buildings(&mut self, hit: impl Fn(&[Point]) -> bool) {
        let doomed: Vec<BuildingId> = self
            .buildings
            .active_buildings()
            .filter(|&b| hit(self.buildings.footprint(b)))
            .collect();
        for b in doomed {
            self.buildings.remove(
                b,
                &self.lots,
                &self.zones,
                self.totals.tick,
                self.cfg.dt,
                self.cfg.seed,
            );
        }
    }

    /// Whether a plant, line, pump or tower stands on the lot, so nothing new is built there.
    pub(super) fn lot_blocked(&mut self, lot: LotId) -> bool {
        let key = (self.lots.version, self.power.version, self.water.version);
        if self.power_cache.blocked_key != Some(key) {
            self.power_cache.blocked = (0..self.lots.len() as LotId)
                .map(|l| self.lots.active[l as usize] && self.blocks_lot(l))
                .collect();
            self.power_cache.blocked_key = Some(key);
        }
        self.power_cache.blocked.get(lot as usize) == Some(&true)
    }

    fn blocks_lot(&self, lot: LotId) -> bool {
        let ring = self.lots.outline(lot);
        let plant = self
            .power
            .active_plants()
            .any(|p| self.power.footprint(p).hits_ring(ring));
        let piece = self
            .water
            .active_pieces()
            .any(|p| self.water.footprint(p).hits_ring(ring));
        plant
            || piece
            || self.power.active_lines().any(|l| {
                let (a, b) = self.power.line(l);
                segment_hits_ring(a, b, ring)
            })
    }
}
