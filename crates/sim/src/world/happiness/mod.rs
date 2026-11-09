//! City happiness (ADR 0058): recomputed on the pollution cadence from every resident's mood.
//! It steers residential demand, sends households that stay miserable out of the city, and
//! paints a mood byte per pollution cell for the overlay.

mod mood;
#[cfg(test)]
mod tests;

pub use mood::{BASE_MOOD, COMPLAINTS, Complaint, Complaints};

use super::World;
use crate::check::{Check, ensure};
use crate::citizen::CitizenId;
use crate::clock::CLOCK_SECONDS_PER_SIM_SECOND;
use crate::pollution::UPDATE_S;
use crate::rng::Rng;
use serde::{Deserialize, Serialize};

/// Mood that neither helps nor hurts demand; the average of a city with nobody in it.
pub const NEUTRAL_MOOD: u8 = 50;
/// A household whose mean mood stays below this for `LEAVE_AFTER_DAYS` may leave the city.
pub const LEAVE_BELOW: u32 = 30;
const LEAVE_AFTER_DAYS: f32 = 1.0;
/// One chance in this many per update that such a household leaves.
const LEAVE_ODDS: u32 = 250;
/// Residential demand, in people, a fully happy or miserable city gains or loses: a base plus
/// a share of population in percent.
const DEMAND_BASE: i32 = 20;
const DEMAND_SHARE: i32 = 10;
const HAPPINESS_SALT: u64 = 0x4a77_1e55_0d0e_5ad5;

#[derive(Serialize, Deserialize)]
pub struct Happiness {
    /// Draws for move-outs only, so moods never shift other draws.
    rng: Rng,
    /// People-weighted mean mood at the last update; `NEUTRAL_MOOD` with nobody in town.
    pub average: u8,
    /// Residents with each complaint at the last update, by `Complaint` code.
    pub complaints: [u32; COMPLAINTS],
    /// Per household ID: the tick its mood fell below `LEAVE_BELOW`, `u64::MAX` while above.
    low_since: Vec<u64>,
    /// Per pollution cell: 0 = nobody lives there, else 1 + mean mood × 254 / 100. Display only.
    cells: Vec<u8>,
    /// Bumped when `cells` change, so the frontend knows when to redraw.
    #[serde(skip)]
    pub version: u32,
}

/// Happiness figures for the top bar.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct HappinessStats {
    pub average: u8,
    pub complaints: [u32; COMPLAINTS],
}

impl Default for HappinessStats {
    fn default() -> Self {
        Self {
            average: NEUTRAL_MOOD,
            complaints: [0; COMPLAINTS],
        }
    }
}

impl Happiness {
    pub(super) fn new(seed: u64) -> Self {
        Self {
            rng: Rng::new(seed ^ HAPPINESS_SALT),
            average: NEUTRAL_MOOD,
            complaints: [0; COMPLAINTS],
            low_since: Vec::new(),
            cells: Vec::new(),
            version: 0,
        }
    }

    /// Mood bytes per pollution cell (see `cells`); empty before the first update.
    pub fn cells(&self) -> &[u8] {
        &self.cells
    }

    pub(super) fn hash_into(&self, mix: &mut impl FnMut(u64)) {
        mix(self.rng.state());
        mix(u64::from(self.average));
        for &n in &self.complaints {
            mix(u64::from(n));
        }
        mix(self.low_since.len() as u64);
        for &t in &self.low_since {
            mix(t);
        }
    }

    pub(super) fn check(&self, households: usize, cells: usize) -> Check {
        ensure(
            usize::from(self.average) <= 100
                && self.low_since.len() <= households
                && (self.cells.is_empty() || self.cells.len() == cells),
            "happiness",
        )
    }
}

impl World {
    pub fn happiness_stats(&self) -> HappinessStats {
        HappinessStats {
            average: self.happiness.average,
            complaints: self.happiness.complaints,
        }
    }

    /// Residential demand the city's mood adds: positive when happy, negative when not.
    pub(super) fn happiness_demand(&self) -> i32 {
        let people = DEMAND_BASE + self.residents.population as i32 * DEMAND_SHARE / 100;
        (i32::from(self.happiness.average) - i32::from(NEUTRAL_MOOD)) * people
            / i32::from(NEUTRAL_MOOD)
    }

    /// Remembers how long citizen `c`'s trip to work took, on arrival there.
    pub(super) fn note_commute(&mut self, c: CitizenId) {
        let i = c as usize;
        let start = self.citizens.trip_start[i];
        if start == u64::MAX {
            return;
        }
        let ticks = self.totals.tick.saturating_sub(start) as f32;
        let minutes = ticks * self.cfg.dt * CLOCK_SECONDS_PER_SIM_SECOND as f32 / 60.0;
        self.citizens.commute[i] = minutes.round().min(f32::from(u16::MAX)) as u16;
    }

    /// Called after pollution each tick; updates on the same cadence.
    pub(super) fn tick_happiness(&mut self) {
        let every = (UPDATE_S / self.cfg.dt).round().max(1.0) as u64;
        if self.totals.tick.is_multiple_of(every) {
            self.update_happiness();
        }
    }

    /// Every resident's mood into the average, complaint counts and cell bytes, then move-outs.
    pub(super) fn update_happiness(&mut self) {
        let mut homes = vec![None; self.buildings.active.len()];
        let cell_count = self.pollution.len();
        let (mut sum, mut count) = (vec![0u32; cell_count], vec![0u32; cell_count]);
        let mut complaints = [0u32; COMPLAINTS];
        let (mut total, mut people) = (0u64, 0u64);
        // Per active household with members: its mean mood.
        let mut means = Vec::new();
        for hh in 0..self.households.active.len() {
            let members = &self.households.members[hh];
            if !self.households.active[hh] || members.is_empty() {
                continue;
            }
            let b = self.homes.building[self.households.home[hh] as usize];
            let home = *homes[b as usize].get_or_insert_with(|| self.home_mood(b));
            let mut moods = 0u32;
            for &c in members {
                let (m, why) = self.person_mood(c, home);
                moods += u32::from(m);
                for (k, n) in complaints.iter_mut().enumerate() {
                    *n += u32::from(why.0 >> k & 1);
                }
            }
            let n = members.len() as u32;
            total += u64::from(moods);
            people += u64::from(n);
            if let Some(cell) = home.cell {
                sum[cell] += moods;
                count[cell] += n;
            }
            means.push((hh, moods / n));
        }
        let cells: Vec<u8> = sum
            .iter()
            .zip(&count)
            .map(|(&s, &n)| {
                s.checked_div(n)
                    .map_or(0, |mean| (1 + mean * 254 / 100) as u8)
            })
            .collect();
        let grace = self.days_to_ticks(LEAVE_AFTER_DAYS);
        let now = self.totals.tick;
        let h = &mut self.happiness;
        if cells != h.cells {
            h.cells = cells;
            h.version = h.version.wrapping_add(1);
        }
        h.average = total.checked_div(people).map_or(NEUTRAL_MOOD, |m| m as u8);
        h.complaints = complaints;
        h.low_since.resize(self.households.active.len(), u64::MAX);
        let mut low = vec![false; h.low_since.len()];
        let mut leaving = Vec::new();
        for (hh, mean) in means {
            if mean >= LEAVE_BELOW {
                continue;
            }
            low[hh] = true;
            let since = &mut h.low_since[hh];
            if *since == u64::MAX {
                *since = now;
            } else if now - *since >= grace && h.rng.below(LEAVE_ODDS) == 0 {
                leaving.push(hh as u32);
            }
        }
        for (since, low) in h.low_since.iter_mut().zip(low) {
            if !low {
                *since = u64::MAX;
            }
        }
        for hh in leaving {
            self.happiness.low_since[hh as usize] = u64::MAX;
            self.household_leaves_city(hh);
        }
    }
}
