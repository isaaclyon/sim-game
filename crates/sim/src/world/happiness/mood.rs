//! One citizen's mood and complaints (ADR 0058), derived from where they live, their job and
//! their commute. Read-only: nothing here mutates the world or draws from its RNGs.

use super::super::World;
use super::super::fire::FIRE_THRESHOLD;
use crate::building::BuildingId;
use crate::citizen::CitizenId;
use serde::{Deserialize, Serialize};

/// Mood with nothing good or bad about one's life.
pub const BASE_MOOD: i32 = 60;
const NO_POWER: i32 = 25;
const NO_WATER: i32 = 25;
/// Living within a fire station's reach; the same taken off when uncovered while fires burn.
const FIRE_COVER: i32 = 10;
/// Air and noise bytes below this cost nothing; full costs the whole penalty.
const POLLUTION_FREE: i32 = 64;
const DIRTY_AIR: i32 = 20;
const NOISE: i32 = 15;
const NO_JOB: i32 = 15;
/// Commutes up to this many clock minutes cost nothing; `LONG_COMMUTE_MIN` costs the most.
const SHORT_COMMUTE_MIN: i32 = 40;
const LONG_COMMUTE_MIN: i32 = 100;
const LONG_COMMUTE: i32 = 20;
/// A penalty this small is not worth complaining about.
const MIN_COMPLAINT: i32 = 3;

/// Something that makes a citizen unhappy. The frontend reads these as codes; append only.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[repr(u8)]
pub enum Complaint {
    NoPower = 0,
    NoWater = 1,
    NoFireCover = 2,
    DirtyAir = 3,
    Noise = 4,
    NoJob = 5,
    LongCommute = 6,
}

pub const COMPLAINTS: usize = 7;
const ALL: [Complaint; COMPLAINTS] = [
    Complaint::NoPower,
    Complaint::NoWater,
    Complaint::NoFireCover,
    Complaint::DirtyAir,
    Complaint::Noise,
    Complaint::NoJob,
    Complaint::LongCommute,
];

/// Complaints as one bit per `Complaint` code.
#[derive(Clone, Copy, Default, Debug, PartialEq, Eq)]
pub struct Complaints(pub u8);

impl Complaints {
    fn add(&mut self, c: Complaint) {
        self.0 |= 1 << c as u8;
    }

    pub fn list(self) -> Vec<Complaint> {
        ALL.into_iter()
            .filter(|&c| self.0 & 1 << c as u8 != 0)
            .collect()
    }
}

/// What a home gives everyone living in it, and the pollution cell it counts in.
#[derive(Clone, Copy, Debug)]
pub(super) struct HomeMood {
    pub(super) points: i32,
    pub(super) complaints: Complaints,
    pub(super) cell: Option<usize>,
}

/// A penalty growing linearly from `free` to `full`, at most `most` points.
fn ramp(v: i32, free: i32, full: i32, most: i32) -> i32 {
    (v - free).clamp(0, full - free) * most / (full - free)
}

impl World {
    /// Home factors for building `b`: power, water, fire cover, air and noise.
    pub(super) fn home_mood(&self, b: BuildingId) -> HomeMood {
        let mut points = 0;
        let mut complaints = Complaints::default();
        let mut cost = |p: i32, c: Complaint, points: &mut i32| {
            *points -= p;
            if p >= MIN_COMPLAINT {
                complaints.add(c);
            }
        };
        if !self.building_powered(b) {
            cost(NO_POWER, Complaint::NoPower, &mut points);
        }
        if !self.building_watered(b) {
            cost(NO_WATER, Complaint::NoWater, &mut points);
        }
        if self.building_covered(b) {
            points += FIRE_COVER;
        } else if self.fire.enabled && self.residents.population >= FIRE_THRESHOLD {
            cost(FIRE_COVER, Complaint::NoFireCover, &mut points);
        }
        let ring = self.buildings.footprint(b);
        let n = ring.len().max(1) as f32;
        let (x, y) = ring
            .iter()
            .fold((0.0, 0.0), |(x, y), p| (x + p.0 / n, y + p.1 / n));
        let cell = self.pollution.cell_at(x, y);
        if let Some(c) = cell {
            let (air, noise) = self.pollution.air_noise(c);
            let full = i32::from(u8::MAX);
            let air = ramp(i32::from(air), POLLUTION_FREE, full, DIRTY_AIR);
            cost(air, Complaint::DirtyAir, &mut points);
            let noise = ramp(i32::from(noise), POLLUTION_FREE, full, NOISE);
            cost(noise, Complaint::Noise, &mut points);
        }
        HomeMood {
            points,
            complaints,
            cell,
        }
    }

    /// Mood 0 to 100 and complaints of citizen `c`, living in a home with `home`'s factors.
    /// Children take only their home's.
    pub(super) fn person_mood(&self, c: CitizenId, home: HomeMood) -> (u8, Complaints) {
        let i = c as usize;
        let (mut points, mut complaints) = (BASE_MOOD + home.points, home.complaints);
        if !self.citizens.child[i] {
            if self.citizens.job[i].is_none() {
                points -= NO_JOB;
                complaints.add(Complaint::NoJob);
            }
            let commute = i32::from(self.citizens.commute[i]);
            let p = ramp(commute, SHORT_COMMUTE_MIN, LONG_COMMUTE_MIN, LONG_COMMUTE);
            if p > 0 {
                points -= p;
                if p >= MIN_COMPLAINT {
                    complaints.add(Complaint::LongCommute);
                }
            }
        }
        (points.clamp(0, 100) as u8, complaints)
    }

    /// Mood and complaints of resident `c` now, from the current derived state.
    pub fn citizen_mood(&self, c: CitizenId) -> (u8, Complaints) {
        let home = self.home_mood(self.home_building(c));
        self.person_mood(c, home)
    }
}
