use super::super::World;
use crate::agents::AgentState;
use crate::citizen::{Activity, CitizenId};
use crate::clock::{CLOCK_SECONDS_PER_SIM_SECOND, clock_at};
use crate::world::school::SCHOOL_DAY_S;
use std::cmp::Reverse;

impl World {
    pub(in crate::world) fn schedule_activity(&mut self, c: CitizenId, seconds: f32) {
        let at = self.totals.tick + self.secs_to_ticks(seconds);
        self.citizens.due[c as usize] = at;
        self.residents.activities.push(Reverse((at, c)));
    }
    pub(in crate::world) fn schedule_search(&mut self, c: CitizenId, seconds: f32) {
        self.start_seeking(c);
        let at = self.totals.tick + self.secs_to_ticks(seconds);
        self.citizens.job_due[c as usize] = at;
        self.residents.searches.push(Reverse((at, c)));
    }
    pub(in crate::world) fn tick_residents(&mut self) {
        self.register_buildings();
        self.admit_households();
        self.tick_families();
        self.tick_schools();
        for _ in 0..8 {
            let Some(&Reverse((at, c))) = self.residents.searches.peek() else {
                break;
            };
            if at > self.totals.tick {
                break;
            }
            self.residents.searches.pop();
            let i = c as usize;
            if !self.citizens.active[i] || self.citizens.job_due[i] != at {
                continue;
            }
            self.citizens.job_due[i] = u64::MAX;
            if self.citizens.job[i].is_some() {
                continue;
            }
            if let Some(b) = self.choose_destination(c, true) {
                self.citizens.job[i] = Some(b);
                self.citizens.any_job_at[i] = u64::MAX;
                self.residents.workers[b as usize] += 1;
                self.residents.employed += 1;
                // Employment changes the next departure, not the current activity.
                if self.citizens.activity[i] == Activity::Home
                    && self.citizens.destination[i].is_none()
                {
                    self.schedule_day(c);
                }
            } else {
                self.schedule_search(c, 120.0);
            }
        }
        for _ in 0..32 {
            let Some(&Reverse((at, c))) = self.residents.activities.peek() else {
                break;
            };
            if at > self.totals.tick {
                break;
            }
            self.residents.activities.pop();
            let i = c as usize;
            if !self.citizens.active[i] || self.citizens.due[i] != at {
                continue;
            }
            self.citizens.due[i] = u64::MAX;
            if self.ride_overdue(c) {
                continue;
            }
            if self.citizens.destination[i].is_some() {
                self.start_citizen_trip(c);
                continue;
            }
            if self.lift_decides(c) {
                continue;
            }
            let next = match self.citizens.activity[i] {
                // A child's only trip from home is to their school; no draw is made.
                Activity::Home if self.citizens.child[i] => self
                    .school_building(c)
                    .map(|school| (school, Activity::School)),
                Activity::Home => {
                    if let Some(job) = self.citizens.job[i] {
                        Some((job, Activity::Work))
                    } else if self.residents.rng.below(100) < 60 {
                        self.choose_destination(c, false)
                            .map(|b| (b, Activity::Shop))
                    } else {
                        None
                    }
                }
                Activity::Work => {
                    let shop = if self.residents.rng.below(100) < 40 {
                        self.choose_destination(c, false)
                    } else {
                        None
                    };
                    Some(shop.map_or((self.home_building(c), Activity::Home), |b| {
                        (b, Activity::Shop)
                    }))
                }
                Activity::Shop | Activity::School | Activity::SchoolRun => {
                    Some((self.home_building(c), Activity::Home))
                }
            };
            if let Some((b, activity)) = next {
                if activity == Activity::Work {
                    self.citizens.trip_start[i] = self.totals.tick;
                }
                self.citizens.destination[i] = Some(b);
                self.citizens.activity[i] = activity;
                self.start_citizen_trip(c);
            } else {
                self.schedule_day(c);
            }
        }
    }

    /// A child's day is their school day; a child without a seat stays home (ADR 0054, 0055).
    pub(in crate::world) fn schedule_day(&mut self, c: CitizenId) {
        let child = self.citizens.child[c as usize];
        if child && self.school_building(c).is_none() {
            return;
        }
        let now = clock_at(self.totals.tick, self.cfg.dt).second_of_day;
        let target = if child {
            self.school_departure(c)
        } else if self.citizens.job[c as usize].is_some() {
            self.citizens.departure[c as usize]
        } else {
            10 * 3600 + self.residents.rng.below(6 * 3600)
        };
        let delay = if target > now {
            target - now
        } else {
            86400 - now + target
        };
        self.schedule_activity(c, delay as f32 / CLOCK_SECONDS_PER_SIM_SECOND as f32);
    }

    pub(in crate::world) fn start_citizen_trip(&mut self, c: CitizenId) {
        let i = c as usize;
        let Some(destination) = self.citizens.destination[i] else {
            return;
        };
        if destination == self.citizens.location[i] {
            self.totals.trips_completed += 1;
            self.citizen_arrived(c);
            return;
        }
        // The mode is chosen leaving home and kept until home again, so the car comes back.
        let home = self.home_building(c);
        if self.citizens.location[i] == home {
            // A school run is always by car: the children could not walk it.
            self.citizens.car_out[i] = self.citizens.owns_car[i]
                && (self.citizens.activity[i] == Activity::SchoolRun
                    || self.front_distance(home, destination) > self.citizens.walk_limit[i]);
        }
        let (Some(from), Some(to)) = (
            self.building_node(self.citizens.location[i]),
            self.building_node(destination),
        ) else {
            self.schedule_activity(c, 30.0);
            return;
        };
        if from == to && self.citizens.car_out[i] {
            self.totals.trips_completed += 1;
            self.citizen_arrived(c);
            return;
        }
        let agent = if let Some(a) = self.citizens.agent[i] {
            a
        } else {
            let a = self.agents.push(from);
            self.agents.citizen[a as usize] = Some(c);
            self.agents.state[a as usize] = AgentState::Dormant;
            self.citizens.agent[i] = Some(a);
            a
        };
        let a = agent as usize;
        debug_assert!(matches!(self.agents.state[a], AgentState::Dormant));
        self.agents.node[a] = from;
        self.agents.dest[a] = to;
        self.agents.state[a] = AgentState::AwaitingPath;
        self.path_queue.push_back(agent);
    }

    pub(in crate::world) fn citizen_arrived(&mut self, c: CitizenId) {
        let i = c as usize;
        self.citizens.location[i] = self.citizens.destination[i]
            .take()
            .expect("trip destination");
        if let Some(a) = self.citizens.agent[i] {
            self.agents.state[a as usize] = AgentState::Dormant;
        }
        match self.citizens.activity[i] {
            Activity::Home => {
                self.citizens.car_out[i] = false;
                self.schedule_day(c);
                self.lift_arrived_home(c);
            }
            Activity::Work => {
                self.note_commute(c);
                self.schedule_activity(c, self.citizens.work_duration[i] as f32);
            }
            Activity::School => self.schedule_activity(c, SCHOOL_DAY_S),
            Activity::SchoolRun => self.school_run_arrived(c),
            Activity::Shop => {
                // 6 to 30 clock minutes.
                let seconds = 20 + self.residents.rng.below(81);
                self.schedule_activity(c, seconds as f32);
            }
        }
    }

    pub(in crate::world) fn retry_citizen_trip(&mut self, c: CitizenId) {
        if let Some(a) = self.citizens.agent[c as usize] {
            self.agents.state[a as usize] = AgentState::Dormant;
            self.agents.cleared[a as usize] = false;
            if let Some(node) = self.building_node(self.citizens.location[c as usize]) {
                self.agents.node[a as usize] = node;
            }
        }
        self.schedule_activity(c, 30.0);
    }
}
