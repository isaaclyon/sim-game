//! Lifts (ADR 0055, slice 5): a child whose school is past their walking reach is driven by a
//! household adult with a car. The adult's car trip is an ordinary citizen trip with the
//! activity `SchoolRun`; the children ride along without a trip of their own and are moved
//! when the adult arrives. A waiting child and a riding child each keep a deadline, so a lift
//! that never comes or never ends still sends them home.

use crate::building::BuildingId;
use crate::citizen::{Activity, CitizenId};
use crate::clock::CLOCK_SECONDS_PER_SIM_SECOND;
use crate::world::World;

const fn clock(seconds: u32) -> f32 {
    seconds as f32 / CLOCK_SECONDS_PER_SIM_SECOND as f32
}
/// An adult leaving home takes the children due to leave within this long: 90 clock minutes.
const LEAD_S: f32 = clock(90 * 60);
/// The stop at the school door or at home between two legs: about a clock minute.
const DOOR_S: f32 = 3.0;
/// A child waits this long after school for a lift, then walks home: four clock hours.
const WAIT_S: f32 = clock(4 * 3600);
/// A ride that has not ended after this long puts the child home: six clock hours.
const RIDE_LIMIT_S: f32 = clock(6 * 3600);

impl World {
    fn household_of(&self, c: CitizenId) -> Vec<CitizenId> {
        self.households.members[self.citizens.household[c as usize] as usize].clone()
    }

    /// An adult with a car of their own.
    fn can_drive(&self, c: CitizenId) -> bool {
        let i = c as usize;
        self.citizens.active[i] && !self.citizens.child[i] && self.citizens.owns_car[i]
    }

    /// Whether an adult in `c`'s household owns a car, so `c` can be driven to school.
    pub(super) fn household_drives(&self, c: CitizenId) -> bool {
        self.household_of(c).iter().any(|&m| self.can_drive(m))
    }

    /// Whether `school` is past child `c`'s walking reach from home.
    fn needs_lift(&self, c: CitizenId, school: BuildingId) -> bool {
        self.front_distance(self.home_building(c), school) > self.citizens.walk_limit[c as usize]
    }

    fn idle_at_home(&self, c: CitizenId) -> bool {
        let i = c as usize;
        self.citizens.activity[i] == Activity::Home
            && self.citizens.destination[i].is_none()
            && self.citizens.location[i] == self.home_building(c)
    }

    /// The adult driving child `c` right now.
    pub(in crate::world) fn riding_with(&self, c: CitizenId) -> Option<CitizenId> {
        self.schools.ride.get(c as usize).copied().flatten()
    }

    /// Whether child `c` stands at their school after the school day, waiting for a lift.
    fn is_waiting(&self, c: CitizenId) -> bool {
        let i = c as usize;
        self.schools.waiting.get(i) == Some(&true)
            && self.citizens.child[i]
            && self.riding_with(c).is_none()
            && self.citizens.activity[i] == Activity::School
            && self.school_building(c) == Some(self.citizens.location[i])
    }

    /// The school where a child of `c`'s household waits for a lift.
    fn waiting_school(&self, c: CitizenId) -> Option<BuildingId> {
        self.household_of(c)
            .into_iter()
            .find(|&m| self.is_waiting(m))
            .and_then(|m| self.school_building(m))
    }

    /// Child `c` gets into `driver`'s car, bound for `activity`.
    fn board(&mut self, c: CitizenId, driver: CitizenId, activity: Activity) {
        let i = c as usize;
        self.schools.grow(c);
        self.schools.ride[i] = Some(driver);
        self.schools.waiting[i] = false;
        self.citizens.activity[i] = activity;
        self.schedule_activity(c, RIDE_LIMIT_S);
    }

    /// Forgets `c`'s part in any lift; used when their household moves.
    pub(in crate::world) fn forget_lift(&mut self, c: CitizenId) {
        let i = c as usize;
        if i < self.schools.ride.len() {
            self.schools.ride[i] = None;
            self.schools.waiting[i] = false;
        }
    }

    /// Ends child `c`'s ride without an arrival: they are home and plan their next day.
    fn release_rider(&mut self, c: CitizenId) {
        let i = c as usize;
        self.forget_lift(c);
        self.citizens.location[i] = self.home_building(c);
        self.citizens.destination[i] = None;
        self.citizens.activity[i] = Activity::Home;
        self.schedule_day(c);
    }

    /// `driver`'s trip was cancelled: the children in the car are put home.
    pub(in crate::world) fn release_riders(&mut self, driver: CitizenId) {
        if self.schools.ride.is_empty() {
            return;
        }
        for m in self.household_of(driver) {
            if self.riding_with(m) == Some(driver) {
                self.release_rider(m);
            }
        }
    }

    /// The children of `driver`'s household who are home and leave for school soon (or are
    /// `first`, who leaves now) get into the car. Answers the first one's school.
    fn board_at_home(&mut self, driver: CitizenId, first: Option<CitizenId>) -> Option<BuildingId> {
        let soon = self.totals.tick + self.secs_to_ticks(LEAD_S);
        let mut stop = None;
        for m in self.household_of(driver) {
            let i = m as usize;
            let leaving = Some(m) == first || self.citizens.due[i] <= soon;
            if !self.citizens.child[i] || !leaving || !self.idle_at_home(m) {
                continue;
            }
            if let Some(school) = self.school_building(m).filter(|&s| self.needs_lift(m, s)) {
                self.board(m, driver, Activity::School);
                stop.get_or_insert(school);
            }
        }
        stop
    }

    /// `driver` sets out for `school` by car at once.
    fn start_school_run(&mut self, driver: CitizenId, school: BuildingId) {
        let i = driver as usize;
        self.citizens.destination[i] = Some(school);
        self.citizens.activity[i] = Activity::SchoolRun;
        self.citizens.due[i] = u64::MAX;
        self.start_citizen_trip(driver);
    }

    /// The first adult of `c`'s household, by ID, who has a car and is idle at home.
    fn driver_at_home(&self, c: CitizenId) -> Option<CitizenId> {
        self.household_of(c)
            .into_iter()
            .find(|&m| self.can_drive(m) && self.idle_at_home(m))
    }

    /// A rider's deadline passed: the ride never ended, so they are put home. True if `c`
    /// was riding.
    pub(in crate::world) fn ride_overdue(&mut self, c: CitizenId) -> bool {
        let riding = self.riding_with(c).is_some();
        if riding {
            self.release_rider(c);
        }
        riding
    }

    /// The lift rules for a citizen who is about to decide what to do next. True if a lift
    /// took the decision; false leaves it to the ordinary schedule.
    pub(in crate::world) fn lift_decides(&mut self, c: CitizenId) -> bool {
        if self.schools.kind.is_empty() {
            return false;
        }
        let i = c as usize;
        let activity = self.citizens.activity[i];
        if self.citizens.child[i] {
            let Some(school) = self.school_building(c).filter(|&s| self.needs_lift(c, s)) else {
                self.forget_lift(c);
                return false;
            };
            return match activity {
                // Morning: an adult at home drives; with nobody to drive, no school today.
                Activity::Home => {
                    let stop = self
                        .driver_at_home(c)
                        .and_then(|d| Some((d, self.board_at_home(d, Some(c))?)));
                    match stop {
                        Some((driver, stop)) => self.start_school_run(driver, stop),
                        None => self.schedule_day(c),
                    }
                    true
                }
                // The school day is over: wait for a lift, fetched at once by an adult at
                // home. Waiting a second time means nobody came, so the child walks.
                Activity::School
                    if !self.schools.waiting[i] && self.citizens.location[i] == school =>
                {
                    self.schools.waiting[i] = true;
                    self.schedule_activity(c, WAIT_S);
                    if let Some(driver) = self.driver_at_home(c) {
                        self.start_school_run(driver, school);
                    }
                    true
                }
                _ => {
                    self.forget_lift(c);
                    false
                }
            };
        }
        if !self.can_drive(c) {
            return false;
        }
        let stop = match activity {
            // Leaving home: the children who leave soon come along.
            Activity::Home if self.idle_at_home(c) => self.board_at_home(c, None),
            // Out with the car: fetch a waiting child on the way home.
            Activity::Work | Activity::Shop if self.citizens.car_out[i] => self.waiting_school(c),
            _ => None,
        };
        if let Some(school) = stop {
            self.start_school_run(c, school);
        }
        stop.is_some()
    }

    /// `driver` reached a school: children bound for it get out, children waiting there get
    /// in, and the driver plans the next leg, which starts after a moment at the door.
    pub(in crate::world) fn school_run_arrived(&mut self, driver: CitizenId) {
        let i = driver as usize;
        let here = self.citizens.location[i];
        let home = self.home_building(driver);
        let members = self.household_of(driver);
        let mut dropped = false;
        for &m in &members {
            let k = m as usize;
            if self.riding_with(m) == Some(driver) {
                if self.citizens.location[k] == home && self.school_building(m) == Some(here) {
                    self.schools.ride[k] = None;
                    self.citizens.location[k] = here;
                    self.schedule_activity(m, super::SCHOOL_DAY_S);
                    dropped = true;
                }
            } else if self.is_waiting(m) && self.citizens.location[k] == here {
                self.board(m, driver, Activity::Home);
            }
        }
        // Next: another school to drop at, then one to fetch from, then home or work.
        let (mut next, mut carrying) = (None, false);
        for &m in &members {
            if self.riding_with(m) != Some(driver) {
                continue;
            }
            if self.citizens.location[m as usize] != home {
                carrying = true;
            } else if let Some(school) = self.school_building(m) {
                next.get_or_insert(school);
            } else {
                // Their school closed on the way.
                self.release_rider(m);
            }
        }
        let next = next.or_else(|| self.waiting_school(driver));
        let (to, activity) = match (next, self.citizens.job[i]) {
            (Some(school), _) => (school, Activity::SchoolRun),
            (None, Some(job)) if dropped && !carrying => {
                self.citizens.trip_start[i] = self.totals.tick;
                (job, Activity::Work)
            }
            _ => (home, Activity::Home),
        };
        self.citizens.destination[i] = Some(to);
        self.citizens.activity[i] = activity;
        self.schedule_activity(driver, DOOR_S);
    }

    /// `c` got home: the children in their car are home too. A driver with a child still
    /// waiting at school goes out again by car.
    pub(in crate::world) fn lift_arrived_home(&mut self, c: CitizenId) {
        if self.schools.ride.is_empty() || !self.can_drive(c) {
            return;
        }
        for m in self.household_of(c) {
            if self.riding_with(m) == Some(c) {
                self.release_rider(m);
            }
        }
        if let Some(school) = self.waiting_school(c) {
            self.citizens.destination[c as usize] = Some(school);
            self.citizens.activity[c as usize] = Activity::SchoolRun;
            self.schedule_activity(c, DOOR_S);
        }
    }
}
