//! Data overlays (ADR 0057): type conversion only, like the rest of this crate.

use wasm_bindgen::prelude::*;

use crate::Sim;

#[wasm_bindgen]
impl Sim {
    /// `[ax, ay, bx, by, class, road, jam]` per rendered segment of each road with a car on
    /// it; see `World::write_road_traffic`.
    pub fn road_traffic(&mut self) -> Vec<f32> {
        self.world.write_road_traffic(&mut self.buf);
        self.buf.clone()
    }

    /// `[gaps, n, x0, y0, …]` per standing building's lot; see `World::write_service_gaps`.
    pub fn service_gaps(&mut self) -> Vec<f32> {
        self.world.write_service_gaps(&mut self.buf);
        self.buf.clone()
    }

    /// Whether missing fire cover counts in `service_gaps`; it turns with population, which
    /// no version tracks.
    pub fn fire_gaps_shown(&self) -> bool {
        self.world.fire_gaps_shown()
    }
}
