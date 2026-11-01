use super::super::World;
use crate::agents::AgentState;
use crate::citizen::{Activity, CitizenId};
use crate::clock::clock_at;
use std::cmp::Reverse;

impl World {
    pub(in crate::world) fn schedule_activity(&mut self, c: CitizenId, seconds: f32) {
        let at = self.totals.tick + self.secs_to_ticks(seconds);
        self.citizens.due[c as usize] = at;
        self.residents.activities.push(Reverse((at, c)));
    }
    pub(in crate::world) fn schedule_search(&mut self, c: CitizenId, seconds: f32) {
        let at = self.totals.tick + self.secs_to_ticks(seconds);
        self.citizens.job_due[c as usize] = at;
        self.residents.searches.push(Reverse((at, c)));
    }
    pub(in crate::world) fn tick_residents(&mut self) {
        self.register_buildings();
        self.admit_households();
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
                self.residents.workers[b as usize] += 1;
                self.residents.employed += 1;
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
            if self.citizens.destination[i].is_some() {
                self.start_citizen_trip(c);
                continue;
            }
            let next = match self.citizens.activity[i] {
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
                Activity::Shop => Some((self.home_building(c), Activity::Home)),
            };
            if let Some((b, activity)) = next {
                self.citizens.destination[i] = Some(b);
                self.citizens.activity[i] = activity;
                self.start_citizen_trip(c);
            } else {
                self.schedule_day(c);
            }
        }
    }

    pub(in crate::world) fn schedule_day(&mut self, c: CitizenId) {
        let now = clock_at(self.totals.tick, self.cfg.dt).second_of_day;
        let target = if self.citizens.job[c as usize].is_some() {
            self.citizens.departure[c as usize]
        } else {
            10 * 3600 + self.residents.rng.below(6 * 3600)
        };
        let delay = if target > now {
            target - now
        } else {
            86400 - now + target
        };
        self.schedule_activity(c, delay as f32 / 6.0);
    }

    pub(in crate::world) fn start_citizen_trip(&mut self, c: CitizenId) {
        let i = c as usize;
        let Some(destination) = self.citizens.destination[i] else {
            return;
        };
        let (Some(from), Some(to)) = (
            self.building_node(self.citizens.location[i]),
            self.building_node(destination),
        ) else {
            self.schedule_activity(c, 30.0);
            return;
        };
        if from == to {
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
            Activity::Home => self.schedule_day(c),
            Activity::Work => self.schedule_activity(c, self.citizens.work_duration[i] as f32),
            Activity::Shop => {
                let seconds = 60 + self.residents.rng.below(241);
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
