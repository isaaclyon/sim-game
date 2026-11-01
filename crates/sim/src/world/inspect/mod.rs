//! Read-only details about one citizen, for the citizen inspector (ADR 0022). Nothing here
//! mutates the world or draws from its RNGs, so inspecting cannot change the simulation.
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
    /// The citizen's own vehicle slot, once allocated; never another citizen's.
    pub agent: Option<AgentId>,
    /// On a road now, so the car is visible.
    pub driving: bool,
}

impl World {
    /// The resident who owns vehicle slot `agent`, or `None` for synthetic or unknown slots.
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
        })
    }
}
