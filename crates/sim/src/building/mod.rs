//! Buildings on lots (ADR 0016/0018). A zoned lot builds after a seeded delay, and only
//! while its use has demand; otherwise it waits and checks again (ADR 0024).
//! Lot geometry is read only at construction; rezoning never changes an existing building,
//! it only starts the building's gradual closure (ADR 0023).
mod footprint;

use std::cmp::Reverse;
use std::collections::BinaryHeap;

use crate::geometry::Point;
use crate::lot::{LotId, Lots};
use crate::rng::Rng;
use crate::road::RoadNetwork;
use crate::zone::{Zone, ZoneGrid};

pub type BuildingId = u32;

/// Sim seconds a due lot waits before checking demand again. Tuning, not configuration.
const DEFER_MIN_S: f32 = 10.0;
const DEFER_MAX_S: f32 = 30.0;

#[derive(Default)]
pub struct Buildings {
    pub lot: Vec<LotId>,
    pub zone: Vec<Zone>,
    pub height: Vec<f32>,
    /// Stable palette choice; zero outside industry.
    pub roof: Vec<u8>,
    pub active: Vec<bool>,
    outline_start: Vec<u32>,
    outline: Vec<Point>,
    /// Building occupying each lot, including retired slots.
    owner: Vec<Option<BuildingId>>,
    /// Scheduled construction per lot; the heap is just an index into these deadlines.
    due: Vec<Option<(u64, Zone)>>,
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
                if let Some(b) = self.owner[l].take() {
                    self.active[b as usize] = false;
                    changed = true;
                }
                self.due[l] = None;
                continue;
            }
            if self.owner[l].is_some() {
                continue;
            }
            let zone = lots.zone(l as LotId, zones);
            if zone == Zone::None {
                self.due[l] = None;
            } else if self.due[l].is_none_or(|(_, old)| old != zone) {
                self.schedule(l, zone, tick, dt, seed);
            }
        }
        self.pending = self
            .due
            .iter()
            .enumerate()
            .filter_map(|(l, due)| due.map(|(at, _)| Reverse((at, l as LotId))))
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
        self.due[l] = Some((at, zone));
        self.pending.push(Reverse((at, l as LotId)));
    }

    /// Removes a closed building and schedules its lot for the lot's current zone. Only the
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
        self.owner[l] = None;
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
            if let Some((_, zone)) = self.due[lot as usize].take_if(|&mut (due, _)| due == at) {
                return Some((lot, zone));
            }
        }
        None
    }

    /// Puts a due lot back to wait for demand (ADR 0024). The retry delay is drawn from the
    /// lot and tick, so it stays independent of traffic RNG and edit order.
    pub(crate) fn defer(&mut self, lot: LotId, zone: Zone, tick: u64, dt: f32, seed: u64) {
        let mut rng = Rng::new(seed ^ (u64::from(lot) << 32) ^ tick.wrapping_mul(0x9E37_79B9));
        let seconds =
            DEFER_MIN_S + rng.below(((DEFER_MAX_S - DEFER_MIN_S) * 10.0) as u32) as f32 / 10.0;
        let at = tick.saturating_add((seconds / dt).ceil().max(1.0) as u64);
        self.due[lot as usize] = Some((at, zone));
        self.pending.push(Reverse((at, lot)));
    }

    /// Builds on a lot that `next_due` returned. Returns whether a building went up.
    pub(crate) fn build(
        &mut self,
        lot: LotId,
        zone: Zone,
        lots: &Lots,
        roads: &RoadNetwork,
        seed: u64,
    ) -> bool {
        let l = lot as usize;
        let mut rng = Rng::new(seed.wrapping_add(lot as u64));
        // The first draw sets growth time; use the next so height is independent.
        rng.next_u64();
        let height = match zone {
            Zone::ResidentialLow => 3.0 * (1 + rng.below(2)) as f32,
            Zone::ResidentialHigh => 3.0 * (4 + rng.below(5)) as f32,
            Zone::CommercialLow => 4.0 * (1 + rng.below(2)) as f32,
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
        let Some(ring) = footprint::make(lots, lot, roads, zone, &mut rng) else {
            return false;
        };
        let id = self.active.len() as BuildingId;
        self.owner[l] = Some(id);
        self.lot.push(lot);
        self.zone.push(zone);
        self.height.push(height);
        self.roof.push(roof);
        self.active.push(true);
        if self.outline_start.is_empty() {
            self.outline_start.push(0);
        }
        self.outline.extend(ring);
        self.outline_start.push(self.outline.len() as u32);
        self.version = self.version.wrapping_add(1);
        true
    }

    /// `[zone, height, roof, n, x0, y0, …]` per active building in id order, not indexed by id.
    pub fn write_snapshot(&self, out: &mut Vec<f32>) {
        out.clear();
        for b in self.active_buildings() {
            let i = b as usize;
            let ring = self.footprint(b);
            out.extend([
                self.zone[i] as u8 as f32,
                self.height[i],
                self.roof[i] as f32,
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
            let ring = self.footprint(b as BuildingId);
            mix(ring.len() as u64);
            for p in ring {
                mix(((p.0.to_bits() as u64) << 32) | p.1.to_bits() as u64);
            }
        }
        mix(self.due.len() as u64);
        for due in &self.due {
            match due {
                Some((tick, zone)) => {
                    mix(1);
                    mix(*tick);
                    mix(*zone as u64);
                }
                None => mix(0),
            }
        }
    }
}
