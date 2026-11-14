//! Feeds the pollution grid from industry, coal plants (ADR 0044) and traffic, and lets land
//! value hold back growth (ADR 0037).

use super::World;
use crate::building::BuildingId;
use crate::lot::LotId;
use crate::pollution::{GroundSource, PollutionSample, Sources, UPDATE_S};
use crate::power::PlantKind;
use crate::zone::Zone;

/// Traffic flow follows the cars on a road with this time constant, sim seconds.
const FLOW_TAU_S: f32 = 600.0;
/// Roads emit from a sample every this many metres along their length.
const ROAD_SAMPLE_M: f32 = 16.0;
/// Emission per car of traffic flow.
const CAR_AIR: f32 = 0.1;
const CAR_NOISE: f32 = 0.5;
/// Emission per 1,000 m² of industrial footprint; high density emits half as much again.
const FACTORY_AIR: f32 = 1.0;
const FACTORY_NOISE: f32 = 0.5;
const HIGH_DENSITY: f32 = 1.5;
/// Emission of one coal plant: more smoke than the biggest factory, less noise.
const COAL_AIR: f32 = 4.0;
const COAL_NOISE: f32 = 1.0;
/// Demand, in people, a lot with no land value needs before it builds; scaled down linearly
/// to 0 for clean land. Industry ignores land value.
const RESIDENTIAL_LAND_PEOPLE: i32 = 150;
const COMMERCIAL_LAND_PEOPLE: i32 = 30;

impl World {
    /// Air, ground, noise and land value at `(x, y)`, or `None` off the grid. Read-only.
    pub fn pollution_at(&self, x: f32, y: f32) -> Option<PollutionSample> {
        self.pollution.sample(x, y)
    }

    /// Called after each tick; updates on a fixed sim-time cadence only.
    pub(super) fn tick_pollution(&mut self) {
        let every = (UPDATE_S / self.cfg.dt).round().max(1.0) as u64;
        if !self.totals.tick.is_multiple_of(every) {
            return;
        }
        let alpha = (every as f32 * self.cfg.dt / FLOW_TAU_S).min(1.0);
        let roads = self.roads.lane_count() / 2;
        let p = &mut self.pollution;
        p.flow.resize(roads, 0.0);
        for (r, flow) in p.flow.iter_mut().enumerate() {
            if !self.roads.is_road_active(r as u32) {
                *flow = 0.0;
                continue;
            }
            let cars: usize = (2 * r..2 * r + 2)
                .map(|l| self.traffic.queues.get(l).map_or(0, |q| q.len()))
                .sum();
            *flow += (cars as f32 - *flow) * alpha;
        }
        let src = self.pollution_sources();
        self.pollution.update(&src);
    }

    fn pollution_sources(&self) -> Sources {
        let mut src = self.pollution.sources();
        self.road_sources(&mut src);
        for b in self.buildings.active_buildings() {
            self.factory_sources(b, &mut src);
        }
        for p in self.power.active_plants() {
            if self.power.plant_kind[p as usize] == PlantKind::Coal {
                let at = self.power.footprint(p).bounds();
                self.area_sources(at, COAL_AIR, COAL_NOISE, &mut src);
            }
        }
        self.fire_boost(&mut src);
        self.school_boost(&mut src);
        src
    }

    fn road_sources(&self, src: &mut Sources) {
        let p = &self.pollution;
        for (r, &flow) in p.flow.iter().enumerate() {
            if flow < 0.01 {
                continue;
            }
            let lane = 2 * r as u32;
            let len = self.roads.lane_len[lane as usize];
            let samples = (len / ROAD_SAMPLE_M).ceil().max(1.0);
            let cars = flow / samples;
            for k in 0..samples as u32 {
                let d = (k as f32 + 0.5) * len / samples;
                let (x, y) = self.roads.lane_point(lane, d, 0.0);
                if let Some(c) = p.cell_at(x, y) {
                    src.air[c] += cars * CAR_AIR;
                    src.noise[c] += cars * CAR_NOISE;
                }
            }
        }
    }

    /// An industrial building's emission, by its footprint's area.
    fn factory_sources(&self, b: BuildingId, src: &mut Sources) {
        let scale = match self.buildings.zone[b as usize] {
            Zone::IndustrialLow => 1.0,
            Zone::IndustrialHigh => HIGH_DENSITY,
            _ => return,
        };
        let ring = self.buildings.footprint(b);
        let mut area = 0.0;
        let mut bounds = [f32::MAX, f32::MAX, f32::MIN, f32::MIN];
        for (k, p) in ring.iter().enumerate() {
            let q = ring[(k + 1) % ring.len()];
            area += p.0 * q.1 - q.0 * p.1;
            bounds = [
                bounds[0].min(p.0),
                bounds[1].min(p.1),
                bounds[2].max(p.0),
                bounds[3].max(p.1),
            ];
        }
        let emission = (area / 2.0).abs() / 1000.0 * scale;
        self.area_sources(
            bounds,
            emission * FACTORY_AIR,
            emission * FACTORY_NOISE,
            src,
        );
    }

    /// Air and noise spread over the cells under `bounds` (`[min_x, min_y, max_x, max_y]`);
    /// ground pollution under them and in the ring of cells around.
    fn area_sources(&self, bounds: [f32; 4], air: f32, noise: f32, src: &mut Sources) {
        let p = &self.pollution;
        let (Some(lo), Some(hi)) = (
            p.cell_at(bounds[0], bounds[1]),
            p.cell_at(bounds[2], bounds[3]),
        ) else {
            return;
        };
        let cols = p.cols as usize;
        let (c0, r0, c1, r1) = (lo % cols, lo / cols, hi % cols, hi / cols);
        let cells = ((c1 - c0 + 1) * (r1 - r0 + 1)) as f32;
        for r in r0.saturating_sub(1)..=(r1 + 1).min(p.rows as usize - 1) {
            for c in c0.saturating_sub(1)..=(c1 + 1).min(cols - 1) {
                let i = r * cols + c;
                let under = (r0..=r1).contains(&r) && (c0..=c1).contains(&c);
                let ground = if under {
                    src.air[i] += air / cells;
                    src.noise[i] += noise / cells;
                    GroundSource::Under
                } else {
                    GroundSource::Beside
                };
                src.ground[i] = src.ground[i].max(ground);
            }
        }
    }

    /// Demand a due lot's use needs before it builds: 0 on clean land, more as land value
    /// falls (ADR 0037). Mean land value over the lot's zone cells.
    pub(super) fn land_threshold(&self, lot: LotId, zone: Zone) -> i32 {
        let people = match zone {
            Zone::ResidentialLow | Zone::ResidentialHigh => RESIDENTIAL_LAND_PEOPLE,
            Zone::CommercialLow | Zone::CommercialHigh => COMMERCIAL_LAND_PEOPLE,
            _ => return 0,
        };
        let p = &self.pollution;
        let cols = self.zones.cols;
        let land = p.land_of(
            self.lots
                .cells(lot)
                .iter()
                .map(|&c| p.cell_of_zone_cell(c, cols)),
        );
        (255 - i32::from(land)) * people / 255
    }
}
