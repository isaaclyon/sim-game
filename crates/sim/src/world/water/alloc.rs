//! Who gets water (ADR 0052). Each network serves its dense buildings oldest first, from pump
//! output and then tower storage, until one does not fit; that building and every newer dense
//! building on the network are dry. Low-density buildings are on wells and ignore water.
//! There is no starter water.

use serde::{Deserialize, Serialize};

use super::super::World;
use super::super::power::power_demand;
use crate::building::BuildingId;
use crate::geometry::Point;
use crate::lot::LotId;
use crate::pollution::UPDATE_S;
use crate::power::grid::{self, Grids};
use crate::water::{TOWER_CAPACITY, WaterKind, pump_output};
use crate::zone::Zone;

/// The town's water, for the top bar.
#[derive(Clone, Copy, Default, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct WaterStats {
    /// What all pumps give each update now.
    pub supply: u32,
    /// Use of the dense buildings that get water, from pumps or towers.
    pub used: u32,
    /// Use of every standing dense building.
    pub demand: u32,
    /// Water in all towers, and what they hold when full.
    pub stored: u32,
    pub capacity: u32,
    /// Standing dense buildings without water.
    pub dry: u32,
    /// Scheduled lots that last waited for water, not demand or power; saved with their schedules.
    pub waiting: u32,
}

/// The derived allocation and its inputs' versions. Never saved: rebuilt on first use.
#[derive(Default)]
pub(in crate::world) struct WaterCache {
    /// Roads, water, buildings and lots versions, the pollution update and the water step.
    key: Option<(u32, u32, u32, u32, u64, u64)>,
    nets_key: Option<(u32, u32)>,
    pub(super) nets: Grids,
    /// Per building ID.
    pub(super) demand: Vec<u32>,
    pub(super) net: Vec<u32>,
    pub(super) watered: Vec<bool>,
    /// Per network: pump output, and what is left of it after the oldest buildings; tower
    /// storage, and what is left of it.
    pub(super) supply: Vec<u32>,
    pub(super) left: Vec<u32>,
    pub(super) stored: Vec<u32>,
    pub(super) store_left: Vec<u32>,
    open: Vec<bool>,
    /// Per piece ID: what a pump gives now; 0 for towers.
    pub(super) output: Vec<u32>,
    /// Bumped whenever what the frontend draws may have changed.
    pub(super) version: u32,
}

/// A building's water use: its power demand (people and jobs) when dense, else 0 (on a well).
pub(in crate::world) fn water_demand(zone: Zone, ring: &[Point], height: f32) -> u32 {
    if zone.is_dense() {
        power_demand(zone, ring, height)
    } else {
        0
    }
}

impl World {
    /// Whether the building has water or does not need it. Removed buildings read false.
    pub fn building_watered(&self, b: BuildingId) -> bool {
        self.water_cache.watered.get(b as usize) == Some(&true)
    }

    /// Mean ground pollution under a pump: the footprint's corners and centre.
    fn ground_under(&self, p: u32) -> u16 {
        let at = self.water.footprint(p);
        let samples: Vec<u64> = at
            .ring()
            .iter()
            .chain([&at.centre])
            .map(|&(x, y)| {
                let cell = self.pollution.cell_at(x, y);
                cell.map_or(0, |c| u64::from(self.pollution.ground[c]))
            })
            .collect();
        (samples.iter().sum::<u64>() / samples.len() as u64) as u16
    }

    /// Brings the allocation up to date with roads, water, buildings, lots, ground pollution
    /// and tower levels. Records when dense buildings go dry or get water back.
    pub(in crate::world) fn ensure_water(&mut self) {
        let every = (UPDATE_S / self.cfg.dt).round().max(1.0) as u64;
        let key = (
            self.roads.version,
            self.water.version,
            self.buildings.version,
            self.lots.version,
            self.totals.tick / every,
            self.water.step,
        );
        if self.water_cache.key == Some(key) {
            return;
        }
        let nets_key = (self.roads.version, self.water.version);
        if self.water_cache.nets_key != Some(nets_key) {
            self.water_cache.nets = grid::derive(&self.roads, &self.water.footprints(), &[]);
            self.water_cache.nets_key = Some(nets_key);
        }
        let n = self.buildings.active.len();
        for b in self.water_cache.demand.len()..n {
            let d = water_demand(
                self.buildings.zone[b],
                self.buildings.footprint(b as BuildingId),
                self.buildings.height[b],
            );
            self.water_cache.demand.push(d);
        }
        let net: Vec<u32> = (0..n)
            .map(|b| self.lot_network(self.buildings.lot[b], &self.water_cache.nets.node))
            .collect();
        let pieces = self.water.active.len();
        let mut output = vec![0; pieces];
        let count = self.water_cache.nets.count;
        let (mut supply, mut stored) = (vec![0; count], vec![0; count]);
        for p in self.water.active_pieces() {
            let i = p as usize;
            let g = self.water_cache.nets.piece[i] as usize;
            match self.water.kind[i] {
                WaterKind::Pump => {
                    output[i] = pump_output(self.ground_under(p));
                    supply[g] += output[i];
                }
                WaterKind::Tower => stored[g] += self.water.level[i],
            }
        }
        let old = std::mem::take(&mut self.water_cache.watered);
        let same_inputs = self.water_cache.key.map(|k| (k.0, k.1, k.5))
            == Some((key.0, key.1, key.5))
            && self.water_cache.output == output
            && self.water_cache.net == net;
        let c = &mut self.water_cache;
        c.net = net;
        c.output = output;
        c.left = supply.clone();
        c.supply = supply;
        c.store_left = stored.clone();
        c.stored = stored;
        c.open = vec![true; count];
        c.watered = vec![false; n];
        for b in 0..n {
            if self.buildings.active[b] {
                self.take_water(b);
            }
        }
        self.water.dry_since.resize(n, u64::MAX);
        for b in 0..n {
            let dry = self.buildings.active[b] && !self.water_cache.watered[b];
            let since = &mut self.water.dry_since[b];
            if !dry {
                *since = u64::MAX;
            } else if *since == u64::MAX {
                *since = self.totals.tick;
            }
        }
        let c = &mut self.water_cache;
        c.key = Some(key);
        if !(same_inputs && old == c.watered) {
            c.version = c.version.wrapping_add(1);
        }
    }

    /// Serves building `b`, the newest so far: from its network's pumps, then its towers.
    fn take_water(&mut self, b: usize) {
        let ignore = self.cfg.ignore_demand;
        let c = &mut self.water_cache;
        let (d, g) = (c.demand[b], c.net[b] as usize);
        c.watered[b] = if d == 0 || ignore {
            true
        } else if g >= c.open.len() || !c.open[g] {
            false
        } else if d <= c.left[g] {
            c.left[g] -= d;
            true
        } else if d <= c.left[g] + c.store_left[g] {
            c.store_left[g] -= d - c.left[g];
            c.left[g] = 0;
            true
        } else {
            c.open[g] = false;
            false
        };
    }

    /// Whether a lot's planned building, using `demand`, would get water from pump output
    /// alone. Tower storage covers dips; it does not let new buildings grow.
    pub(in crate::world) fn water_for(&mut self, lot: LotId, demand: u32) -> bool {
        self.ensure_water();
        if demand == 0 || self.cfg.ignore_demand {
            return true;
        }
        let g = self.lot_network(lot, &self.water_cache.nets.node) as usize;
        let c = &self.water_cache;
        g < c.open.len() && c.open[g] && demand <= c.left[g]
    }

    /// Records a building just built after `water_for` said yes, keeping the allocation
    /// exactly what a full recompute would give.
    pub(in crate::world) fn watered_new_building(&mut self, demand: u32, lot: LotId) {
        let b = self.buildings.active.len() - 1;
        let g = self.lot_network(lot, &self.water_cache.nets.node);
        let c = &mut self.water_cache;
        c.demand.push(demand);
        c.net.push(g);
        c.watered.push(false);
        self.take_water(b);
        self.water.dry_since.resize(b + 1, u64::MAX);
        let c = &mut self.water_cache;
        if let Some(key) = &mut c.key {
            key.2 = self.buildings.version;
        }
        c.version = c.version.wrapping_add(1);
    }

    pub fn water_stats(&self) -> WaterStats {
        let c = &self.water_cache;
        let mut s = WaterStats {
            supply: c.supply.iter().sum(),
            stored: c.stored.iter().sum(),
            ..Default::default()
        };
        s.capacity = self
            .water
            .active_pieces()
            .filter(|&p| self.water.kind[p as usize] == WaterKind::Tower)
            .count() as u32
            * TOWER_CAPACITY;
        for b in self.buildings.active_buildings() {
            let i = b as usize;
            let d = c.demand.get(i).copied().unwrap_or(0);
            if d == 0 {
                continue;
            }
            s.demand += d;
            if c.watered.get(i) == Some(&true) {
                s.used += d;
            } else {
                s.dry += 1;
            }
        }
        s.waiting = self.buildings.waiting_for_water();
        s
    }
}
