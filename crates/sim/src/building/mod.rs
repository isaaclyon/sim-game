//! Buildings on lots (ADR 0016/0018). Growth is a timed visual placeholder for demand.
//! Lot geometry is read only at construction; rezoning never changes an existing building.
mod footprint;

use std::cmp::Reverse;
use std::collections::BinaryHeap;

use crate::geometry::Point;
use crate::lot::{LotId, Lots};
use crate::rng::Rng;
use crate::road::RoadNetwork;
use crate::zone::{Zone, ZoneGrid};

pub type BuildingId = u32;

#[derive(Default)]
pub struct Buildings {
    pub lot: Vec<LotId>,
    pub zone: Vec<Zone>,
    pub height: Vec<f32>,
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
                // Per-lot RNG keeps construction independent of traffic RNG and edit order.
                let mut rng = Rng::new(seed.wrapping_add(l as u64));
                let seconds = 2.0 + rng.below(580) as f32 / 10.0;
                let delay = (seconds / dt).ceil().max(1.0) as u64;
                self.due[l] = Some((tick.saturating_add(delay), zone));
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

    pub(crate) fn grow(&mut self, tick: u64, lots: &Lots, roads: &RoadNetwork, seed: u64) {
        let mut changed = false;
        while let Some(&Reverse((at, lot))) = self.pending.peek() {
            if at > tick {
                break;
            }
            self.pending.pop();
            let l = lot as usize;
            let Some((_, zone)) = self.due[l].take() else {
                continue;
            };
            let Some(ring) = footprint::make(lots, lot, roads, zone) else {
                continue;
            };
            let mut rng = Rng::new(seed.wrapping_add(lot as u64));
            let height = match zone {
                Zone::ResidentialLow => 3.0 * (1 + rng.below(2)) as f32,
                Zone::ResidentialHigh => 3.0 * (4 + rng.below(5)) as f32,
                Zone::CommercialLow => 4.0 * (1 + rng.below(2)) as f32,
                Zone::CommercialHigh => 4.0 * (3 + rng.below(5)) as f32,
                Zone::IndustrialLow => 5.0,
                Zone::IndustrialHigh => 9.0,
                Zone::None => unreachable!("unzoned lots are not scheduled"),
            };
            let id = self.active.len() as BuildingId;
            self.owner[l] = Some(id);
            self.lot.push(lot);
            self.zone.push(zone);
            self.height.push(height);
            self.active.push(true);
            if self.outline_start.is_empty() {
                self.outline_start.push(0);
            }
            self.outline.extend(ring);
            self.outline_start.push(self.outline.len() as u32);
            changed = true;
        }
        if changed {
            self.version = self.version.wrapping_add(1);
        }
    }

    /// `[zone, height, n, x0, y0, …]` per active building in id order, not indexed by id.
    pub fn write_snapshot(&self, out: &mut Vec<f32>) {
        out.clear();
        for b in self.active_buildings() {
            let i = b as usize;
            let ring = self.footprint(b);
            out.extend([self.zone[i] as u8 as f32, self.height[i], ring.len() as f32]);
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
