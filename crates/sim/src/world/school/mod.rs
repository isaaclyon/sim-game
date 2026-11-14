//! Schools (ADR 0055): primary and high schools placed beside roads, the children enrolled in
//! them, and what staying enrolled teaches.
//!
//! A school is a lot-less building of a service kind (`Zone::PrimarySchool`, `Zone::HighSchool`)
//! plus a record here. Schools, seats, enrolment and stage events are stored, saved and hashed.
//! Road access and road distances from each school are derived from roads and schools and
//! rebuilt whenever either changes.

mod enrol;
mod lift;
mod place;
mod run;
#[cfg(test)]
mod tests;
mod view;

use std::cmp::Reverse;
use std::collections::BinaryHeap;

use serde::{Deserialize, Serialize};

use super::World;
use super::fire::distances;
use crate::building::BuildingId;
use crate::citizen::CitizenId;
use crate::geometry::Point;
use crate::pollution::Sources;
use crate::power::Footprint;
use crate::road::{NodeId, RoadId};
use crate::zone::Zone;
pub(in crate::world) use run::SCHOOL_DAY_S;
pub use view::{
    AREA_HIGH, AREA_PRIMARY, EducationStats, SCHOOL_ROAD_STRIDE, SCHOOL_VIEW_STRIDE, SchoolInfo,
};

/// Floats per school in the schools snapshot: `[kind, x, y, half_size, front_x, front_y]`.
pub const SCHOOL_STRIDE: usize = 6;
/// Seat blocks a new school opens with (ADR 0056 lets budgets change them later).
pub const START_BLOCKS: u32 = 3;
/// Land value the cells of a home inside a school's area gain, as a share of full value.
pub const SCHOOL_BOOST: f32 = 0.1;
/// The school building stands this far inside its site on every side, metres.
const SITE_INSET: f32 = 3.0;

/// Which stage of childhood a school teaches.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[repr(u8)]
pub enum SchoolKind {
    /// The first half of childhood.
    Primary = 0,
    /// The second half.
    High = 1,
}

impl SchoolKind {
    pub fn from_u8(v: u8) -> Option<Self> {
        match v {
            0 => Some(Self::Primary),
            1 => Some(Self::High),
            _ => None,
        }
    }

    /// The building kind a school of this kind is stored as.
    pub fn zone(self) -> Zone {
        match self {
            Self::Primary => Zone::PrimarySchool,
            Self::High => Zone::HighSchool,
        }
    }

    /// Half the side of the square site, metres.
    pub fn half_size(self) -> f32 {
        match self {
            Self::Primary => 14.0,
            Self::High => 18.0,
        }
    }

    /// Building height, metres.
    pub fn height(self) -> f32 {
        match self {
            Self::Primary => 5.0,
            Self::High => 8.0,
        }
    }

    /// What placing one costs, dollars.
    pub fn cost(self) -> i64 {
        match self {
            Self::Primary => 4_000,
            Self::High => 7_000,
        }
    }

    /// Upkeep per seat block per clock hour, dollars.
    pub fn block_upkeep(self) -> i64 {
        match self {
            Self::Primary => 20,
            Self::High => 35,
        }
    }

    /// Seats per seat block.
    pub fn block_seats(self) -> u32 {
        match self {
            Self::Primary => 30,
            Self::High => 40,
        }
    }

    /// Teacher jobs per seat block.
    pub fn block_teachers(self) -> u32 {
        match self {
            Self::Primary => 2,
            Self::High => 3,
        }
    }

    /// A school covers homes whose frontage junction is this far along roads, metres.
    pub fn cover_m(self) -> f32 {
        match self {
            Self::Primary => 800.0,
            Self::High => 1_200.0,
        }
    }
}

/// Derived from roads and schools; never saved.
#[derive(Default)]
struct Cache {
    /// Roads and schools versions the rest is for.
    key: Option<(u32, u32)>,
    /// Per school: the road it faces and that road's nearer junction.
    access: Vec<Option<(RoadId, NodeId)>>,
    /// Per school, per node: driving distance from the school, unreached past its cover.
    dist: Vec<Vec<f32>>,
}

/// The city's schools. School IDs are indices and append-only.
#[derive(Default, Serialize, Deserialize)]
pub struct Schools {
    pub kind: Vec<SchoolKind>,
    pub x: Vec<f32>,
    pub y: Vec<f32>,
    /// Unit direction the school's front faces: toward its road.
    pub fx: Vec<f32>,
    pub fy: Vec<f32>,
    pub active: Vec<bool>,
    /// The service building standing on the site.
    pub building: Vec<BuildingId>,
    /// Seat blocks; seats and teacher jobs follow (ADR 0056).
    pub blocks: Vec<u32>,
    /// Seats taken.
    pub pupils: Vec<u32>,
    /// Per citizen ID, grown on demand: the school a child is enrolled in.
    seat: Vec<Option<u32>>,
    /// Per citizen ID: the tick the child took their seat.
    since: Vec<u64>,
    /// Per citizen ID: ticks of the current stage spent enrolled before the current seat.
    schooled: Vec<u64>,
    /// When each child moves from primary to high school age.
    stages: BinaryHeap<Reverse<(u64, CitizenId)>>,
    /// Seats or roads changed: children without a seat look again on the next tick.
    pending: bool,
    /// Per citizen ID: the adult whose car a child is riding in (a lift).
    ride: Vec<Option<CitizenId>>,
    /// Per citizen ID: the child waits at school for a lift home.
    waiting: Vec<bool>,
    /// Bumped whenever a school is added or removed.
    #[serde(skip)]
    pub version: u32,
    #[serde(skip)]
    cache: Cache,
}

impl Schools {
    pub fn active_schools(&self) -> impl Iterator<Item = u32> + '_ {
        let on = &self.active;
        (0..on.len()).filter(|&s| on[s]).map(|s| s as u32)
    }

    pub fn footprint(&self, s: u32) -> Footprint {
        let i = s as usize;
        Footprint {
            centre: (self.x[i], self.y[i]),
            front: (self.fx[i], self.fy[i]),
            half: self.kind[i].half_size(),
        }
    }

    /// Seats school `s` has.
    pub fn seats(&self, s: u32) -> u32 {
        self.blocks[s as usize] * self.kind[s as usize].block_seats()
    }

    /// Teacher jobs school `s` has.
    pub fn teachers(&self, s: u32) -> u32 {
        self.blocks[s as usize] * self.kind[s as usize].block_teachers()
    }

    /// The school child `c` is enrolled in.
    pub fn school_of(&self, c: CitizenId) -> Option<u32> {
        self.seat.get(c as usize).copied().flatten()
    }

    /// The active school whose building is `b`.
    pub fn of_building(&self, b: BuildingId) -> Option<u32> {
        self.active_schools()
            .find(|&s| self.building[s as usize] == b)
    }

    fn bump(&mut self) {
        self.version = self.version.wrapping_add(1);
    }

    fn grow(&mut self, c: CitizenId) {
        let n = c as usize + 1;
        if self.seat.len() < n {
            self.seat.resize(n, None);
            self.since.resize(n, 0);
            self.schooled.resize(n, 0);
            self.ride.resize(n, None);
            self.waiting.resize(n, false);
        }
    }

    /// Mixes nothing for a city that never had a school, so cities from before schools hash
    /// as they did.
    pub(super) fn hash_into(&self, mix: &mut impl FnMut(u64)) {
        if self.kind.is_empty() {
            return;
        }
        mix(self.kind.len() as u64);
        for s in 0..self.kind.len() {
            mix(u64::from(self.active[s]) << 8 | self.kind[s] as u64);
            mix(u64::from(self.x[s].to_bits()) << 32 | u64::from(self.y[s].to_bits()));
            mix(u64::from(self.fx[s].to_bits()) << 32 | u64::from(self.fy[s].to_bits()));
            mix(u64::from(self.building[s]) << 32 | u64::from(self.blocks[s]));
            mix(u64::from(self.pupils[s]));
        }
        for c in 0..self.seat.len() {
            if self.seat[c].is_some() || self.schooled[c] != 0 {
                mix(c as u64);
                mix(self.seat[c].map_or(u64::MAX, u64::from));
                mix(self.since[c]);
                mix(self.schooled[c]);
            }
        }
        let mut stages: Vec<_> = self.stages.iter().map(|&Reverse(e)| e).collect();
        stages.sort_unstable();
        mix(stages.len() as u64);
        for (at, c) in stages {
            mix(at);
            mix(u64::from(c));
        }
        mix(u64::from(self.pending));
        for c in 0..self.ride.len() {
            if self.ride[c].is_some() || self.waiting[c] {
                mix(c as u64);
                mix(self.ride[c].map_or(u64::MAX, u64::from) << 1 | u64::from(self.waiting[c]));
            }
        }
    }

    /// Internal consistency of loaded school state (ADR 0033).
    pub(super) fn check(&self, buildings: usize, citizens: usize) -> crate::check::Check {
        use crate::check::{below, ensure, finite, lens};
        let n = self.kind.len();
        let fields = [
            self.x.len(),
            self.y.len(),
            self.fx.len(),
            self.fy.len(),
            self.active.len(),
            self.building.len(),
            self.blocks.len(),
            self.pupils.len(),
        ];
        lens(&fields, n, "school fields")?;
        finite(
            self.x.iter().chain(&self.y).chain(&self.fx).chain(&self.fy),
            "school positions",
        )?;
        below(&self.building, buildings, "school buildings")?;
        ensure(self.seat.len() <= citizens, "school seats")?;
        lens(
            &[
                self.since.len(),
                self.schooled.len(),
                self.ride.len(),
                self.waiting.len(),
            ],
            self.seat.len(),
            "enrolment fields",
        )?;
        below(self.seat.iter().flatten(), n, "enrolled schools")?;
        below(self.ride.iter().flatten(), citizens, "lift drivers")?;
        below(
            self.stages.iter().map(|Reverse((_, c))| c),
            citizens,
            "school stages",
        )?;
        let mut pupils = vec![0u32; n];
        for &s in self.seat.iter().flatten() {
            pupils[s as usize] += 1;
        }
        ensure(pupils == self.pupils, "school pupil counts")?;
        ensure(
            (0..n).all(|s| self.active[s] || pupils[s] == 0),
            "pupils at removed schools",
        )?;
        ensure(
            (0..n).all(|s| self.blocks[s] <= 1_000 && pupils[s] <= self.seats(s as u32)),
            "school seats taken",
        )
    }
}

/// School figures: schools standing, their seats, and seats taken.
#[derive(Clone, Copy, Default, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SchoolStats {
    pub schools: u32,
    pub seats: u32,
    pub pupils: u32,
}

impl World {
    /// Figures for schools of one kind.
    pub fn school_stats(&self, kind: SchoolKind) -> SchoolStats {
        let mut stats = SchoolStats::default();
        for s in self.schools.active_schools() {
            if self.schools.kind[s as usize] == kind {
                stats.schools += 1;
                stats.seats += self.schools.seats(s);
                stats.pupils += self.schools.pupils[s as usize];
            }
        }
        stats
    }

    /// School upkeep per clock hour.
    pub(super) fn school_upkeep(&self) -> i64 {
        let s = &self.schools;
        s.active_schools()
            .map(|i| i64::from(s.blocks[i as usize]) * s.kind[i as usize].block_upkeep())
            .sum()
    }

    /// The road school building `b` faces and its door: the middle of its site's front edge.
    pub(super) fn school_access(&self, b: BuildingId) -> Option<(RoadId, Point)> {
        let s = self.schools.of_building(b)?;
        let (road, _) = self.schools.cache.access.get(s as usize).copied()??;
        if !self.roads.is_road_active(road) {
            return None;
        }
        let at = self.schools.footprint(s);
        let door = (
            at.centre.0 + at.front.0 * at.half,
            at.centre.1 + at.front.1 * at.half,
        );
        Some((road, door))
    }

    /// Brings road access and road distances up to date. When roads or schools changed since
    /// they were last derived, children without a seat look again.
    pub(super) fn ensure_schools(&mut self) {
        let key = (self.roads.version, self.schools.version);
        if self.schools.cache.key == Some(key) {
            return;
        }
        self.roads.rebuild_adjacency();
        let n = self.schools.kind.len();
        let mut access = vec![None; n];
        let mut dist = vec![Vec::new(); n];
        for s in self.schools.active_schools() {
            let i = s as usize;
            access[i] = self.roadside_access(self.schools.footprint(s));
            if let Some((_, node)) = access[i] {
                dist[i] = distances(&self.roads, node, self.schools.kind[i].cover_m());
            }
        }
        let cache = &mut self.schools.cache;
        // After a load nothing changed: the saved city already knew whether anyone waits.
        let changed = cache.key.is_some();
        cache.key = Some(key);
        cache.access = access;
        cache.dist = dist;
        if changed && n > 0 {
            self.schools.pending = true;
        }
    }

    /// Driving distance from school `s` to `node`, if the school's area reaches it.
    fn school_distance(&self, s: u32, node: NodeId) -> Option<f32> {
        let d = *self
            .schools
            .cache
            .dist
            .get(s as usize)?
            .get(node as usize)?;
        (d <= self.schools.kind[s as usize].cover_m()).then_some(d)
    }

    /// Raises land value on the cells of homes inside a school's area (ADR 0055). A home in
    /// several areas gains once.
    pub(super) fn school_boost(&self, src: &mut Sources) {
        if self.schools.active_schools().next().is_none() {
            return;
        }
        let cols = self.zones.cols;
        let mut seen = vec![false; src.boost.len()];
        for b in self.buildings.active_buildings() {
            let i = b as usize;
            let home = matches!(
                self.buildings.zone[i],
                Zone::ResidentialLow | Zone::ResidentialHigh
            );
            let Some(node) = self.building_node(b).filter(|_| home) else {
                continue;
            };
            let covered = self
                .schools
                .active_schools()
                .any(|s| self.school_distance(s, node).is_some());
            if !covered {
                continue;
            }
            for &cell in self.lots.cells(self.buildings.lot[i]) {
                let c = self.pollution.cell_of_zone_cell(cell, cols);
                if !std::mem::replace(&mut seen[c], true) {
                    src.boost[c] += SCHOOL_BOOST;
                }
            }
        }
    }
}
