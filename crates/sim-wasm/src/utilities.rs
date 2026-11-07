//! Power and water placement and snapshots (ADR 0041, 0052). Converts types only.

use crate::Sim;
use sim::Command;
use sim::power::PlantKind;
use sim::water::WaterKind;
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
impl Sim {
    /// `kind` is 0 = coal, 1 = solar, 2 = wind (ADR 0041). Unknown values build nothing.
    pub fn place_plant(&mut self, kind: u8, x: f32, y: f32) -> bool {
        let Some(kind) = PlantKind::from_u8(kind) else {
            return false;
        };
        self.apply(Command::PlacePlant { kind, x, y })
    }

    pub fn build_line(&mut self, ax: f32, ay: f32, bx: f32, by: f32) -> bool {
        self.apply(Command::BuildLine { ax, ay, bx, by })
    }

    /// `[ok, x0, y0, …]`; see `World::write_plant_preview`. Empty for an unknown kind.
    pub fn plant_preview(&mut self, kind: u8, x: f32, y: f32) -> Vec<f32> {
        self.buf.clear();
        if let Some(kind) = PlantKind::from_u8(kind) {
            self.world.write_plant_preview(kind, x, y, &mut self.buf);
        }
        self.buf.clone()
    }

    /// `[ax, ay, a_join, bx, by, b_join]`; see `World::write_line_preview`.
    pub fn line_preview(&mut self, ax: f32, ay: f32, bx: f32, by: f32) -> Vec<f32> {
        self.world.write_line_preview(ax, ay, bx, by, &mut self.buf);
        self.buf.clone()
    }

    pub fn power_version(&self) -> u32 {
        self.world.power_version()
    }

    /// `[kind, x, y, half_size, front_x, front_y, terminal_x, terminal_y, output, grid]` per
    /// plant; see `World::write_plant_snapshot`.
    pub fn plant_snapshot(&mut self) -> Vec<f32> {
        self.world.write_plant_snapshot(&mut self.buf);
        self.buf.clone()
    }

    /// `[ax, ay, bx, by]` per transmission line.
    pub fn line_snapshot(&mut self) -> Vec<f32> {
        self.world.write_line_snapshot(&mut self.buf);
        self.buf.clone()
    }

    /// `[x, y, height, grid, powered]` per active building; see `World::write_power_buildings`.
    pub fn power_buildings(&mut self) -> Vec<f32> {
        self.world.write_power_buildings(&mut self.buf);
        self.buf.clone()
    }

    /// `kind` is 0 = pump, 1 = tower (ADR 0052). Unknown values build nothing.
    pub fn place_water(&mut self, kind: u8, x: f32, y: f32) -> bool {
        let Some(kind) = WaterKind::from_u8(kind) else {
            return false;
        };
        self.apply(Command::PlaceWater { kind, x, y })
    }

    /// `[ok, x0, y0, …]`; see `World::write_water_preview`. Empty for an unknown kind.
    pub fn water_preview(&mut self, kind: u8, x: f32, y: f32) -> Vec<f32> {
        self.buf.clear();
        if let Some(kind) = WaterKind::from_u8(kind) {
            self.world.write_water_preview(kind, x, y, &mut self.buf);
        }
        self.buf.clone()
    }

    pub fn water_version(&self) -> u32 {
        self.world.water_version()
    }

    /// `[kind, x, y, half_size, front_x, front_y, output, level, network]` per pump or tower;
    /// see `World::write_water_pieces`.
    pub fn water_pieces(&mut self) -> Vec<f32> {
        self.world.write_water_pieces(&mut self.buf);
        self.buf.clone()
    }

    /// `[x, y, height, network, watered]` per standing dense building; see
    /// `World::write_water_buildings`.
    pub fn water_buildings(&mut self) -> Vec<f32> {
        self.world.write_water_buildings(&mut self.buf);
        self.buf.clone()
    }
}
