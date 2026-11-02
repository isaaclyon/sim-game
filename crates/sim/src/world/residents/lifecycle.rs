use super::super::World;
use crate::agents::AgentState;
use crate::building::BuildingId;
use crate::citizen::{Activity, CitizenId, HouseholdId};
use std::cmp::Reverse;

impl World {
    pub(in crate::world) fn home_building(&self, c: CitizenId) -> BuildingId {
        let household = self.citizens.household[c as usize] as usize;
        self.homes.building[self.households.home[household] as usize]
    }

    /// Cancel without arrival. All traffic indexes are cleaned before the slot can be reused.
    pub fn cancel_citizen_trip(&mut self, c: CitizenId) {
        let i = c as usize;
        if let Some(a) = self.citizens.agent[i] {
            if self.agents.state[a as usize] == AgentState::Driving {
                self.traffic.queues[self.agents.lane[a as usize] as usize].retain(|&v| v != a);
            }
            self.entering.retain(|&v| v != a);
            self.walking.retain(|&v| v != a);
            self.path_queue.retain(|&v| v != a);
            self.agents.state[a as usize] = AgentState::Dormant;
            self.agents.cleared[a as usize] = false;
        }
        self.citizens.destination[i] = None;
        self.citizens.due[i] = u64::MAX;
    }

    /// Atomically reserve an eligible vacant home for the whole household, or leave town.
    /// Also usable by gradual closure; it deliberately excludes the current home.
    pub fn relocate_household(&mut self, household: HouseholdId) {
        let hh = household as usize;
        if !self.households.active[hh] {
            return;
        }
        let old = self.households.home[hh] as usize;
        let size = self.households.members[hh].len();
        let replacement = self.vacant_home(old, size);
        self.homes.household[old] = None;
        let b = self.homes.building[old];
        if self.buildings.active[b as usize] {
            self.residents.arrivals.push(Reverse((
                self.totals.tick + self.secs_to_ticks(30.0),
                old as u32,
            )));
            if !self.building_closing(b) {
                self.residents.vacant.insert(old as u32);
            }
        }
        if let Some(h) = replacement {
            self.homes.household[h] = Some(household);
            self.residents.vacant.remove(&(h as u32));
            self.households.home[hh] = h as u32;
        } else {
            self.households.active[hh] = false;
        }
        for c in self.households.members[hh].clone() {
            let i = c as usize;
            self.cancel_citizen_trip(c);
            if let Some(h) = replacement {
                self.citizens.location[i] = self.homes.building[h];
                self.citizens.car_out[i] = false;
                self.citizens.activity[i] = Activity::Home;
                self.schedule_activity(c, 30.0);
            } else {
                self.release_job(c);
                self.citizens.active[i] = false;
                self.citizens.job_due[i] = u64::MAX;
                self.residents.population -= 1;
            }
        }
    }

    /// Lowest-id vacant home other than `old` that fits `size` and accepts assignments. The
    /// index only narrows the candidates; the full eligibility test still decides.
    pub(in crate::world) fn vacant_home(&self, old: usize, size: usize) -> Option<usize> {
        self.residents
            .vacant
            .iter()
            .map(|&h| h as usize)
            .find(|&h| {
                h != old
                    && self.homes.household[h].is_none()
                    && self.homes.capacity[h] as usize >= size
                    && self.building_accepts_assignments(self.homes.building[h])
            })
    }

    /// The vacancy index lists exactly the vacant homes of active buildings that are not closing.
    #[cfg(test)]
    pub(in crate::world) fn vacancies_hold(&self) -> bool {
        (0..self.homes.building.len()).all(|h| {
            let b = self.homes.building[h];
            let open = self.buildings.active[b as usize] && !self.building_closing(b);
            let expected = open && self.homes.household[h].is_none();
            self.residents.vacant.contains(&(h as u32)) == expected
        })
    }

    pub(in crate::world) fn release_job(&mut self, c: CitizenId) {
        if let Some(job) = self.citizens.job[c as usize].take() {
            self.residents.workers[job as usize] -= 1;
            self.residents.employed -= 1;
        }
    }

    /// Run after building removal, not each tick. Loss of access alone never evicts residents
    /// or releases employment. Assigned workers and physical activity locations are distinct.
    pub fn reconcile_residents(&mut self) {
        self.register_buildings();
        for hh in 0..self.households.active.len() {
            let b = self.homes.building[self.households.home[hh] as usize];
            if self.households.active[hh] && !self.buildings.active[b as usize] {
                self.relocate_household(hh as u32);
            }
        }
        for i in 0..self.citizens.active.len() {
            if !self.citizens.active[i] {
                continue;
            }
            let c = i as u32;
            if self.citizens.job[i].is_some_and(|b| !self.buildings.active[b as usize]) {
                self.release_job(c);
                self.schedule_search(c, 1.0);
            }
            let removed_location = !self.buildings.active[self.citizens.location[i] as usize];
            let removed_destination =
                self.citizens.destination[i].is_some_and(|b| !self.buildings.active[b as usize]);
            if removed_location || removed_destination {
                self.cancel_citizen_trip(c);
                let home = self.home_building(c);
                self.citizens.activity[i] = Activity::Home;
                if removed_location {
                    // A car out at a removed building comes home with its owner.
                    self.citizens.location[i] = home;
                    self.citizens.car_out[i] = false;
                } else if self.citizens.location[i] != home {
                    self.citizens.destination[i] = Some(home);
                }
                self.schedule_activity(c, 30.0);
            }
        }
    }
}
