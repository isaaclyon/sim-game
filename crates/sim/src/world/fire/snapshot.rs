//! What the frontend draws of fire (ADR 0053): stations, burning buildings, ruins, coverage
//! and previews. Read-only.

use crate::world::World;

/// Floats per station in `write_fire_stations`.
pub const STATION_STRIDE: usize = 5;
/// Floats per burning building in `write_fires`.
pub const FIRE_STRIDE: usize = 4;

fn closed_ring(ring: &[(f32, f32)], out: &mut Vec<f32>) {
    out.extend(ring.iter().chain(&ring[..1]).flat_map(|p| [p.0, p.1]));
}

impl World {
    /// Bumped whenever stations, fires, ruins or the fire setting change. Coverage also
    /// follows roads and buildings, which have versions of their own.
    pub fn fire_version(&self) -> u32 {
        self.fire.version
    }

    /// `[x, y, half_size, front_x, front_y]` per active station, in ID order.
    pub fn write_fire_stations(&self, out: &mut Vec<f32>) {
        out.clear();
        for s in self.fire.active_stations() {
            let at = self.fire.footprint(s);
            out.extend([at.centre.0, at.centre.1, at.half, at.front.0, at.front.1]);
        }
    }

    /// `[x, y, height, truck]` per burning building, in the order the fires started: the
    /// footprint's centre, its height in metres, and 1 if a truck is on the way, else 0.
    pub fn write_fires(&self, out: &mut Vec<f32>) {
        out.clear();
        let f = &self.fire;
        for (k, &b) in f.fire_building.iter().enumerate() {
            let ring = self.buildings.footprint(b);
            let n = ring.len() as f32;
            let (sx, sy) = ring.iter().fold((0.0, 0.0), |(x, y), p| (x + p.0, y + p.1));
            let truck = f32::from(u8::from(f.fire_truck[k].is_some()));
            out.extend([sx / n, sy / n, self.buildings.height[b as usize], truck]);
        }
    }

    /// `[n, x0, y0, …]` per lot holding ruins: its outline's point count, then its points.
    pub fn write_ruins(&self, out: &mut Vec<f32>) {
        out.clear();
        for &lot in &self.fire.ruin_lot {
            let ring = self.lots.outline(lot);
            out.push(ring.len() as f32);
            out.extend(ring.iter().flat_map(|p| [p.0, p.1]));
        }
    }

    /// Coverage for the fire view: `roads` gets `[ax, ay, bx, by]` per rendered segment of a
    /// road with an end in reach of a station; `buildings` gets `[x, y]` per covered building
    /// (the centre of its footprint).
    pub fn write_fire_cover(&self, roads: &mut Vec<f32>, buildings: &mut Vec<f32>) {
        roads.clear();
        buildings.clear();
        for r in self.roads.active_roads() {
            if self.road_covered(r) {
                for (_, a, _, b) in self.roads.shape(r).segments() {
                    roads.extend([a.0, a.1, b.0, b.1]);
                }
            }
        }
        for b in self.buildings.active_buildings() {
            if self.building_covered(b) {
                let ring = self.buildings.footprint(b);
                let n = ring.len() as f32;
                let (sx, sy) = ring.iter().fold((0.0, 0.0), |(x, y), p| (x + p.0, y + p.1));
                buildings.extend([sx / n, sy / n]);
            }
        }
    }

    /// `[ok, affordable, x0, y0, …]` (see `push_placement`), then the closed footprint where a
    /// station asked for at the cursor would stand; empty where no road is in reach.
    pub fn write_station_preview(&self, x: f32, y: f32, out: &mut Vec<f32>) {
        out.clear();
        let Some(at) = self.station_spot(x, y) else {
            return;
        };
        self.push_placement(self.station_fits(x, y), super::STATION_COST, out);
        closed_ring(&at.ring(), out);
    }

    /// Writes the bulldoze outline of the station under the point; true if there is one.
    pub(in crate::world) fn write_station_bulldoze(
        &self,
        x: f32,
        y: f32,
        out: &mut Vec<f32>,
    ) -> bool {
        let Some(s) = self.station_at(x, y) else {
            return false;
        };
        closed_ring(&self.fire.footprint(s).ring(), out);
        true
    }

    /// Writes the bulldoze outline of the ruins under the point; true if there are any.
    pub(in crate::world) fn write_ruin_bulldoze(&self, x: f32, y: f32, out: &mut Vec<f32>) -> bool {
        let Some(r) = self.ruin_at(x, y) else {
            return false;
        };
        closed_ring(self.lots.outline(self.fire.ruin_lot[r]), out);
        true
    }
}
