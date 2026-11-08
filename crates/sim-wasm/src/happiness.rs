//! Happiness (ADR 0058): type conversion only, like the rest of this crate.

use wasm_bindgen::prelude::*;

use crate::Sim;

#[wasm_bindgen]
impl Sim {
    /// `[average, residents with complaint 0, 1, …]`: the city's mean mood (0 to 100) and how
    /// many residents have each complaint, by `sim::world::Complaint` code.
    pub fn happiness_stats(&self) -> Vec<u32> {
        let h = self.world.happiness_stats();
        std::iter::once(u32::from(h.average))
            .chain(h.complaints)
            .collect()
    }

    /// One byte per pollution cell, row-major like `pollution_cells`: 0 = nobody lives there,
    /// else 1 + mean mood × 254 / 100. Empty before the first update.
    pub fn mood_cells(&self) -> Vec<u8> {
        self.world.happiness.cells().to_vec()
    }

    /// Changes whenever `pollution_cells` or `mood_cells` do.
    pub fn ground_version(&self) -> u32 {
        self.world
            .pollution
            .version
            .wrapping_add(self.world.happiness.version)
    }
}
