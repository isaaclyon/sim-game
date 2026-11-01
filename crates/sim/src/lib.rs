//! City simulation core.
//!
//! Engine-agnostic by design: no rendering, no wall-clock reads, no global state, no
//! nondeterministic iteration. `World::tick` turns state N into state N+1, and the same seed
//! plus the same command log always produces the same city.

pub mod agents;
pub mod junction;
pub mod path;
pub mod rng;
pub mod road;
pub mod snap;
pub mod traffic;
pub mod world;
pub mod zone;

pub use world::{Command, Config, Stats, World};
pub use zone::Zone;
