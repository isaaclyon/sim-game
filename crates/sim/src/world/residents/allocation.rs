use super::super::World;
use crate::building::BuildingId;
use crate::citizen::{CitizenId, household_size};
use crate::road::NodeId;
use crate::zone::Zone;
use std::cmp::Reverse;

impl World {
    /// Original use is retained on repaint; only new assignments are blocked. The closing
    /// flag (ADR 0023) is a cheap first test; the zone and access tests remain authoritative.
    pub fn building_accepts_assignments(&self, b: BuildingId) -> bool {
        let i = b as usize;
        self.buildings.active.get(i) == Some(&true)
            && !self.building_closing(b)
            && self.lots.zone(self.buildings.lot[i], &self.zones) == self.buildings.zone[i]
            && self.building_node(b).is_some()
    }

    pub(in crate::world) fn building_node(&self, b: BuildingId) -> Option<NodeId> {
        let i = b as usize;
        if self.buildings.active.get(i) != Some(&true) {
            return None;
        }
        let l = self.buildings.lot[i] as usize;
        let road = self.lots.road[l];
        if !self.lots.active[l] || !self.roads.is_road_active(road) {
            return None;
        }
        let lane = road as usize * 2;
        let (a, z) = (self.roads.lane_from[lane], self.roads.lane_to[lane]);
        let distance = |n: NodeId| {
            (self.roads.node_x[n as usize] - self.lots.front_x[l])
                .hypot(self.roads.node_y[n as usize] - self.lots.front_y[l])
        };
        Some(if distance(a) <= distance(z) { a } else { z })
    }

    pub(in crate::world) fn register_buildings(&mut self) {
        for b in self.residents.registered..self.buildings.active.len() {
            let area = crate::poly::area(&crate::poly::ring(
                self.buildings.footprint(b as u32).iter().copied(),
            ))
            .abs();
            let zone = self.buildings.zone[b];
            let residential = matches!(zone, Zone::ResidentialLow | Zone::ResidentialHigh);
            let jobs = match zone {
                Zone::CommercialLow | Zone::CommercialHigh => {
                    (area * (self.buildings.height[b] / 4.0).floor() / 30.0)
                        .floor()
                        .max(1.0) as u32
                }
                Zone::IndustrialLow | Zone::IndustrialHigh => (area / 60.0).floor().max(1.0) as u32,
                _ => 0,
            };
            self.residents.jobs.push(jobs);
            self.residents.workers.push(0);
            if residential && self.buildings.active[b] {
                let usable = area * (self.buildings.height[b] / 3.0).floor().max(1.0);
                let count = if zone == Zone::ResidentialLow {
                    1
                } else {
                    (usable / 80.0).floor().max(1.0) as u32
                };
                let capacity = (usable / count as f32 / 25.0).floor().clamp(2.0, 7.0) as u8;
                for _ in 0..count {
                    let h = self.homes.building.len() as u32;
                    self.homes.building.push(b as u32);
                    self.homes.capacity.push(capacity);
                    self.homes.household.push(None);
                    if !self.building_closing(b as u32) {
                        self.residents.vacant.insert(h);
                    }
                    let delay = 5 + self.residents.rng.below(41);
                    self.residents.arrivals.push(Reverse((
                        self.totals.tick + self.secs_to_ticks(delay as f32),
                        h,
                    )));
                }
            }
        }
        let first = self.residents.registered;
        self.residents.registered = self.buildings.active.len();
        for b in first..self.residents.registered {
            self.count_supply(b as BuildingId);
        }
    }

    pub(in crate::world) fn admit_households(&mut self) {
        for _ in 0..32 {
            let Some(&Reverse((at, home))) = self.residents.arrivals.peek() else {
                break;
            };
            if at > self.totals.tick {
                break;
            }
            self.residents.arrivals.pop();
            let h = home as usize;
            let b = self.homes.building[h];
            if self.homes.household[h].is_some() || !self.buildings.active[b as usize] {
                continue;
            }
            if !self.building_accepts_assignments(b) {
                self.residents
                    .arrivals
                    .push(Reverse((self.totals.tick + self.secs_to_ticks(30.0), home)));
                continue;
            }
            let household = self.households.active.len() as u32;
            self.households.active.push(true);
            self.households.home.push(home);
            self.households.members.push(Vec::new());
            self.homes.household[h] = Some(household);
            self.residents.vacant.remove(&home);
            let size = household_size(self.homes.capacity[h], &mut self.residents.rng);
            for _ in 0..size {
                let c = self.citizens.push(household, b, &mut self.residents.rng);
                self.households.members[household as usize].push(c);
                self.residents.population += 1;
                let delay = 15 + self.residents.rng.below(106);
                self.schedule_activity(c, delay as f32);
                self.schedule_search(c, 1.0);
            }
        }
    }

    /// Roads are bidirectional: a connected component is exact reachability. Rebuild only
    /// after topology changes, so nearby disconnected employers cannot starve reachable ones.
    fn refresh_resident_connectivity(&mut self) {
        if self.residents.connectivity_version == Some(self.roads.version) {
            return;
        }
        let mut component = vec![u32::MAX; self.roads.node_count()];
        let mut pending = Vec::new();
        for start in 0..component.len() {
            if component[start] != u32::MAX {
                continue;
            }
            component[start] = start as u32;
            pending.push(start as u32);
            while let Some(node) = pending.pop() {
                for &lane in self.roads.out_lanes(node) {
                    let to = self.roads.lane_to[lane as usize] as usize;
                    if component[to] == u32::MAX {
                        component[to] = start as u32;
                        pending.push(to as u32);
                    }
                }
            }
        }
        self.residents.component = component;
        self.residents.connectivity_version = Some(self.roads.version);
    }

    /// Seeded distance preference among reachable options. No route searches for assignments.
    /// Without a car at hand, only options within walking reach count (ADR 0026).
    pub(in crate::world) fn choose_destination(
        &mut self,
        c: CitizenId,
        job: bool,
    ) -> Option<BuildingId> {
        let origin = if job {
            self.home_building(c)
        } else {
            self.citizens.location[c as usize]
        };
        let from = self.building_node(origin)?;
        let reach = self.walk_reach(c, job);
        self.refresh_resident_connectivity();
        let mut best: Option<(f32, BuildingId)> = None;
        for b in 0..self.residents.registered {
            let eligible = if job {
                self.residents.workers[b] < self.residents.jobs[b]
            } else {
                matches!(
                    self.buildings.zone[b],
                    Zone::CommercialLow | Zone::CommercialHigh
                )
            };
            if !eligible
                || !self.building_accepts_assignments(b as u32)
                || reach.is_some_and(|r| self.front_distance(origin, b as u32) > r)
            {
                continue;
            }
            let to = self.building_node(b as u32).unwrap();
            if self.residents.component[from as usize] != self.residents.component[to as usize] {
                continue;
            }
            let distance = (self.roads.node_x[from as usize] - self.roads.node_x[to as usize])
                .hypot(self.roads.node_y[from as usize] - self.roads.node_y[to as usize]);
            let score = (distance + 120.0) / self.residents.rng.range(0.25, 1.0);
            if best.is_none_or(|(old, _)| score < old) {
                best = Some((score, b as u32));
            }
        }
        best.map(|(_, b)| b)
    }
}
