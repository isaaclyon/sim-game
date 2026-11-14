//! Data overlays (ADR 0057): read-only answers for the traffic and services views. Nothing here
//! changes the city or enters the hash.

use super::World;
use super::fire::FIRE_THRESHOLD;
use crate::traffic::SPACING;

#[cfg(test)]
mod tests;

/// Floats per rendered segment in `write_road_traffic`.
pub const TRAFFIC_STRIDE: usize = 7;
/// `write_service_gaps` bits: the building has no power.
pub const GAP_POWER: u8 = 1;
/// The building needs water (dense) and has none.
pub const GAP_WATER: u8 = 2;
/// No fire station reaches the building, while fires can start (`fire_gaps_shown`).
pub const GAP_FIRE: u8 = 4;

impl World {
    /// How backed up `road` is, 0 to 1, or None when no car is on it: per direction, the
    /// queue length its cars' lost speed adds up to (a stopped car counts its whole spacing, a
    /// car at the limit nothing) over the lane's length; the worse direction wins.
    fn road_jam(&self, road: u32) -> Option<f32> {
        let mut cars = false;
        let mut jam = 0.0f32;
        for lane in [2 * road as usize, 2 * road as usize + 1] {
            let Some(queue) = self.traffic.queues.get(lane) else {
                continue;
            };
            let limit = self.roads.lane_speed[lane];
            let lost: f32 = queue
                .iter()
                .map(|&a| 1.0 - (self.agents.speed[a as usize] / limit).clamp(0.0, 1.0))
                .sum();
            cars |= !queue.is_empty();
            jam = jam.max(lost * SPACING / self.roads.lane_len[lane].max(SPACING));
        }
        cars.then_some(jam.min(1.0))
    }

    /// `[ax, ay, bx, by, class, road, jam]` per rendered segment of each active road with a car
    /// on it, in road ID order (`TRAFFIC_STRIDE`). `road` is the road's ID, stable while it
    /// stands; `jam` is 0 (flowing at the limit) to 1 (lanes full of stopped cars).
    pub fn write_road_traffic(&self, out: &mut Vec<f32>) {
        out.clear();
        for road in self.roads.active_roads() {
            let Some(jam) = self.road_jam(road) else {
                continue;
            };
            let class = self.roads.road_class(road) as u8 as f32;
            for (_, a, _, b) in self.roads.shape(road).segments() {
                out.extend([a.0, a.1, b.0, b.1, class, road as f32, jam]);
            }
        }
    }

    /// Whether missing fire cover counts as a gap: fires are on and the city is big enough
    /// for them to start (ADR 0053).
    pub fn fire_gaps_shown(&self) -> bool {
        self.fire.enabled && self.residents.population >= FIRE_THRESHOLD
    }

    /// `[gaps, n, x0, y0, …]` per standing building in ID order: its `GAP_*` bits, then its
    /// lot's outline point count and points (counter-clockwise, not closed).
    pub fn write_service_gaps(&self, out: &mut Vec<f32>) {
        out.clear();
        let fire = self.fire_gaps_shown();
        for b in self.buildings.active_buildings() {
            if self.buildings.lot[b as usize] == crate::building::NO_LOT {
                continue;
            }
            let mut gaps = 0;
            if !self.building_powered(b) {
                gaps |= GAP_POWER;
            }
            if !self.building_watered(b) {
                gaps |= GAP_WATER;
            }
            if fire && !self.building_covered(b) {
                gaps |= GAP_FIRE;
            }
            let ring = self.lots.outline(self.buildings.lot[b as usize]);
            out.extend([f32::from(gaps), ring.len() as f32]);
            out.extend(ring.iter().flat_map(|p| [p.0, p.1]));
        }
    }
}
