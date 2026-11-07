use super::super::World;
use crate::building::BuildingId;
use crate::citizen::{CitizenId, household_size};
use crate::road::NodeId;
use crate::zone::Zone;
use std::cmp::Reverse;

/// Employment capacity, not physical attendance. Low shops staff their ground-floor trade;
/// dense commerce uses all floors, while industrial height never implies extra job floors.
fn job_capacity(zone: Zone, area: f32, height: f32) -> u32 {
    match zone {
        Zone::CommercialLow => (area / 25.0).floor().max(2.0) as u32,
        Zone::CommercialHigh => (area * (height / 4.0).floor() / 30.0).floor().max(1.0) as u32,
        Zone::IndustrialLow => (area / 30.0).floor().max(5.0) as u32,
        Zone::IndustrialHigh => (area / 10.0).floor().max(10.0) as u32,
        _ => 0,
    }
}

/// Homes, people per home and jobs a building of this use, footprint area and height has.
pub(in crate::world) struct Capacity {
    pub homes: u32,
    pub home_size: u8,
    pub jobs: u32,
}

pub(in crate::world) fn capacity(zone: Zone, area: f32, height: f32) -> Capacity {
    let mut c = Capacity {
        homes: 0,
        home_size: 0,
        jobs: job_capacity(zone, area, height),
    };
    if matches!(zone, Zone::ResidentialLow | Zone::ResidentialHigh) {
        let usable = area * (height / 3.0).floor().max(1.0);
        c.homes = if zone == Zone::ResidentialLow {
            1
        } else {
            (usable / 80.0).floor().max(1.0) as u32
        };
        c.home_size = (usable / c.homes as f32 / 25.0).floor().clamp(2.0, 7.0) as u8;
    }
    c
}

/// Footprint area of a ring, square metres.
pub(in crate::world) fn ring_area(ring: &[crate::geometry::Point]) -> f32 {
    crate::poly::area(&crate::poly::ring(ring.iter().copied())).abs()
}

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
            let area = ring_area(self.buildings.footprint(b as u32));
            let Capacity {
                homes,
                home_size: capacity,
                jobs,
            } = capacity(self.buildings.zone[b], area, self.buildings.height[b]);
            self.residents.jobs.push(jobs);
            self.residents.workers.push(0);
            if self.buildings.active[b] {
                for _ in 0..homes {
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
            if !self.building_fills_vacancies(b) {
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
                self.schedule_day(c);
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
                    && self.building_powered(b as u32)
                    && self.building_watered(b as u32)
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

#[cfg(test)]
mod capacity_tests {
    use super::{Zone, job_capacity};

    #[test]
    fn low_shops_do_not_staff_every_upper_floor_as_retail() {
        // A compact 200 m² shop needs a small team, not just two employees.
        assert_eq!(job_capacity(Zone::CommercialLow, 200.0, 4.0), 8);
        assert_eq!(job_capacity(Zone::CommercialLow, 200.0, 8.0), 8);
        assert_eq!(job_capacity(Zone::CommercialLow, 130.0, 4.0), 5);
        assert_eq!(job_capacity(Zone::CommercialLow, 256.0, 4.0), 10);
        // Older, larger footprints still scale by ground floor, never upper floors.
        assert_eq!(job_capacity(Zone::CommercialLow, 810.0, 8.0), 32);
        assert_eq!(job_capacity(Zone::CommercialLow, 810.0, 4.0), 32);
        assert_eq!(job_capacity(Zone::CommercialLow, 40.0, 4.0), 2);
        // Dense commerce retains multi-floor capacity rather than becoming corner shops.
        assert_eq!(job_capacity(Zone::CommercialHigh, 810.0, 16.0), 108);
    }

    #[test]
    fn small_industry_has_a_team_and_large_halls_still_scale_by_area() {
        assert_eq!(job_capacity(Zone::IndustrialLow, 45.0, 4.0), 5);
        assert_eq!(job_capacity(Zone::IndustrialHigh, 15.0, 6.0), 10);
        assert_eq!(job_capacity(Zone::IndustrialLow, 900.0, 8.0), 30);
        assert_eq!(job_capacity(Zone::IndustrialHigh, 900.0, 12.0), 90);
        assert_eq!(job_capacity(Zone::IndustrialHigh, 900.0, 20.0), 90);
        assert_eq!(job_capacity(Zone::ResidentialLow, 100.0, 6.0), 0);
    }
}
