//! The bulldozer demolishes the building under the cursor (ADR 0045). Removal is immediate and
//! free, like a road edit clearing a lot; `World::apply` then reconciles closures and residents.

use super::World;
use crate::building::BuildingId;
use crate::poly;

impl World {
    /// The standing building whose footprint holds the point; overlaps keep the lowest ID.
    pub(super) fn building_at(&self, x: f32, y: f32) -> Option<BuildingId> {
        self.buildings
            .active_buildings()
            .find(|&b| poly::contains(self.buildings.footprint(b), (x, y)))
    }

    /// Removes the building under the point. Its lot keeps its zone and regrows.
    pub(super) fn demolish(&mut self, x: f32, y: f32) -> bool {
        let Some(b) = self.building_at(x, y) else {
            return false;
        };
        self.buildings.remove(
            b,
            &self.lots,
            &self.zones,
            self.totals.tick,
            self.cfg.dt,
            self.cfg.seed,
        )
    }
}
