//! R/C/I demand: what the town wants built next (ADR 0025).
//!
//! People move in where there are jobs, and businesses open where there are people. Residential
//! demand is open jobs plus a small base, minus residents and the room left in empty homes, so
//! it equals job vacancies minus unemployment, less the households still on their way.
//! Commercial and industrial demand want a share of residents as jobs, plus a small base, minus
//! the jobs they already offer. A due lot builds only while its use has positive demand.

use super::World;
use crate::building::BuildingId;
use crate::zone::Zone;

/// Residents the town wants before any jobs exist, so an empty map can start a village.
const RESIDENTIAL_BASE: i32 = 40;
const COMMERCIAL_BASE: i32 = 8;
const INDUSTRIAL_BASE: i32 = 15;
/// Shares of residents wanted as commercial and industrial jobs, in percent. They add up to
/// 100, so a balanced town keeps every demand positive and keeps growing.
const COMMERCIAL_SHARE: i32 = 40;
const INDUSTRIAL_SHARE: i32 = 60;
/// Residents expected per empty home that will admit a household (ADR 0021 sizes average ~2).
const ROOM_PER_VACANT_HOME: i32 = 2;

/// Demand per use, in people: positive wants more buildings, negative has a surplus.
#[derive(Clone, Copy, Default, Debug, PartialEq, Eq)]
pub struct Demand {
    pub residential: i32,
    pub commercial: i32,
    pub industrial: i32,
}

/// Jobs offered by open buildings: active and not closing. Derived from hashed state, so it is
/// not hashed; `supply_holds` checks it against a full scan.
#[derive(Default)]
pub(super) struct Supply {
    /// Whether each building's jobs are in the totals below.
    counted: Vec<bool>,
    commercial_jobs: u32,
    industrial_jobs: u32,
}

impl World {
    pub fn demand(&self) -> Demand {
        let population = self.residents.population as i32;
        let jobs = (self.supply.commercial_jobs + self.supply.industrial_jobs) as i32;
        let room = self.residents.vacant.len() as i32 * ROOM_PER_VACANT_HOME;
        Demand {
            residential: RESIDENTIAL_BASE + jobs - population - room,
            commercial: COMMERCIAL_BASE + population * COMMERCIAL_SHARE / 100
                - self.supply.commercial_jobs as i32,
            industrial: INDUSTRIAL_BASE + population * INDUSTRIAL_SHARE / 100
                - self.supply.industrial_jobs as i32,
        }
    }

    fn demand_for(&self, zone: Zone) -> i32 {
        let d = self.demand();
        match zone {
            Zone::ResidentialLow | Zone::ResidentialHigh => d.residential,
            Zone::CommercialLow | Zone::CommercialHigh => d.commercial,
            Zone::IndustrialLow | Zone::IndustrialHigh => d.industrial,
            Zone::None => 0,
        }
    }

    /// Brings one registered building's jobs in or out of the totals after it was built,
    /// started or stopped closing, or was removed.
    pub(super) fn count_supply(&mut self, b: BuildingId) {
        let i = b as usize;
        if i >= self.residents.registered {
            return;
        }
        if self.supply.counted.len() <= i {
            self.supply.counted.resize(i + 1, false);
        }
        let open = self.buildings.active[i] && !self.building_closing(b);
        if open == self.supply.counted[i] {
            return;
        }
        self.supply.counted[i] = open;
        let jobs = self.residents.jobs[i];
        let total = match self.buildings.zone[i] {
            Zone::CommercialLow | Zone::CommercialHigh => &mut self.supply.commercial_jobs,
            Zone::IndustrialLow | Zone::IndustrialHigh => &mut self.supply.industrial_jobs,
            _ => return,
        };
        if open {
            *total += jobs;
        } else {
            *total -= jobs;
        }
    }

    /// Due lots build while their use has demand; the rest wait and check again. Each new
    /// building is registered at once, so the next lot due this tick sees its homes and jobs.
    pub(super) fn grow_buildings(&mut self) {
        let tick = self.totals.tick;
        while let Some((lot, zone)) = self.buildings.next_due(tick) {
            if !self.cfg.ignore_demand && self.demand_for(zone) <= 0 {
                self.buildings
                    .defer(lot, zone, tick, self.cfg.dt, self.cfg.seed);
            } else if self
                .buildings
                .build(lot, zone, &self.lots, &self.roads, self.cfg.seed)
            {
                self.register_buildings();
            }
        }
    }

    #[cfg(test)]
    pub(super) fn supply_holds(&self) -> bool {
        let (mut commercial, mut industrial) = (0, 0);
        for b in 0..self.residents.registered {
            if !self.buildings.active[b] || self.building_closing(b as BuildingId) {
                continue;
            }
            match self.buildings.zone[b] {
                Zone::CommercialLow | Zone::CommercialHigh => commercial += self.residents.jobs[b],
                Zone::IndustrialLow | Zone::IndustrialHigh => industrial += self.residents.jobs[b],
                _ => {}
            }
        }
        (commercial, industrial) == (self.supply.commercial_jobs, self.supply.industrial_jobs)
    }
}
