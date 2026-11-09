//! Eras (ADR 0059): the city starts as a village and becomes a town once enough people live
//! there. The era decides what the player may build; buildings keep the era they were built in.

use serde::{Deserialize, Serialize};

use crate::road::RoadClass;
use crate::zone::Zone;

/// Residents, children included, at which a village becomes a town.
pub const TOWN_POPULATION: u32 = 1_500;

/// The city's era, in the order they arrive. The discriminant is the snapshot value.
#[derive(
    Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize,
)]
#[repr(u8)]
pub enum Era {
    /// Dirt roads, cottages and small shops.
    #[default]
    Village = 0,
    /// Paved streets, shopfronts and apartments.
    Town = 1,
}

impl Era {
    /// The era that unlocks painting `zone`: dense zones wait for the town.
    pub fn of_zone(zone: Zone) -> Era {
        if zone.is_dense() {
            Era::Town
        } else {
            Era::Village
        }
    }

    /// The era that unlocks building roads of `class`: main roads wait for the town.
    pub fn of_road(class: RoadClass) -> Era {
        match class {
            RoadClass::Main => Era::Town,
            RoadClass::Street | RoadClass::Alley => Era::Village,
        }
    }
}
