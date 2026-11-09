//! Buildings on lots (ADR 0016/0018). A zoned lot builds after a seeded delay, and only
//! while its use has demand; otherwise it waits and checks again (ADR 0025).
//! Lot geometry is read only at construction; rezoning never changes an existing building,
//! it only starts the building's gradual closure (ADR 0023). The bulldozer demolishes a building
//! at once; a lot's replacement building gets a fresh design (ADR 0045).
use serde::{Deserialize, Serialize};
mod footprint;

use std::cmp::Reverse;
use std::collections::BinaryHeap;

use crate::era::Era;
use crate::geometry::Point;
use crate::lot::{LotId, Lots};
use crate::rng::Rng;
use crate::road::RoadNetwork;
use crate::zone::{Zone, ZoneGrid};

pub type BuildingId = u32;

/// Why a scheduled lot last waited instead of building.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[repr(u8)]
pub enum Wait {
    /// Not yet checked, or waiting for demand or land value (ADR 0025, 0037).
    Demand = 0,
    /// Its building would have no power (ADR 0041).
    Power = 1,
    /// Its dense building would have no water (ADR 0052).
    Water = 2,
}

/// Sim seconds a due lot waits before checking demand again. Tuning, not configuration.
const DEFER_MIN_S: f32 = 10.0;
const DEFER_MAX_S: f32 = 30.0;

#[derive(Default, Serialize, Deserialize)]
pub struct Buildings {
    pub lot: Vec<LotId>,
    pub zone: Vec<Zone>,
    pub height: Vec<f32>,
    /// Stable palette choice; zero outside industry.
    pub roof: Vec<u8>,
    /// The city's era when the building went up; kept for life, looks only (ADR 0059).
    pub era: Vec<Era>,
    pub active: Vec<bool>,
    outline_start: Vec<u32>,
    outline: Vec<Point>,
    /// Last building put up on each lot; it stands only while active. A lot's next building
    /// draws its design from this one, so a replacement differs from what stood before.
    owner: Vec<Option<BuildingId>>,
    /// Scheduled construction per lot: (tick, zone, why it last waited).
    /// The waiting reason survives saves and disappears with the schedule on unzoning.
    due: Vec<Option<(u64, Zone, Wait)>>,
    pending: BinaryHeap<Reverse<(u64, LotId)>>,
    pub version: u32,
}

impl Buildings {
    pub fn active_buildings(&self) -> impl Iterator<Item = BuildingId> + '_ {
        self.active
            .iter()
            .enumerate()
            .filter_map(|(b, &on)| on.then_some(b as BuildingId))
    }

    /// Counter-clockwise ring in simulation coordinates; no repeated closing point.
    pub fn footprint(&self, building: BuildingId) -> &[Point] {
        let b = building as usize;
        &self.outline[self.outline_start[b] as usize..self.outline_start[b + 1] as usize]
    }

    /// Called only after zoning or lot edits, never on the traffic tick's idle path.
    pub(crate) fn reconcile(
        &mut self,
        lots: &Lots,
        zones: &ZoneGrid,
        tick: u64,
        dt: f32,
        seed: u64,
    ) {
        self.owner.resize(lots.len(), None);
        self.due.resize(lots.len(), None);
        let mut changed = false;
        for l in 0..lots.len() {
            if !lots.active[l] {
                if let Some(b) = self.owner[l].take().filter(|&b| self.active[b as usize]) {
                    self.active[b as usize] = false;
                    changed = true;
                }
                self.due[l] = None;
                continue;
            }
            if self.owner[l].is_some_and(|b| self.active[b as usize]) {
                continue;
            }
            let zone = lots.zone(l as LotId, zones);
            if zone == Zone::None {
                self.due[l] = None;
            } else if self.due[l].is_none_or(|(_, old, _)| old != zone) {
                self.schedule(l, zone, tick, dt, seed);
            }
        }
        self.pending = self
            .due
            .iter()
            .enumerate()
            .filter_map(|(l, due)| due.map(|(at, _, _)| Reverse((at, l as LotId))))
            .collect();
        if changed {
            self.version = self.version.wrapping_add(1);
        }
    }

    fn schedule(&mut self, l: usize, zone: Zone, tick: u64, dt: f32, seed: u64) {
        // Per-lot RNG keeps construction independent of traffic RNG and edit order.
        let mut rng = Rng::new(seed.wrapping_add(l as u64));
        let seconds = 2.0 + rng.below(580) as f32 / 10.0;
        let delay = (seconds / dt).ceil().max(1.0) as u64;
        let at = tick.saturating_add(delay);
        self.due[l] = Some((at, zone, Wait::Demand));
        self.pending.push(Reverse((at, l as LotId)));
    }

    /// Removes a closed or demolished building and schedules its lot for the lot's current zone. Only the
    /// lot's active owner can be removed, so a stale id never frees a replacement's lot.
    /// Occupancy cleanup is the caller's job.
    pub(crate) fn remove(
        &mut self,
        building: BuildingId,
        lots: &Lots,
        zones: &ZoneGrid,
        tick: u64,
        dt: f32,
        seed: u64,
    ) -> bool {
        let b = building as usize;
        let l = self.lot[b] as usize;
        if !self.active[b] || self.owner[l] != Some(building) {
            return false;
        }
        self.active[b] = false;
        self.version = self.version.wrapping_add(1);
        let zone = lots.zone(l as LotId, zones);
        if lots.active[l] && zone != Zone::None {
            self.schedule(l, zone, tick, dt, seed);
        }
        true
    }

    /// Pops the next lot whose construction is due by `tick`, skipping stale heap entries.
    pub(crate) fn next_due(&mut self, tick: u64) -> Option<(LotId, Zone)> {
        while let Some(&Reverse((at, lot))) = self.pending.peek() {
            if at > tick {
                return None;
            }
            self.pending.pop();
            // Entries whose deadline was replaced are stale.
            if let Some((_, zone, _)) = self.due[lot as usize].take_if(|&mut (due, _, _)| due == at)
            {
                return Some((lot, zone));
            }
        }
        None
    }

    /// Puts a due lot back to wait for demand, power or water (ADR 0025, 0041, 0052). The retry delay is
    /// drawn from the lot and tick, independent of traffic RNG and edit order.
    pub(crate) fn defer(
        &mut self,
        lot: LotId,
        zone: Zone,
        tick: u64,
        dt: f32,
        seed: u64,
        reason: Wait,
    ) {
        let mut rng = Rng::new(seed ^ (u64::from(lot) << 32) ^ tick.wrapping_mul(0x9E37_79B9));
        let seconds =
            DEFER_MIN_S + rng.below(((DEFER_MAX_S - DEFER_MIN_S) * 10.0) as u32) as f32 / 10.0;
        let at = tick.saturating_add((seconds / dt).ceil().max(1.0) as u64);
        self.due[lot as usize] = Some((at, zone, reason));
        self.pending.push(Reverse((at, lot)));
    }

    /// Scheduled lots whose most recent growth check failed for lack of power.
    pub(crate) fn waiting_for_power(&self) -> u32 {
        self.waiting_for(Wait::Power)
    }

    /// Scheduled lots whose most recent growth check failed for lack of water.
    pub(crate) fn waiting_for_water(&self) -> u32 {
        self.waiting_for(Wait::Water)
    }

    fn waiting_for(&self, reason: Wait) -> u32 {
        self.due
            .iter()
            .flatten()
            .filter(|(_, _, why)| *why == reason)
            .count() as u32
    }

    /// The building a lot that `next_due` returned would get; `None` if nothing fits.
    /// Changes nothing, so the caller can check power and water first (ADR 0041, 0052).
    pub(crate) fn plan(
        &self,
        lot: LotId,
        zone: Zone,
        lots: &Lots,
        roads: &RoadNetwork,
        seed: u64,
    ) -> Option<Plan> {
        let l = lot as usize;
        // A lot's first building keeps the original per-lot stream; a replacement mixes in the
        // building it replaces (ADR 0045).
        let previous = self.owner[l].map_or(0, |b| {
            (u64::from(b) + 1).wrapping_mul(0x9E37_79B9_7F4A_7C15)
        });
        let mut rng = Rng::new(seed.wrapping_add(lot as u64) ^ previous);
        // The first draw sets growth time; use the next so height is independent.
        rng.next_u64();
        let height = match zone {
            Zone::ResidentialLow => 3.0 * (1 + rng.below(2)) as f32,
            Zone::ResidentialHigh => 3.0 * (4 + rng.below(5)) as f32,
            Zone::CommercialLow => {
                if rng.below(5) == 0 {
                    8.0
                } else {
                    4.0
                }
            }
            Zone::CommercialHigh => 4.0 * (3 + rng.below(5)) as f32,
            Zone::IndustrialLow | Zone::IndustrialHigh => {
                let base = if lots.frontage[l] < 24.0 {
                    4.0
                } else if lots.frontage[l] < 48.0 {
                    6.0
                } else {
                    8.0
                };
                base + rng.below(4) as f32
                    + if zone == Zone::IndustrialHigh {
                        2.0
                    } else {
                        0.0
                    }
            }
            Zone::None => unreachable!("unzoned lots are not scheduled"),
        };
        let roof = if matches!(zone, Zone::IndustrialLow | Zone::IndustrialHigh) {
            rng.below(4) as u8
        } else {
            0
        };
        let ring = footprint::make(lots, lot, roads, zone, &mut rng)?;
        Some(Plan {
            lot,
            zone,
            height,
            roof,
            ring,
        })
    }

    /// Puts up a planned building in the city's `era`.
    pub(crate) fn commit(&mut self, plan: Plan, era: Era) {
        let id = self.active.len() as BuildingId;
        self.owner[plan.lot as usize] = Some(id);
        self.lot.push(plan.lot);
        self.zone.push(plan.zone);
        self.height.push(plan.height);
        self.roof.push(plan.roof);
        self.era.push(era);
        self.active.push(true);
        if self.outline_start.is_empty() {
            self.outline_start.push(0);
        }
        self.outline.extend(plan.ring);
        self.outline_start.push(self.outline.len() as u32);
        self.version = self.version.wrapping_add(1);
    }

    /// `[zone, height, roof, front, era, n, x0, y0, …]` per active building in id order, not
    /// indexed by id. `front` is the footprint edge (from point `front` to the next) facing the
    /// lot's road; it is derived here for drawing and is not stored or hashed. `era` is the
    /// `Era` it was built in (ADR 0059).
    pub fn write_snapshot(&self, lots: &Lots, out: &mut Vec<f32>) {
        out.clear();
        for b in self.active_buildings() {
            let i = b as usize;
            let ring = self.footprint(b);
            let lot = self.lot[i] as usize;
            let facing = (lots.facing_x[lot], lots.facing_y[lot]);
            out.extend([
                self.zone[i] as u8 as f32,
                self.height[i],
                self.roof[i] as f32,
                front_edge(ring, facing) as f32,
                self.era[i] as u8 as f32,
                ring.len() as f32,
            ]);
            out.extend(ring.iter().flat_map(|p| [p.0, p.1]));
        }
    }

    pub(crate) fn hash_into(&self, mix: &mut impl FnMut(u64)) {
        mix(self.active.len() as u64);
        for b in 0..self.active.len() {
            mix(self.active[b] as u64);
            mix(self.lot[b] as u64);
            mix(self.zone[b] as u64);
            mix(self.height[b].to_bits() as u64);
            mix(self.roof[b] as u64);
            mix(self.era[b] as u64);
            let ring = self.footprint(b as BuildingId);
            mix(ring.len() as u64);
            for p in ring {
                mix(((p.0.to_bits() as u64) << 32) | p.1.to_bits() as u64);
            }
        }
        mix(self.due.len() as u64);
        for due in &self.due {
            match due {
                Some((tick, zone, reason)) => {
                    mix(1);
                    mix(*tick);
                    mix(*zone as u64);
                    mix(*reason as u64);
                }
                None => mix(0),
            }
        }
    }
}

/// A building `Buildings::plan` chose for a lot, not yet built.
pub(crate) struct Plan {
    lot: LotId,
    pub zone: Zone,
    pub height: f32,
    roof: u8,
    pub ring: Vec<Point>,
}

/// The counter-clockwise ring edge whose outward normal best matches `facing`. Edges shorter
/// than 2 m are skipped unless nothing else is left, so corner slivers never become fronts.
fn front_edge(ring: &[Point], facing: Point) -> usize {
    let score = |k: usize, min: f32| {
        let (a, b) = (ring[k], ring[(k + 1) % ring.len()]);
        let len = (b.0 - a.0).hypot(b.1 - a.1);
        if len < min {
            return f32::NEG_INFINITY;
        }
        ((b.1 - a.1) * facing.0 - (b.0 - a.0) * facing.1) / len
    };
    let best = |min| (0..ring.len()).max_by(|&j, &k| score(j, min).total_cmp(&score(k, min)));
    best(2.0)
        .filter(|&k| score(k, 2.0).is_finite())
        .or_else(|| best(0.0))
        .unwrap_or(0)
}

impl Buildings {
    /// Internal consistency of loaded buildings (ADR 0033).
    pub(crate) fn check(&self, lots: usize) -> crate::check::Check {
        use crate::check::{below, csr, ensure, lens};
        let n = self.active.len();
        let fields = [
            self.lot.len(),
            self.zone.len(),
            self.height.len(),
            self.roof.len(),
            self.era.len(),
        ];
        lens(&fields, n, "building fields")?;
        csr(
            &self.outline_start,
            n,
            self.outline.len(),
            "building outlines",
        )?;
        below(&self.lot, lots, "building lots")?;
        ensure(
            self.owner.len() <= lots && self.due.len() == self.owner.len(),
            "lot schedules",
        )?;
        below(self.owner.iter().flatten(), n, "lot owners")?;
        below(
            self.pending.iter().map(|Reverse((_, lot))| lot),
            self.due.len(),
            "pending lots",
        )
    }
}
