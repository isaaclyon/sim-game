//! School runs (ADR 0055, slice 3): an enrolled child walks to school in the morning, stays
//! for the school day and walks home. The trips themselves are ordinary citizen trips
//! (`residents::schedule`); this module only says where and when.

use crate::building::BuildingId;
use crate::citizen::CitizenId;
use crate::clock::CLOCK_SECONDS_PER_SIM_SECOND;
use crate::world::World;

/// Children set out between these clock seconds: 7:30 and 8:00.
const DEPART_FROM: u32 = 7 * 3600 + 1800;
const DEPART_SPREAD: u32 = 1800;
/// How long a child stays once they arrive, sim seconds: six and a half clock hours.
pub(in crate::world) const SCHOOL_DAY_S: f32 =
    (6 * 3600 + 1800) as f32 / CLOCK_SECONDS_PER_SIM_SECOND as f32;

impl World {
    /// The building of the school child `c` holds a seat at.
    pub(in crate::world) fn school_building(&self, c: CitizenId) -> Option<BuildingId> {
        let s = self.schools.school_of(c)?;
        Some(self.schools.building[s as usize])
    }

    /// Clock second child `c` leaves for school: stable per child, spread over half an hour.
    /// Derived from the departure every citizen is given at birth (7:00 to 9:00), so it needs
    /// no draw and no saved field.
    pub(in crate::world) fn school_departure(&self, c: CitizenId) -> u32 {
        let own = self.citizens.departure[c as usize].saturating_sub(7 * 3600);
        DEPART_FROM + own % DEPART_SPREAD
    }
}
