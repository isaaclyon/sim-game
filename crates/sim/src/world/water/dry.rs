//! Dry dense buildings slowly empty (ADR 0052). After a clock day's grace, one household moves
//! out of the city, or one worker loses their job, every `DRY_INTERVAL_S` while the building
//! stays dry. An empty building stays standing and fills again once it has water.

use super::super::World;
use crate::building::BuildingId;
use crate::clock::clock_seconds;
use crate::water::{DRY_GRACE_S, DRY_INTERVAL_S, STEP_S};

impl World {
    /// Departures due between the clock steps `from` and `to`, for every building still dry.
    pub(super) fn dry_departures(&mut self, from: u64, to: u64) {
        let (a, b) = (from * STEP_S, to * STEP_S);
        for bld in 0..self.water.dry_since.len() {
            let since = self.water.dry_since[bld];
            if since == u64::MAX {
                continue;
            }
            let start = clock_seconds(since, self.cfg.dt) + DRY_GRACE_S;
            // How many departures are due by clock second `t`.
            let due = |t: u64| {
                if t < start {
                    0
                } else {
                    (t - start) / DRY_INTERVAL_S + 1
                }
            };
            for _ in due(a)..due(b) {
                self.dry_departure(bld as BuildingId);
            }
        }
    }

    /// One household leaves the city, or else one worker loses their job here.
    fn dry_departure(&mut self, b: BuildingId) {
        if let Some(household) = self.next_household(b) {
            self.household_leaves_city(household);
            return;
        }
        let worker = (0..self.citizens.active.len())
            .find(|&c| self.citizens.active[c] && self.citizens.job[c] == Some(b));
        if let Some(c) = worker {
            self.dismiss(c as u32, b);
        }
    }
}
