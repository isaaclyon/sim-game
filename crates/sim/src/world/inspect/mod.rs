//! Read-only details about one citizen or building (ADRs 0022, 0029). Nothing here
//! mutates the world or draws from its RNGs, so inspecting cannot change the simulation.
#[cfg(test)]
mod buildings_tests;
mod names;
#[cfg(test)]
mod tests;

use super::World;
use crate::agents::{AgentId, AgentState};
use crate::building::BuildingId;
use crate::citizen::{Activity, CitizenId};
use crate::zone::Zone;

/// A building as the inspector names it: its ID and original use.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Place {
    pub building: BuildingId,
    pub zone: Zone,
}

#[derive(Clone, Debug, PartialEq)]
pub struct CitizenInfo {
    pub name: String,
    /// False once the citizen has left town; then every place below is `None`.
    pub resident: bool,
    pub home: Option<Place>,
    pub job: Option<Place>,
    /// Current activity, or the one the current trip is for.
    pub activity: Activity,
    /// Where the citizen is, or where the current trip started.
    pub location: Option<Place>,
    /// Where the current trip goes; `None` while staying put.
    pub destination: Option<Place>,
    /// The citizen's own travel slot, once allocated; never another citizen's.
    pub agent: Option<AgentId>,
    /// On a road now, so the car is visible.
    pub driving: bool,
    /// The current trip is on foot (ADR 0026); the walker is drawn once it sets off.
    pub on_foot: bool,
}

/// Counts for one standing building. Occupancy means residents or assigned workers,
/// including those currently elsewhere; capacity is not an offer of new assignments.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BuildingInfo {
    pub building: BuildingId,
    pub zone: Zone,
    pub capacity: u32,
    pub occupied: u32,
    pub closing: bool,
}

impl World {
    /// Read-only totals for a building, absent after demolition or for an unknown ID.
    pub fn building_info(&self, b: BuildingId) -> Option<BuildingInfo> {
        let i = b as usize;
        if self.buildings.active.get(i) != Some(&true) {
            return None;
        }
        let zone = self.buildings.zone[i];
        let (capacity, occupied) = if matches!(zone, Zone::ResidentialLow | Zone::ResidentialHigh) {
            // Homes are appended in building ID order, as in closure processing.
            let start = self.homes.building.partition_point(|&home| home < b);
            let end = self.homes.building.partition_point(|&home| home <= b);
            (start..end).fold((0, 0), |(capacity, occupied), h| {
                let residents = self.homes.household[h]
                    .filter(|&hh| self.households.active[hh as usize])
                    .map_or(0, |hh| self.households.members[hh as usize].len() as u32);
                (
                    capacity + self.homes.capacity[h] as u32,
                    occupied + residents,
                )
            })
        } else {
            (self.residents.jobs[i], self.residents.workers[i])
        };
        Some(BuildingInfo {
            building: b,
            zone,
            capacity,
            occupied,
            closing: self.building_closing(b),
        })
    }

    /// The resident who owns travel slot `agent`, or `None` for synthetic or unknown slots.
    pub fn citizen_of_agent(&self, agent: AgentId) -> Option<CitizenId> {
        self.agents.citizen.get(agent as usize).copied().flatten()
    }

    /// Details for citizen `c`, or `None` for an unknown ID. Places are reported only while
    /// their building stands, so a removed building never shows as a current place.
    pub fn citizen_info(&self, c: CitizenId) -> Option<CitizenInfo> {
        let i = c as usize;
        let household = *self.citizens.household.get(i)?;
        let resident = self.citizens.active[i];
        let place = |b: BuildingId| {
            (resident && self.buildings.active.get(b as usize) == Some(&true)).then(|| Place {
                building: b,
                zone: self.buildings.zone[b as usize],
            })
        };
        let agent = self.citizens.agent[i].filter(|_| resident);
        Some(CitizenInfo {
            name: names::display_name(self.cfg.seed, c, household),
            resident,
            home: place(self.home_building(c)),
            job: self.citizens.job[i].and_then(place),
            activity: self.citizens.activity[i],
            location: place(self.citizens.location[i]),
            destination: self.citizens.destination[i].and_then(place),
            agent,
            driving: agent.is_some_and(|a| self.agents.state[a as usize] == AgentState::Driving),
            on_foot: resident
                && self.citizens.destination[i].is_some()
                && !self.citizens.car_out[i],
        })
    }
}
