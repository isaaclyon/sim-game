//! Who gets power (ADR 0041). Starter power goes to the oldest buildings first, wherever they
//! stand, until one does not fit. Then each grid serves the rest oldest first until one does not
//! fit; that building and every newer one on the grid are unpowered.

use serde::{Deserialize, Serialize};

use super::super::World;
use super::super::residents::{capacity, ring_area};
use crate::building::BuildingId;
use crate::clock::clock_seconds;
use crate::lot::LotId;
use crate::power::{OUTPUT_STEP_S, STARTER_POWER, grid};
use crate::zone::Zone;

/// The town's power, for the readout beside the demand bars.
#[derive(Clone, Copy, Default, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PowerStats {
    /// What plants produce now on grids with at least one standing building, not counting
    /// starter power. A plant no building can draw from adds nothing.
    pub supply: u32,
    pub starter: u32,
    /// Demand of the powered buildings.
    pub used: u32,
    /// Demand of every standing building.
    pub demand: u32,
    pub unpowered: u32,
    /// Scheduled lots that last waited for power, not demand; saved with their schedules.
    pub waiting: u32,
}

/// The derived allocation and its inputs' versions. Never saved: rebuilt on first use.
#[derive(Default)]
pub(in crate::world) struct PowerCache {
    /// Roads, power, buildings and lots versions and the output step the allocation is for.
    key: Option<(u32, u32, u32, u32, u64)>,
    grids_key: Option<(u32, u32)>,
    pub(super) grids: grid::Grids,
    /// Per building ID.
    pub(super) demand: Vec<u32>,
    pub(super) grid: Vec<u32>,
    pub(super) powered: Vec<bool>,
    /// Per grid: what its plants produce now, and what is left after the oldest buildings.
    pub(super) supply: Vec<u32>,
    left: Vec<u32>,
    open: Vec<bool>,
    starter_left: u32,
    starter_open: bool,
    /// Per plant ID.
    pub(super) output: Vec<u32>,
    /// Bumped whenever the allocation may have changed, so the frontend knows to redraw.
    pub(super) version: u32,
    pub(super) blocked_key: Option<(u32, u32, u32)>,
    pub(super) blocked: Vec<bool>,
}

/// Which pool would power a new building.
#[derive(Clone, Copy)]
pub(in crate::world) enum Source {
    Starter,
    Grid(u32),
}

/// A building's power demand: one per resident place and per job, two per industrial job.
pub(in crate::world) fn power_demand(
    zone: Zone,
    ring: &[crate::geometry::Point],
    height: f32,
) -> u32 {
    let c = capacity(zone, ring_area(ring), height);
    let heavy = matches!(zone, Zone::IndustrialLow | Zone::IndustrialHigh);
    c.homes * u32::from(c.home_size) + c.jobs * if heavy { 2 } else { 1 }
}

impl World {
    /// Whether starter power or the building's grid covers it. Removed buildings read false.
    pub fn building_powered(&self, b: BuildingId) -> bool {
        self.power_cache.powered.get(b as usize) == Some(&true)
    }

    /// New homes and jobs fill only in open, powered buildings with water if they need it
    /// (ADR 0023, 0041, 0052).
    pub(in crate::world) fn building_fills_vacancies(&self, b: BuildingId) -> bool {
        self.building_accepts_assignments(b) && self.building_powered(b) && self.building_watered(b)
    }

    /// The network in `nodes` (per road node) of the lot's road; `u32::MAX` once it is gone.
    pub(in crate::world) fn lot_network(&self, lot: LotId, nodes: &[u32]) -> u32 {
        // A service building (ADR 0055) stands on no lot and draws nothing.
        let Some(&road) = self.lots.road.get(lot as usize) else {
            return u32::MAX;
        };
        if !self.roads.is_road_active(road) {
            return u32::MAX;
        }
        let node = self.roads.lane_from[road as usize * 2] as usize;
        nodes.get(node).copied().unwrap_or(u32::MAX)
    }

    fn lot_grid(&self, lot: LotId) -> u32 {
        self.lot_network(lot, &self.power_cache.grids.node)
    }

    /// Brings the allocation up to date with roads, power, buildings, lots and the clock.
    pub(in crate::world) fn ensure_power(&mut self) {
        let step = clock_seconds(self.totals.tick, self.cfg.dt) / OUTPUT_STEP_S;
        let key = (
            self.roads.version,
            self.power.version,
            self.buildings.version,
            self.lots.version,
            step,
        );
        if self.power_cache.key == Some(key) {
            return;
        }
        let grids_key = (self.roads.version, self.power.version);
        if self.power_cache.grids_key != Some(grids_key) {
            self.power_cache.grids =
                grid::derive(&self.roads, &self.power.footprints(), &self.power.lines());
            self.power_cache.grids_key = Some(grids_key);
        }
        let n = self.buildings.active.len();
        for b in self.power_cache.demand.len()..n {
            let id = b as BuildingId;
            let d = power_demand(
                self.buildings.zone[b],
                self.buildings.footprint(id),
                self.buildings.height[b],
            );
            self.power_cache.demand.push(d);
        }
        let grid: Vec<u32> = (0..n)
            .map(|b| self.lot_grid(self.buildings.lot[b]))
            .collect();
        let at = step * OUTPUT_STEP_S;
        let cache = &mut self.power_cache;
        cache.grid = grid;
        cache.output = vec![0; self.power.plant_active.len()];
        cache.supply = vec![0; cache.grids.count];
        for p in self.power.active_plants() {
            let i = p as usize;
            let out = self.power.plant_kind[i].output(self.cfg.seed, at);
            cache.output[i] = out;
            cache.supply[cache.grids.piece[i] as usize] += out;
        }
        cache.left = cache.supply.clone();
        cache.open = vec![true; cache.grids.count];
        cache.starter_left = STARTER_POWER;
        cache.starter_open = true;
        cache.powered = vec![false; n];
        for b in 0..n {
            if self.buildings.active[b] && self.buildings.zone[b].is_school() {
                // Schools need no power and take from no pool.
                self.power_cache.powered[b] = true;
            } else if self.buildings.active[b] {
                let c = &self.power_cache;
                let source = self.power_source(c.demand[b], c.grid[b]);
                self.take_power(b, source);
            }
        }
        self.power_cache.key = Some(key);
        self.power_cache.version = self.power_cache.version.wrapping_add(1);
    }

    /// The pool that would power a building of demand `d` on grid `g`, served after every
    /// standing building. `None`: it would be unpowered.
    pub(in crate::world) fn power_source(&self, d: u32, g: u32) -> Option<Source> {
        let c = &self.power_cache;
        if self.cfg.ignore_demand || (c.starter_open && d <= c.starter_left) {
            return Some(Source::Starter);
        }
        let g = g as usize;
        (g < c.open.len() && c.open[g] && d <= c.left[g]).then_some(Source::Grid(g as u32))
    }

    /// Serves building `b`, the newest so far, from `source`, closing the pools it missed.
    fn take_power(&mut self, b: usize, source: Option<Source>) {
        let c = &mut self.power_cache;
        let d = c.demand[b];
        if !self.cfg.ignore_demand {
            match source {
                Some(Source::Starter) => c.starter_left -= d,
                _ => c.starter_open = false,
            }
        }
        match source {
            Some(Source::Grid(g)) => c.left[g as usize] -= d,
            None if (c.grid[b] as usize) < c.open.len() => c.open[c.grid[b] as usize] = false,
            _ => {}
        }
        c.powered[b] = source.is_some();
    }

    /// Whether a lot's planned building would be powered, and from where.
    pub(in crate::world) fn power_for(&mut self, lot: LotId, demand: u32) -> Option<Source> {
        self.ensure_power();
        self.power_source(demand, self.lot_grid(lot))
    }

    /// Records a building just built with `source` from `power_for`, keeping the allocation
    /// exactly what a full recompute would give.
    pub(in crate::world) fn powered_new_building(
        &mut self,
        demand: u32,
        lot: LotId,
        source: Source,
    ) {
        let b = self.buildings.active.len() - 1;
        let g = self.lot_grid(lot);
        let c = &mut self.power_cache;
        c.demand.push(demand);
        c.grid.push(g);
        c.powered.push(false);
        self.take_power(b, Some(source));
        let c = &mut self.power_cache;
        if let Some(key) = &mut c.key {
            key.2 = self.buildings.version;
        }
        c.version = c.version.wrapping_add(1);
    }

    pub fn power_stats(&self) -> PowerStats {
        let c = &self.power_cache;
        let mut s = PowerStats {
            starter: STARTER_POWER,
            ..Default::default()
        };
        let mut served = vec![false; c.supply.len()];
        for b in self.buildings.active_buildings() {
            let i = b as usize;
            if let Some(g) = c.grid.get(i).and_then(|&g| served.get_mut(g as usize)) {
                *g = true;
            }
            let d = c.demand.get(i).copied().unwrap_or(0);
            s.demand += d;
            if c.powered.get(i) == Some(&true) {
                s.used += d;
            } else {
                s.unpowered += 1;
            }
        }
        s.supply = (c.supply.iter().zip(&served))
            .filter(|(_, served)| **served)
            .map(|(supply, _)| supply)
            .sum();
        s.waiting = self.buildings.waiting_for_power();
        s
    }
}
