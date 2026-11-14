//! World settings fixed when a city starts.

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Config {
    pub seed: u64,
    /// Sim seconds per tick. The frontend's tick clock must match.
    pub dt: f32,
    /// Max pathfinding requests served per tick; the rest wait in a queue.
    pub path_budget: u32,
    pub dwell_min_s: f32,
    pub dwell_max_s: f32,
    /// A vehicle blocked this long at a lane end is despawned, the way gridlock is cleared in
    /// most city builders.
    pub stuck_limit_s: f32,
    pub grid_spacing: f32,
    /// Builds every due lot whatever the demand (ADR 0025) and powers and waters every
    /// building (ADR 0041, 0052). For tests and benchmarks that need full blocks; the game
    /// leaves it off.
    pub ignore_demand: bool,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            seed: 1,
            dt: 0.1,
            path_budget: 1000,
            dwell_min_s: 20.0,
            dwell_max_s: 240.0,
            stuck_limit_s: 60.0,
            grid_spacing: 120.0,
            ignore_demand: false,
        }
    }
}
