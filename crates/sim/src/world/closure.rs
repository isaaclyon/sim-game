//! Gradual closure of buildings whose lot was repainted or unzoned (ADR 0023).
//!
//! A building closes while its original zone differs from its lot's effective zone. Workers
//! lose their jobs when closure starts. Each step relocates one household; the first step that
//! finds none removes the building, and the lot grows for whatever zone it has by then.
//! Restoring the exact original zone cancels the remaining steps.

use std::cmp::Reverse;
use std::collections::BinaryHeap;

use super::World;
use crate::building::BuildingId;
use crate::citizen::{Activity, HouseholdId};

/// Sim seconds between closure steps, drawn uniformly per step. Tuning, not configuration.
pub(crate) const STEP_MIN_S: f32 = 3.0;
pub(crate) const STEP_MAX_S: f32 = 12.0;
/// A former worker at or heading to a closing workplace sets off home after this long.
const LEAVE_AFTER_S: f32 = 30.0;

#[derive(Default)]
pub(super) struct Closures {
    /// Next closure step per building id; `u64::MAX` when the building is not closing.
    pub due: Vec<u64>,
    /// Index into `due`. Entries whose deadline no longer matches are stale and skipped.
    pub heap: BinaryHeap<Reverse<(u64, BuildingId)>>,
}

impl World {
    /// The building stands but is emptying out because its lot was repainted or unzoned.
    pub fn building_closing(&self, b: BuildingId) -> bool {
        self.closures
            .due
            .get(b as usize)
            .is_some_and(|&due| due != u64::MAX)
    }

    fn schedule_closure_step(&mut self, b: BuildingId) {
        let tenths = ((STEP_MAX_S - STEP_MIN_S) * 10.0) as u32 + 1;
        let seconds = STEP_MIN_S + self.residents.rng.below(tenths) as f32 / 10.0;
        let at = self.totals.tick + self.secs_to_ticks(seconds);
        self.closures.due[b as usize] = at;
        self.closures.heap.push(Reverse((at, b)));
    }

    /// Runs with building reconciliation after lot or zone edits, never on the idle tick.
    pub(super) fn reconcile_closures(&mut self) {
        self.closures
            .due
            .resize(self.buildings.active.len(), u64::MAX);
        let mut started = false;
        for b in 0..self.buildings.active.len() {
            let closing = self.closures.due[b] != u64::MAX;
            if !self.buildings.active[b] {
                // Road edits remove buildings without a closure step.
                self.closures.due[b] = u64::MAX;
                self.list_vacancies(b as BuildingId, false);
                continue;
            }
            let lot = self.buildings.lot[b];
            let matches = self.lots.zone(lot, &self.zones) == self.buildings.zone[b];
            if !matches && !closing {
                self.schedule_closure_step(b as BuildingId);
                self.list_vacancies(b as BuildingId, false);
                started = true;
            } else if matches && closing {
                // Departures already made are not recalled.
                self.closures.due[b] = u64::MAX;
                self.list_vacancies(b as BuildingId, true);
            }
        }
        if started {
            self.release_closing_jobs();
        }
    }

    /// Workers of closing buildings become unemployed. Anyone working there or commuting
    /// there heads home instead, so nobody keeps a work activity without a job.
    fn release_closing_jobs(&mut self) {
        for i in 0..self.citizens.active.len() {
            let Some(job) = self.citizens.job[i] else {
                continue;
            };
            if !self.citizens.active[i] || !self.building_closing(job) {
                continue;
            }
            let c = i as u32;
            self.release_job(c);
            self.schedule_search(c, 1.0);
            let at = self.citizens.destination[i].unwrap_or(self.citizens.location[i]);
            if self.citizens.activity[i] == Activity::Work && at == job {
                self.cancel_citizen_trip(c);
                let home = self.home_building(c);
                self.citizens.activity[i] = Activity::Home;
                self.citizens.destination[i] = (self.citizens.location[i] != home).then_some(home);
                self.schedule_activity(c, LEAVE_AFTER_S);
            }
        }
    }

    /// Homes are registered in building order, so a building's homes are one contiguous run.
    fn homes_of(&self, b: BuildingId) -> std::ops::Range<usize> {
        let start = self.homes.building.partition_point(|&home| home < b);
        let end = self.homes.building.partition_point(|&home| home <= b);
        start..end
    }

    fn next_household(&self, b: BuildingId) -> Option<HouseholdId> {
        self.homes_of(b).find_map(|h| self.homes.household[h])
    }

    /// Adds a reopened building's vacant homes to the relocation index, or drops a closing one's.
    fn list_vacancies(&mut self, b: BuildingId, open: bool) {
        for h in self.homes_of(b) {
            if !open {
                self.residents.vacant.remove(&(h as u32));
            } else if self.homes.household[h].is_none() {
                self.residents.vacant.insert(h as u32);
            }
        }
    }

    /// Due closure steps. Removals share one resident cleanup per tick.
    pub(super) fn tick_closures(&mut self) {
        let mut removed = false;
        while let Some(&Reverse((at, b))) = self.closures.heap.peek() {
            if at > self.totals.tick {
                break;
            }
            self.closures.heap.pop();
            if self.closures.due[b as usize] != at {
                continue;
            }
            self.closures.due[b as usize] = u64::MAX;
            if let Some(household) = self.next_household(b) {
                // Still closing while it relocates, so its other homes stay ineligible.
                self.schedule_closure_step(b);
                self.relocate_household(household);
            } else {
                self.list_vacancies(b, false);
                removed |= self.buildings.remove(
                    b,
                    &self.lots,
                    &self.zones,
                    self.totals.tick,
                    self.cfg.dt,
                    self.cfg.seed,
                );
            }
        }
        if removed {
            self.reconcile_residents();
        }
    }

    pub(super) fn hash_closures(&self, mix: &mut impl FnMut(u64)) {
        mix(self.closures.due.len() as u64);
        for &due in &self.closures.due {
            mix(due);
        }
        mix(self.closures.heap.len() as u64);
        for entry in self.closures.heap.clone().into_sorted_vec() {
            mix(entry.0.0);
            mix(entry.0.1 as u64);
        }
    }
}
