//! What the frontend draws for power, and the previews of power tools. Read-only.

use super::super::World;
use super::End;
use crate::power::{Footprint, PlantKind};

/// Floats per plant in `write_plant_snapshot`.
pub const PLANT_STRIDE: usize = 10;
/// Floats per building in `write_power_buildings`.
pub const POWER_BUILDING_STRIDE: usize = 5;

impl World {
    /// Changes whenever plants, lines, grids or the allocation may have changed.
    pub fn power_version(&self) -> u32 {
        self.power_cache.version
    }

    /// `[kind, x, y, half_size, front_x, front_y, terminal_x, terminal_y, output, grid]` per
    /// active plant in ID order, not indexed by ID. Kind is 0 = coal, 1 = solar, 2 = wind; the
    /// front is the unit direction the plant faces (toward its road for coal); the terminal is
    /// where lines join it; output is what it produces now.
    pub fn write_plant_snapshot(&self, out: &mut Vec<f32>) {
        out.clear();
        let c = &self.power_cache;
        for p in self.power.active_plants() {
            let i = p as usize;
            let at = self.power.footprint(p);
            let terminal = self.power.terminal(p);
            out.extend([
                self.power.plant_kind[i] as u8 as f32,
                at.centre.0,
                at.centre.1,
                at.half,
                at.front.0,
                at.front.1,
                terminal.0,
                terminal.1,
                c.output.get(i).copied().unwrap_or(0) as f32,
                c.grids.piece.get(i).copied().unwrap_or(u32::MAX) as f32,
            ]);
        }
    }

    /// `[ax, ay, bx, by]` per active transmission line in ID order.
    pub fn write_line_snapshot(&self, out: &mut Vec<f32>) {
        out.clear();
        for l in self.power.active_lines() {
            let (a, b) = self.power.line(l);
            out.extend([a.0, a.1, b.0, b.1]);
        }
    }

    /// `[x, y, height, grid, powered]` per active building in ID order: its footprint's mean
    /// point, its height, its grid (-1 when its road is gone) and 1 or 0.
    pub fn write_power_buildings(&self, out: &mut Vec<f32>) {
        out.clear();
        let c = &self.power_cache;
        for b in self.buildings.active_buildings() {
            let i = b as usize;
            let (x, y) = self.building_centre(b);
            let grid = c.grid.get(i).copied().unwrap_or(u32::MAX);
            out.extend([
                x,
                y,
                self.buildings.height[i],
                if grid == u32::MAX { -1.0 } else { grid as f32 },
                f32::from(u8::from(self.building_powered(b))),
            ]);
        }
    }

    /// The mean point of a building's footprint, where markers stand.
    pub(in crate::world) fn building_centre(&self, b: crate::building::BuildingId) -> (f32, f32) {
        let ring = self.buildings.footprint(b);
        let n = ring.len().max(1) as f32;
        let (sx, sy) = ring.iter().fold((0.0, 0.0), |(x, y), p| (x + p.0, y + p.1));
        (sx / n, sy / n)
    }

    /// `[ok, x0, y0, …]`: whether a plant of `kind` asked for here fits, then the footprint
    /// it would take (`plant_spot`, snapped beside a road for coal) as a closed ring.
    pub fn write_plant_preview(&self, kind: PlantKind, x: f32, y: f32, out: &mut Vec<f32>) {
        out.clear();
        let Some(at) = self.plant_spot(kind, x, y) else {
            return;
        };
        out.push(f32::from(u8::from(self.plant_fits(kind, x, y))));
        push_ring(at, out);
    }

    /// `[ax, ay, a_join, bx, by, b_join]` for the line `BuildLine` would build, join 1 where an
    /// end meets a line, plant or road and 0 where it stands free; empty if refused.
    pub fn write_line_preview(&self, ax: f32, ay: f32, bx: f32, by: f32, out: &mut Vec<f32>) {
        out.clear();
        if let Some(ends) = self.plan_line(ax, ay, bx, by) {
            for (p, end) in ends {
                out.extend([p.0, p.1, f32::from(u8::from(end == End::Joined))]);
            }
        }
    }

    /// Like `write_bulldoze_preview` for a plant (its closed outline) or line (its two ends)
    /// under the point. False when there is none, so a road may be the target.
    pub(in crate::world) fn write_power_bulldoze(
        &self,
        x: f32,
        y: f32,
        out: &mut Vec<f32>,
    ) -> bool {
        match self.power_target(x, y) {
            Some(Ok(p)) => {
                push_ring(self.power.footprint(p), out);
            }
            Some(Err(l)) => {
                let (a, b) = self.power.line(l);
                out.extend([a.0, a.1, b.0, b.1]);
            }
            None => return false,
        }
        true
    }
}

/// The footprint's corners as a closed `[x, y]` ring.
fn push_ring(at: Footprint, out: &mut Vec<f32>) {
    let ring = at.ring();
    for p in ring.iter().chain(&ring[..1]) {
        out.extend([p.0, p.1]);
    }
}
