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

    /// `[none, primary, high school, university, unschooled]`: adults by how far they got
    /// in school, then children without a school seat.
    pub fn education_stats(&self) -> Vec<u32> {
        let e = self.world.education_stats();
        e.adults.into_iter().chain([e.unschooled]).collect()
    }

    /// The schools view, one array: `[schools_len, roads_len, schools…, roads…, homes…]`,
    /// lengths in floats; see `World::write_school_view`.
    pub fn school_view(&mut self) -> Vec<f32> {
        let (mut schools, mut roads, mut homes) = (Vec::new(), Vec::new(), Vec::new());
        self.world
            .write_school_view(&mut schools, &mut roads, &mut homes);
        let mut out = vec![schools.len() as f32, roads.len() as f32];
        out.extend(schools);
        out.extend(roads);
        out.extend(homes);
        out
    }

    /// Seats taken across all schools; the schools view is resent when it changes.
    pub fn school_pupils(&self) -> u32 {
        self.world.schools.pupils.iter().sum()
    }
}
