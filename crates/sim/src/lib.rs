//! City simulation core.
//!
//! Engine-agnostic by design: no rendering, no wall-clock reads, no global state, no
//! nondeterministic iteration. `World::tick` turns state N into state N+1, and the same seed
//! plus the same command log always produces the same city.

pub mod agents;
pub mod block;
pub mod building;
mod check;
pub mod citizen;
pub mod clock;
pub mod frontage;
mod geometry;
pub mod guide;
pub mod junction;
pub mod lot;
pub mod path;
pub mod pollution;
mod poly;
pub mod power;
pub mod replay;
pub mod rng;
pub mod road;
pub mod snap;
pub mod traffic;
pub mod water;
pub mod world;
pub mod zone;

pub use replay::CommandLog;
pub use world::{
    Command, Config, PLANT_STRIDE, POWER_BUILDING_STRIDE, PowerStats, SaveError, Stats,
    WATER_BUILDING_STRIDE, WATER_PIECE_STRIDE, WaterStats, World,
};
pub use zone::Zone;
