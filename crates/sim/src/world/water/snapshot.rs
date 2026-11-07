//! What the frontend draws for water, and the pump and tower preview. Read-only.

use super::super::World;
use crate::water::WaterKind;

/// Floats per piece in `write_water_pieces`.
pub const WATER_PIECE_STRIDE: usize = 9;
/// Floats per building in `write_water_buildings`.
pub const WATER_BUILDING_STRIDE: usize = 5;

impl World {
    /// Changes whenever pieces, networks, tower levels or who has water may have changed.
    pub fn water_version(&self) -> u32 {
        self.water_cache.version
    }

    /// `[kind, x, y, half_size, front_x, front_y, output, level, network]` per active pump or
    /// tower in ID order, not indexed by ID. Kind is 0 = pump, 1 = tower; the front is the unit
    /// direction it faces, toward its road; output is what a pump gives now (0 for towers);
    /// level is what a tower holds (0 for pumps).
    pub fn write_water_pieces(&self, out: &mut Vec<f32>) {
        out.clear();
        let c = &self.water_cache;
        for p in self.water.active_pieces() {
            let i = p as usize;
            let at = self.water.footprint(p);
            out.extend([
                self.water.kind[i] as u8 as f32,
                at.centre.0,
                at.centre.1,
                at.half,
                at.front.0,
                at.front.1,
                c.output.get(i).copied().unwrap_or(0) as f32,
                self.water.level[i] as f32,
                network(c.nets.piece.get(i).copied()),
            ]);
        }
    }

    /// `[x, y, height, network, watered]` per standing dense building in ID order, the
    /// buildings that use water: its footprint's mean point, its height, its network (-1 when
    /// its road is gone) and 1 or 0.
    pub fn write_water_buildings(&self, out: &mut Vec<f32>) {
        out.clear();
        let c = &self.water_cache;
        for b in self.buildings.active_buildings() {
            let i = b as usize;
            if c.demand.get(i).is_none_or(|&d| d == 0) {
                continue;
            }
            let (x, y) = self.building_centre(b);
            out.extend([
                x,
                y,
                self.buildings.height[i],
                network(c.net.get(i).copied()),
                f32::from(u8::from(self.building_watered(b))),
            ]);
        }
    }

    /// `[ok, x0, y0, …]`: whether a pump or tower asked for here fits, then the footprint it
    /// would take (`water_spot`, beside a road) as a closed ring.
    pub fn write_water_preview(&self, kind: WaterKind, x: f32, y: f32, out: &mut Vec<f32>) {
        out.clear();
        let Some(at) = self.water_spot(kind, x, y) else {
            return;
        };
        out.push(f32::from(u8::from(self.water_fits(kind, x, y))));
        let ring = at.ring();
        for p in ring.iter().chain(&ring[..1]) {
            out.extend([p.0, p.1]);
        }
    }

    /// Like `write_bulldoze_preview` for the pump or tower under the point: its closed
    /// outline. False when there is none.
    pub(in crate::world) fn write_water_bulldoze(
        &self,
        x: f32,
        y: f32,
        out: &mut Vec<f32>,
    ) -> bool {
        if !(x.is_finite() && y.is_finite()) {
            return false;
        }
        let Some(p) = self.water.piece_at(x, y, 0.0) else {
            return false;
        };
        let ring = self.water.footprint(p).ring();
        for q in ring.iter().chain(&ring[..1]) {
            out.extend([q.0, q.1]);
        }
        true
    }
}

fn network(id: Option<u32>) -> f32 {
    match id {
        Some(id) if id != u32::MAX => id as f32,
        _ => -1.0,
    }
}
