//! Water in the world (ADR 0052): placing and removing pumps and towers, tower levels, and
//! dense buildings that lose people while dry.
//!
//! Pieces, tower levels, the last update step and dry-since ticks are stored. Networks follow
//! roads and are derived like power grids (`power::grid`), and so is the allocation
//! (`alloc`), kept current between ticks so a loaded world reads the same.

mod alloc;
mod dry;
mod snapshot;
#[cfg(test)]
mod tests;

use super::World;
use super::money::water_price;
use crate::clock::clock_seconds;
use crate::power::Footprint;
use crate::water::{STEP_S, TOWER_CAPACITY, WaterKind};

pub use alloc::WaterStats;
pub(super) use alloc::{WaterCache, water_demand};
pub use snapshot::{WATER_BUILDING_STRIDE, WATER_PIECE_STRIDE};

impl World {
    /// Where a pump or tower asked for at the cursor would stand: beside the nearest road in
    /// reach, facing it, like coal (ADR 0042). `None` off the map.
    pub fn water_spot(&self, kind: WaterKind, x: f32, y: f32) -> Option<Footprint> {
        self.roadside_spot(kind.half_size(), x, y)
    }

    /// Whether `water_spot` is beside a road and clear of roads, plants, pumps and towers.
    pub fn water_fits(&self, kind: WaterKind, x: f32, y: f32) -> bool {
        self.water_spot(kind, x, y)
            .is_some_and(|at| self.spot_fits(at, true))
    }

    /// Places a pump or tower at its `water_spot`, clearing its land like a plant. Refused
    /// when it does not fit or the city cannot pay for it.
    pub(super) fn place_water(&mut self, kind: WaterKind, x: f32, y: f32) -> bool {
        let Some(at) = self.water_spot(kind, x, y) else {
            return false;
        };
        let cost = water_price(kind).0;
        if !self.spot_fits(at, true) || !self.can_afford(cost) {
            return false;
        }
        self.treasury.balance -= cost;
        self.water.add(kind, at);
        self.claim_land(at);
        true
    }

    /// Removes the pump or tower under the point; a tower's water goes with it.
    pub(super) fn bulldoze_water(&mut self, x: f32, y: f32) -> bool {
        if !(x.is_finite() && y.is_finite()) {
            return false;
        }
        let Some(p) = self.water.piece_at(x, y, 0.0) else {
            return false;
        };
        self.water.remove(p);
        true
    }

    /// The clock step water updates count in.
    pub(super) fn water_step(&self) -> u64 {
        clock_seconds(self.totals.tick, self.cfg.dt) / STEP_S
    }

    /// Called after each tick. On each new clock step, towers take their network's spare
    /// pump output or give what its buildings drew from them, then dry buildings lose people.
    pub(super) fn tick_water(&mut self) {
        let (last, step) = (self.water.step, self.water_step());
        if step <= last {
            return;
        }
        self.ensure_water();
        let steps = i64::try_from(step - last).unwrap_or(i64::MAX);
        let c = &self.water_cache;
        let mut flow: Vec<i64> = (0..c.left.len())
            .map(|g| {
                let drawn = i64::from(c.stored[g] - c.store_left[g]);
                (i64::from(c.left[g]) - drawn).saturating_mul(steps)
            })
            .collect();
        for p in 0..self.water.active.len() {
            if !self.water.active[p] || self.water.kind[p] != WaterKind::Tower {
                continue;
            }
            let g = self.water_cache.nets.piece[p] as usize;
            let level = &mut self.water.level[p];
            let moved = if flow[g] < 0 {
                -(-flow[g]).min(i64::from(*level))
            } else {
                flow[g].min(i64::from(TOWER_CAPACITY - *level))
            };
            *level = (i64::from(*level) + moved) as u32;
            flow[g] -= moved;
        }
        self.water.step = step;
        self.ensure_water();
        self.dry_departures(last, step);
    }
}
