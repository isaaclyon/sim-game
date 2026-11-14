//! Schools (ADR 0055): type conversion only, like the rest of this crate.

use sim::Command;
use sim::world::SchoolKind;
use wasm_bindgen::prelude::*;

use crate::Sim;

#[wasm_bindgen]
impl Sim {
    /// Places a school beside the nearest road, facing it; `kind` is 0 = primary, 1 = high.
    /// False if refused or for an unknown kind.
    pub fn place_school(&mut self, kind: u8, x: f32, y: f32) -> bool {
        let Some(kind) = SchoolKind::from_u8(kind) else {
            return false;
        };
        self.apply(Command::PlaceSchool { kind, x, y })
    }

    /// `[ok, affordable, x0, y0, …]`; see `World::write_school_preview`. Empty for an unknown
    /// kind.
    pub fn school_preview(&mut self, kind: u8, x: f32, y: f32) -> Vec<f32> {
        self.buf.clear();
        if let Some(kind) = SchoolKind::from_u8(kind) {
            self.world.write_school_preview(kind, x, y, &mut self.buf);
        }
        self.buf.clone()
    }

    pub fn schools_version(&self) -> u32 {
        self.world.schools_version()
    }

    /// `[kind, x, y, half_size, front_x, front_y]` per school; see `World::write_schools`.
    pub fn schools(&mut self) -> Vec<f32> {
        self.world.write_schools(&mut self.buf);
        self.buf.clone()
    }
}
