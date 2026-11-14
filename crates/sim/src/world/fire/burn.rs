//! Fires starting, burning down and leaving ruins, and trucks sent to them (ADR 0053).

use super::{BURN_S, CHECK_S, FIRE_THRESHOLD, RUIN_S, Task};
use crate::building::BuildingId;
use crate::world::World;
use crate::zone::Zone;

/// Chance per fire check that a building catches fire, per million.
pub(super) fn risk(zone: Zone) -> u32 {
    match zone {
        Zone::ResidentialLow | Zone::CommercialLow => 150,
        Zone::ResidentialHigh | Zone::CommercialHigh => 300,
        Zone::IndustrialLow => 450,
        Zone::IndustrialHigh => 750,
        // Schools do not burn (ADR 0055).
        Zone::None | Zone::PrimarySchool | Zone::HighSchool => 0,
    }
}

impl World {
    /// Runs once per tick, after traffic and before growth.
    pub(in crate::world) fn tick_fire(&mut self) {
        let f = &self.fire;
        if !f.fire_building.is_empty() {
            self.burn_fires();
        }
        if !self.fire.ruin_lot.is_empty() {
            self.expire_ruins();
        }
        let every = self.secs_to_ticks(CHECK_S);
        if self.fire.enabled
            && self.residents.population >= FIRE_THRESHOLD
            && self.totals.tick.is_multiple_of(every)
        {
            self.ignite();
        }
        if self.fire.fire_truck.contains(&None) {
            self.dispatch();
        }
    }

    /// Each standing building may catch fire, drawing from the fire stream in ID order.
    pub(super) fn ignite(&mut self) {
        let until = self.totals.tick + self.secs_to_ticks(BURN_S);
        for b in 0..self.buildings.active.len() {
            if !self.buildings.active[b] {
                continue;
            }
            let roll = self.fire.rng.below(1_000_000);
            let b = b as BuildingId;
            if roll < risk(self.buildings.zone[b as usize]) && self.fire.burning(b).is_none() {
                self.start_fire(b, until);
            }
        }
    }

    pub(super) fn start_fire(&mut self, b: BuildingId, until: u64) {
        let f = &mut self.fire;
        f.fire_building.push(b);
        f.fire_until.push(until);
        f.fire_truck.push(None);
        f.bump();
    }

    /// Fires on buildings already gone end; fires past their time burn the building down.
    fn burn_fires(&mut self) {
        let tick = self.totals.tick;
        let mut f = 0;
        while f < self.fire.fire_building.len() {
            let b = self.fire.fire_building[f];
            if !self.buildings.active[b as usize] {
                self.drop_fire(f);
            } else if self.fire.fire_until[f] <= tick {
                self.burn_down(f);
            } else {
                f += 1;
            }
        }
    }

    /// Fire `f` destroys its building: its lot holds ruins, its households move or leave and
    /// its workers lose their jobs.
    fn burn_down(&mut self, f: usize) {
        let b = self.fire.fire_building[f];
        let lot = self.buildings.lot[b as usize];
        self.drop_fire(f);
        let (tick, dt, seed) = (self.totals.tick, self.cfg.dt, self.cfg.seed);
        self.buildings
            .remove(b, &self.lots, &self.zones, tick, dt, seed);
        let until = tick + self.secs_to_ticks(RUIN_S);
        self.fire.ruin_lot.push(lot);
        self.fire.ruin_until.push(until);
        self.fire.destroyed += 1;
        self.reconcile_closures();
        self.reconcile_residents();
    }

    /// Ruins stand until their time is up or their lot is gone.
    fn expire_ruins(&mut self) {
        let tick = self.totals.tick;
        let mut r = 0;
        while r < self.fire.ruin_lot.len() {
            let lot = self.fire.ruin_lot[r] as usize;
            if self.fire.ruin_until[r] > tick && self.lots.active[lot] {
                r += 1;
                continue;
            }
            self.fire.ruin_lot.remove(r);
            self.fire.ruin_until.remove(r);
            self.fire.bump();
        }
    }

    /// Each fire without a truck gets the free truck at the nearest covering station. A fire
    /// at the station's own junction is out at once.
    fn dispatch(&mut self) {
        let mut f = 0;
        while f < self.fire.fire_building.len() {
            if self.fire.fire_truck[f].is_some() {
                f += 1;
                continue;
            }
            let b = self.fire.fire_building[f];
            let free = |s: u32| self.free_truck(s).is_some();
            let Some((s, _)) = self.nearest_station_where(b, free) else {
                f += 1;
                continue;
            };
            let t = self.free_truck(s).expect("checked above");
            let to = self
                .building_node(b)
                .expect("covered buildings have a junction");
            if self.fire.cover.node[s as usize] == Some(to) {
                self.drop_fire(f);
                self.fire.saved += 1;
                continue;
            }
            self.fire.fire_truck[f] = Some(t as u32);
            self.fire.bump();
            self.send_truck(t, to);
            f += 1;
        }
    }

    /// The lowest-numbered free truck of station `s`.
    fn free_truck(&self, s: u32) -> Option<usize> {
        let f = &self.fire;
        (0..f.truck_agent.len()).find(|&t| f.truck_station[t] == s && f.truck_task[t] == Task::Home)
    }
}
