//! Children, births, growing up and moving out (ADR 0054). Everything runs off event heaps:
//! nothing scans idle citizens on the traffic tick.
use super::super::World;
use crate::building::BuildingId;
use crate::citizen::{Activity, CitizenId, HomeId, HouseholdId};
use crate::clock::{CLOCK_SECONDS_PER_SIM_SECOND, SECONDS_PER_DAY};
use std::cmp::Reverse;

/// Clock days from birth to coming of age.
pub const CHILDHOOD_DAYS: f32 = 10.0;
/// Age in years at coming of age. Inspection shows ages in years on this scale, so a clock
/// day is 1.8 years.
pub const ADULT_YEARS: u32 = 18;
/// Chance, percent, that an arriving member beyond the first adult is a child.
pub const CHILD_PERCENT: u32 = 40;
/// Ages of arriving adults, years.
const ARRIVING_ADULT_YEARS: (u32, u32) = (18, 60);
/// Clock days between a household's birth checks.
const BIRTH_CHECK_DAYS: f32 = 1.0;
/// Chance per birth check, percent, that a household with an adult and room has a baby.
pub const BIRTH_PERCENT: u32 = 15;
/// Clock days a grown child waits for a home of their own before leaving the city.
pub const MOVE_OUT_DAYS: f32 = 3.0;
/// Events of each kind handled per tick at most; the rest wait for the next tick.
const PER_TICK: usize = 32;

impl World {
    pub(in crate::world) fn days_to_ticks(&self, days: f32) -> u64 {
        let day_s = SECONDS_PER_DAY as f32 / CLOCK_SECONDS_PER_SIM_SECOND as f32;
        self.secs_to_ticks(days * day_s)
    }

    /// Age of citizen `c` in years on the `ADULT_YEARS` scale.
    pub(in crate::world) fn age_years(&self, c: CitizenId) -> u32 {
        let age = self.totals.tick as i64 - self.citizens.born[c as usize];
        let childhood = self.days_to_ticks(CHILDHOOD_DAYS) as i64;
        (age.max(0) * ADULT_YEARS as i64 / childhood) as u32
    }

    /// A new, empty household in vacant `home`, with its first birth check within a day.
    pub(in crate::world) fn found_household(&mut self, home: HomeId) -> HouseholdId {
        let household = self.households.active.len() as HouseholdId;
        self.households.active.push(true);
        self.households.home.push(home);
        self.households.members.push(Vec::new());
        self.homes.household[home as usize] = Some(household);
        self.residents.vacant.remove(&home);
        let day = self.days_to_ticks(BIRTH_CHECK_DAYS) as u32;
        let first = self.residents.birth_rng.below(day) as u64 + 1;
        self.residents
            .births
            .push(Reverse((self.totals.tick + first, household)));
        household
    }

    /// An arriving member of `household`. Members after the first may be children; everyone
    /// draws an age.
    pub(in crate::world) fn arrive_member(
        &mut self,
        household: HouseholdId,
        b: BuildingId,
        first: bool,
    ) {
        let childhood = self.days_to_ticks(CHILDHOOD_DAYS);
        let child = !first && self.residents.rng.below(100) < CHILD_PERCENT;
        let age = if child {
            self.residents.rng.below(childhood as u32) as u64
        } else {
            let (lo, hi) = ARRIVING_ADULT_YEARS;
            let years = lo + self.residents.rng.below(hi - lo);
            years as u64 * childhood / ADULT_YEARS as u64
        };
        let born = self.totals.tick as i64 - age as i64;
        let c = self
            .citizens
            .push(household, b, born, child, &mut self.residents.rng);
        self.add_member(household, c);
        if !child {
            self.citizens.education[c as usize] = self.arriving_education();
            self.schedule_day(c);
            self.schedule_search(c, 1.0);
        }
    }

    fn add_member(&mut self, household: HouseholdId, c: CitizenId) {
        self.households.members[household as usize].push(c);
        self.residents.population += 1;
        if self.citizens.child[c as usize] {
            self.residents.children += 1;
            let at = self.citizens.born[c as usize] + self.days_to_ticks(CHILDHOOD_DAYS) as i64;
            let at = (at.max(0) as u64).max(self.totals.tick + 1);
            self.residents.grown.push(Reverse((at, c)));
        }
    }

    /// Births, coming of age and grown children leaving, for events due by now.
    pub(in crate::world) fn tick_families(&mut self) {
        let now = self.totals.tick;
        for _ in 0..PER_TICK {
            match self.residents.grown.peek() {
                Some(&Reverse((at, c))) if at <= now => {
                    self.residents.grown.pop();
                    self.grow_up(c);
                }
                _ => break,
            }
        }
        for _ in 0..PER_TICK {
            match self.residents.movers.first() {
                Some(&(at, c)) if at <= now => self.leave_alone(c),
                _ => break,
            }
        }
        for _ in 0..PER_TICK {
            match self.residents.births.peek() {
                Some(&Reverse((at, hh))) if at <= now => {
                    self.residents.births.pop();
                    self.check_birth(hh);
                }
                _ => break,
            }
        }
    }

    /// One birth check: a household with an adult and room may have a baby. Checks repeat
    /// while the household lives here.
    pub(in crate::world) fn check_birth(&mut self, household: HouseholdId) {
        let hh = household as usize;
        if !self.households.active[hh] {
            return;
        }
        let roll = self.residents.birth_rng.below(100);
        let home = self.households.home[hh] as usize;
        let members = &self.households.members[hh];
        let adult = members.iter().any(|&c| !self.citizens.child[c as usize]);
        let room = members.len() < self.homes.capacity[home] as usize;
        if adult && room && roll < BIRTH_PERCENT {
            let b = self.homes.building[home];
            let born = self.totals.tick as i64;
            let c = self
                .citizens
                .push(household, b, born, true, &mut self.residents.birth_rng);
            self.add_member(household, c);
        }
        let next = self.totals.tick + self.days_to_ticks(BIRTH_CHECK_DAYS);
        self.residents.births.push(Reverse((next, household)));
    }

    /// A child comes of age: looks for work, goes about the day and wants a home of their own.
    fn grow_up(&mut self, c: CitizenId) {
        let i = c as usize;
        if !self.citizens.active[i] || !self.citizens.child[i] {
            return;
        }
        self.citizens.child[i] = false;
        self.residents.children -= 1;
        let deadline = self.totals.tick + self.days_to_ticks(MOVE_OUT_DAYS);
        self.citizens.move_by[i] = deadline;
        self.residents.movers.insert((deadline, c));
        self.schedule_day(c);
        self.schedule_search(c, 1.0);
    }

    /// Stops `c` waiting for a home of their own.
    fn stop_waiting(&mut self, c: CitizenId) {
        let i = c as usize;
        let deadline = std::mem::replace(&mut self.citizens.move_by[i], u64::MAX);
        self.residents.movers.remove(&(deadline, c));
    }

    /// Bookkeeping for a member leaving the city with their household.
    pub(in crate::world) fn member_left(&mut self, c: CitizenId) {
        self.stop_waiting(c);
        if self.citizens.child[c as usize] {
            self.residents.children -= 1;
        }
    }

    /// A grown child found no home in time and leaves the city alone.
    fn leave_alone(&mut self, c: CitizenId) {
        let i = c as usize;
        self.stop_waiting(c);
        let household = self.citizens.household[i];
        self.households.members[household as usize].retain(|&m| m != c);
        self.cancel_citizen_trip(c);
        self.release_job(c);
        self.citizens.active[i] = false;
        self.citizens.job_due[i] = u64::MAX;
        self.residents.population -= 1;
        if self.households.members[household as usize].is_empty() {
            self.household_leaves_city(household);
        }
    }

    /// Gives vacant `home` to the grown child who has waited longest, ahead of an arriving
    /// household. They keep their job and draw a car as anyone moving in does (ADR 0026).
    pub(in crate::world) fn move_in_grown_child(&mut self, home: HomeId) -> bool {
        let Some(&(_, c)) = self.residents.movers.first() else {
            return false;
        };
        let i = c as usize;
        self.stop_waiting(c);
        let old = self.citizens.household[i] as usize;
        self.households.members[old].retain(|&m| m != c);
        let household = self.found_household(home);
        self.households.members[household as usize].push(c);
        self.citizens.household[i] = household;
        self.cancel_citizen_trip(c);
        self.citizens.location[i] = self.homes.building[home as usize];
        self.citizens.activity[i] = Activity::Home;
        self.citizens.car_out[i] = false;
        self.citizens.draw_car(c, true, &mut self.residents.rng);
        self.schedule_day(c);
        if self.households.members[old].is_empty() {
            self.household_leaves_city(old as HouseholdId);
        }
        true
    }
}
