//! Fire (ADR 0053): roadside fire stations, the buildings they cover, fires on grown buildings
//! and the trucks sent through traffic to put them out.
//!
//! Stations, trucks, burning buildings, ruins, the fire RNG stream and the city's fire setting
//! are stored, saved and hashed. Coverage (road distances from each station) is derived from
//! roads and stations and rebuilt whenever either changes.

mod burn;
mod cover;
mod place;
mod snapshot;
#[cfg(test)]
mod tests;
mod truck;

use serde::{Deserialize, Serialize};

use super::World;
use crate::agents::AgentId;
use crate::building::BuildingId;
use crate::lot::LotId;
use crate::power::Footprint;
use crate::rng::Rng;

pub use snapshot::{FIRE_STRIDE, STATION_STRIDE};

/// Half the side of a station's square footprint, metres.
pub const STATION_HALF: f32 = 12.0;
/// What placing a station costs, dollars.
pub const STATION_COST: i64 = 6_000;
/// What each station costs to run per clock hour, dollars.
pub const STATION_UPKEEP: i64 = 120;
/// Jobs each station adds to commercial demand, like a coal plant's to industry (ADR 0041).
pub const STATION_JOBS: u32 = 12;
/// Trucks each station owns.
pub const TRUCKS_PER_STATION: u32 = 2;
/// A station covers buildings whose frontage junction is this far along roads, metres.
pub const COVER_M: f32 = 1_200.0;
/// No fire starts while fewer people than this live in the city.
pub const FIRE_THRESHOLD: u32 = 1_000;
/// Sim seconds between fire checks.
pub const CHECK_S: f32 = 10.0;
/// Sim seconds a fire burns before the building is lost.
pub const BURN_S: f32 = 150.0;
/// Sim seconds ruins stand: one clock day.
pub const RUIN_S: f32 = 4_800.0;
/// Land value a covered building's cells gain, as a share of full value.
pub const LAND_BOOST: f32 = 0.15;
/// Separates the fire stream from every other stream drawn from the world seed.
const FIRE_SALT: u64 = 0xf12e_5eed;

/// What a truck is doing.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[repr(u8)]
pub enum Task {
    /// At its station, free.
    Home,
    /// Driving to the fire it was sent to.
    Outbound,
    /// Driving back to its station.
    Returning,
    /// Its station was bulldozed; the agent slot stays dormant forever.
    Retired,
}

/// The city's fire state. IDs are indices and append-only, except burning buildings and ruins,
/// which are kept in the order they started and dropped once over.
#[derive(Serialize, Deserialize)]
pub struct Fire {
    /// Fires start only while this is on (the default).
    pub enabled: bool,
    rng: Rng,
    pub station_x: Vec<f32>,
    pub station_y: Vec<f32>,
    /// Unit direction the station's front faces: toward its road.
    pub station_fx: Vec<f32>,
    pub station_fy: Vec<f32>,
    pub station_active: Vec<bool>,
    truck_agent: Vec<AgentId>,
    truck_station: Vec<u32>,
    truck_task: Vec<Task>,
    /// Burning buildings, the tick each burns down at, and the truck sent, if any.
    fire_building: Vec<BuildingId>,
    fire_until: Vec<u64>,
    fire_truck: Vec<Option<u32>>,
    ruin_lot: Vec<LotId>,
    ruin_until: Vec<u64>,
    /// Buildings saved and lost so far.
    pub saved: u32,
    pub destroyed: u32,
    /// Bumped whenever anything the frontend draws changes.
    #[serde(skip)]
    pub version: u32,
    #[serde(skip)]
    cover: cover::Cover,
    /// Per agent ID: whether it is a fire truck. Derived from `truck_agent`.
    #[serde(skip)]
    is_truck: Vec<bool>,
}

/// Fire figures for the top bar.
#[derive(Clone, Copy, Default, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct FireStats {
    pub enabled: bool,
    pub threshold: u32,
    pub stations: u32,
    pub trucks: u32,
    pub free_trucks: u32,
    pub burning: u32,
    /// Burning with no truck on the way.
    pub unattended: u32,
    pub covered: u32,
    pub saved: u32,
    pub destroyed: u32,
}

impl Fire {
    pub(super) fn new(seed: u64) -> Self {
        Self {
            enabled: true,
            rng: Rng::new(seed ^ FIRE_SALT),
            station_x: Vec::new(),
            station_y: Vec::new(),
            station_fx: Vec::new(),
            station_fy: Vec::new(),
            station_active: Vec::new(),
            truck_agent: Vec::new(),
            truck_station: Vec::new(),
            truck_task: Vec::new(),
            fire_building: Vec::new(),
            fire_until: Vec::new(),
            fire_truck: Vec::new(),
            ruin_lot: Vec::new(),
            ruin_until: Vec::new(),
            saved: 0,
            destroyed: 0,
            version: 0,
            cover: cover::Cover::default(),
            is_truck: Vec::new(),
        }
    }

    pub fn active_stations(&self) -> impl Iterator<Item = u32> + '_ {
        let on = &self.station_active;
        (0..on.len()).filter(|&s| on[s]).map(|s| s as u32)
    }

    pub fn footprint(&self, s: u32) -> Footprint {
        let i = s as usize;
        Footprint {
            centre: (self.station_x[i], self.station_y[i]),
            front: (self.station_fx[i], self.station_fy[i]),
            half: STATION_HALF,
        }
    }

    /// Whether agent `a` is a fire truck.
    pub fn is_truck(&self, a: AgentId) -> bool {
        self.is_truck.get(a as usize) == Some(&true)
    }

    fn mark_truck(&mut self, a: AgentId) {
        let i = a as usize;
        if self.is_truck.len() <= i {
            self.is_truck.resize(i + 1, false);
        }
        self.is_truck[i] = true;
    }

    fn truck_of(&self, a: AgentId) -> Option<usize> {
        if !self.is_truck(a) {
            return None;
        }
        self.truck_agent.iter().position(|&t| t == a)
    }

    fn burning(&self, b: BuildingId) -> Option<usize> {
        self.fire_building.iter().position(|&f| f == b)
    }

    /// Whether ruins stand on the lot, so nothing grows there.
    pub(super) fn ruined(&self, lot: LotId) -> bool {
        self.ruin_lot.contains(&lot)
    }

    fn bump(&mut self) {
        self.version = self.version.wrapping_add(1);
    }

    fn pristine(&self, seed: u64) -> bool {
        self.enabled
            && self.station_active.is_empty()
            && self.fire_building.is_empty()
            && self.ruin_lot.is_empty()
            && self.saved == 0
            && self.destroyed == 0
            && self.rng.state() == Rng::new(seed ^ FIRE_SALT).state()
    }

    /// Mixes nothing for a city that never had fire state, so cities from before fire hash
    /// as they did.
    pub(super) fn hash_into(&self, seed: u64, mix: &mut impl FnMut(u64)) {
        if self.pristine(seed) {
            return;
        }
        mix(u64::from(self.enabled));
        mix(self.rng.state());
        mix(self.station_active.len() as u64);
        for s in 0..self.station_active.len() {
            mix(u64::from(self.station_active[s]));
            mix(u64::from(self.station_x[s].to_bits()) << 32
                | u64::from(self.station_y[s].to_bits()));
            mix(u64::from(self.station_fx[s].to_bits()) << 32
                | u64::from(self.station_fy[s].to_bits()));
        }
        for t in 0..self.truck_agent.len() {
            mix(u64::from(self.truck_agent[t]) << 32 | u64::from(self.truck_station[t]));
            mix(self.truck_task[t] as u64);
        }
        mix(self.fire_building.len() as u64);
        for f in 0..self.fire_building.len() {
            mix(u64::from(self.fire_building[f]));
            mix(self.fire_until[f]);
            mix(self.fire_truck[f].map_or(u64::MAX, u64::from));
        }
        mix(self.ruin_lot.len() as u64);
        for r in 0..self.ruin_lot.len() {
            mix(u64::from(self.ruin_lot[r]) << 32 ^ self.ruin_until[r]);
        }
        mix(u64::from(self.saved) << 32 | u64::from(self.destroyed));
    }

    /// Internal consistency of loaded fire state (ADR 0033).
    pub(super) fn check(
        &self,
        agents: usize,
        buildings: usize,
        lots: usize,
    ) -> crate::check::Check {
        use crate::check::{below, ensure, finite, lens};
        let stations = self.station_active.len();
        let fields = [
            self.station_x.len(),
            self.station_y.len(),
            self.station_fx.len(),
            self.station_fy.len(),
        ];
        lens(&fields, stations, "station fields")?;
        finite(
            self.station_x
                .iter()
                .chain(&self.station_y)
                .chain(&self.station_fx)
                .chain(&self.station_fy),
            "station positions",
        )?;
        let trucks = self.truck_agent.len();
        lens(
            &[self.truck_station.len(), self.truck_task.len()],
            trucks,
            "truck fields",
        )?;
        below(&self.truck_agent, agents, "truck agents")?;
        below(&self.truck_station, stations, "truck stations")?;
        let fires = self.fire_building.len();
        lens(
            &[self.fire_until.len(), self.fire_truck.len()],
            fires,
            "fire fields",
        )?;
        below(&self.fire_building, buildings, "burning buildings")?;
        below(self.fire_truck.iter().flatten(), trucks, "fire trucks")?;
        ensure(self.ruin_until.len() == self.ruin_lot.len(), "ruin fields")?;
        below(&self.ruin_lot, lots, "ruin lots")
    }
}

impl World {
    /// Turns fires on or off for this city. Turning them off puts out every fire burning now.
    pub(super) fn set_fires(&mut self, on: bool) -> bool {
        if self.fire.enabled == on {
            return false;
        }
        self.fire.enabled = on;
        if !on {
            while let Some(&b) = self.fire.fire_building.first() {
                self.end_fire(b);
            }
        }
        self.fire.bump();
        true
    }

    /// Fire figures for the top bar.
    pub fn fire_stats(&self) -> FireStats {
        let f = &self.fire;
        let trucks = || (0..f.truck_agent.len()).filter(|&t| f.truck_task[t] != Task::Retired);
        FireStats {
            enabled: f.enabled,
            threshold: FIRE_THRESHOLD,
            stations: f.active_stations().count() as u32,
            trucks: trucks().count() as u32,
            free_trucks: trucks().filter(|&t| f.truck_task[t] == Task::Home).count() as u32,
            burning: f.fire_building.len() as u32,
            unattended: f.fire_truck.iter().filter(|t| t.is_none()).count() as u32,
            covered: f.cover.covered.iter().filter(|&&c| c).count() as u32,
            saved: f.saved,
            destroyed: f.destroyed,
        }
    }

    /// Station upkeep per clock hour.
    pub(super) fn fire_upkeep(&self) -> i64 {
        self.fire.active_stations().count() as i64 * STATION_UPKEEP
    }

    /// Jobs at stations, counted with commercial jobs in demand.
    pub(super) fn fire_jobs(&self) -> u32 {
        self.fire.active_stations().count() as u32 * STATION_JOBS
    }

    /// Brings derived fire state up to date: which agents are trucks, and coverage.
    pub(super) fn ensure_fire(&mut self) {
        let f = &mut self.fire;
        // Empty only after a load: placing a station marks its trucks as it adds them.
        if f.is_truck.is_empty() {
            for t in 0..f.truck_agent.len() {
                f.mark_truck(f.truck_agent[t]);
            }
        }
        self.ensure_cover();
    }
}
