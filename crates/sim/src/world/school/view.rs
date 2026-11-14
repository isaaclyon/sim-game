//! What the page shows about schools (ADR 0055, slice 4): one school's seats, the city's
//! education figures and the schools view. Read-only; nothing here is saved or hashed.

use serde::{Deserialize, Serialize};

use super::SchoolKind;
use crate::building::{BuildingId, NO_LOT};
use crate::citizen::Education;
use crate::world::World;
use crate::zone::Zone;

/// Floats per school in `write_school_view`'s schools.
pub const SCHOOL_VIEW_STRIDE: usize = 9;
/// Floats per segment in `write_school_view`'s roads.
pub const SCHOOL_ROAD_STRIDE: usize = 5;
/// Area bits in the schools view: inside a primary school's area.
pub const AREA_PRIMARY: u8 = 1;
/// Inside a high school's area.
pub const AREA_HIGH: u8 = 2;

/// Seats at one school.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SchoolInfo {
    pub kind: SchoolKind,
    pub seats: u32,
    pub pupils: u32,
}

/// How far the city's residents got in school.
#[derive(Clone, Copy, Default, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct EducationStats {
    /// Adults by `Education` level: none, primary, high school, university.
    pub adults: [u32; 4],
    /// Children without a school seat.
    pub unschooled: u32,
}

fn bit(kind: SchoolKind) -> u8 {
    match kind {
        SchoolKind::Primary => AREA_PRIMARY,
        SchoolKind::High => AREA_HIGH,
    }
}

impl World {
    /// Seats and pupils of the school standing as building `b`.
    pub fn school_info(&self, b: BuildingId) -> Option<SchoolInfo> {
        let s = self.schools.of_building(b)?;
        Some(SchoolInfo {
            kind: self.schools.kind[s as usize],
            seats: self.schools.seats(s),
            pupils: self.schools.pupils[s as usize],
        })
    }

    pub fn education_stats(&self) -> EducationStats {
        let mut stats = EducationStats::default();
        for c in 0..self.citizens.active.len() {
            if !self.citizens.active[c] {
                continue;
            }
            if self.citizens.child[c] {
                stats.unschooled += u32::from(self.schools.school_of(c as u32).is_none());
            } else {
                let level = self.citizens.education[c].min(Education::University);
                stats.adults[level as usize] += 1;
            }
        }
        stats
    }

    /// `AREA_*` bits of the school areas that reach `node`.
    fn areas_at(&self, node: u32) -> u8 {
        self.schools
            .active_schools()
            .filter(|&s| self.school_distance(s, node).is_some())
            .fold(0, |bits, s| bits | bit(self.schools.kind[s as usize]))
    }

    /// The schools view. `schools`: `[x, y, half_size, front_x, front_y, kind, seats, pupils,
    /// building]` per school, building its stable building ID. `roads`: `[ax, ay, bx, by, areas]` per rendered segment of a road with an
    /// end inside a school's area. `homes`: `[areas, n, x0, y0, …]` per standing home, its
    /// lot's outline. `areas` are `AREA_*` bits.
    pub fn write_school_view(
        &self,
        schools: &mut Vec<f32>,
        roads: &mut Vec<f32>,
        homes: &mut Vec<f32>,
    ) {
        schools.clear();
        roads.clear();
        homes.clear();
        for s in self.schools.active_schools() {
            let (i, at) = (s as usize, self.schools.footprint(s));
            schools.extend([at.centre.0, at.centre.1, at.half, at.front.0, at.front.1]);
            let kind = self.schools.kind[i] as u8;
            let (seats, pupils) = (self.schools.seats(s), self.schools.pupils[i]);
            schools.extend([f32::from(kind), seats as f32, pupils as f32]);
            schools.push(self.schools.building[i] as f32);
        }
        if schools.is_empty() {
            return;
        }
        for road in self.roads.active_roads() {
            let lane = road as usize * 2;
            let areas =
                self.areas_at(self.roads.lane_from[lane]) | self.areas_at(self.roads.lane_to[lane]);
            if areas == 0 {
                continue;
            }
            for (_, a, _, b) in self.roads.shape(road).segments() {
                roads.extend([a.0, a.1, b.0, b.1, f32::from(areas)]);
            }
        }
        for b in self.buildings.active_buildings() {
            let i = b as usize;
            let home = matches!(
                self.buildings.zone[i],
                Zone::ResidentialLow | Zone::ResidentialHigh
            );
            if !home || self.buildings.lot[i] == NO_LOT {
                continue;
            }
            let areas = self.building_node(b).map_or(0, |n| self.areas_at(n));
            let ring = self.lots.outline(self.buildings.lot[i]);
            homes.extend([f32::from(areas), ring.len() as f32]);
            homes.extend(ring.iter().flat_map(|p| [p.0, p.1]));
        }
    }
}
