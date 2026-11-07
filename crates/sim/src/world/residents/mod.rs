//! Event-driven residents. No idle citizen scan on the traffic tick.
use serde::{Deserialize, Serialize};
mod allocation;
pub(super) mod family;
mod hash;
mod lifecycle;
mod schedule;
#[cfg(test)]
mod tests;

use crate::citizen::{CitizenId, HomeId, HouseholdId};
use crate::rng::Rng;
pub(in crate::world) use allocation::{capacity, ring_area};
use std::cmp::Reverse;
use std::collections::{BTreeSet, BinaryHeap};

#[derive(Serialize, Deserialize)]
pub(super) struct Residents {
    pub rng: Rng,
    pub registered: usize,
    pub jobs: Vec<u32>,
    pub workers: Vec<u32>,
    pub population: u32,
    /// Active residents who are children (ADR 0054).
    pub children: u32,
    /// Active residents with an assigned job; changes only where `Citizens::job` does.
    pub employed: u32,
    pub connectivity_version: Option<u32>,
    pub component: Vec<u32>,
    pub arrivals: BinaryHeap<Reverse<(u64, HomeId)>>,
    pub activities: BinaryHeap<Reverse<(u64, CitizenId)>>,
    pub searches: BinaryHeap<Reverse<(u64, CitizenId)>>,
    /// Derived index for relocation: exactly the vacant homes of active buildings that are not
    /// closing. Not hashed, since it follows from hashed state; `vacancies_hold` checks that.
    pub vacant: BTreeSet<HomeId>,
    /// Draws for births only, so families growing never shift other residents' draws.
    pub birth_rng: Rng,
    /// Next birth check of each active household.
    pub births: BinaryHeap<Reverse<(u64, HouseholdId)>>,
    /// When each child comes of age.
    pub grown: BinaryHeap<Reverse<(u64, CitizenId)>>,
    /// Grown children waiting for a home of their own, by `Citizens::move_by` deadline.
    pub movers: BTreeSet<(u64, CitizenId)>,
}
impl Residents {
    pub fn new(seed: u64) -> Self {
        Self {
            rng: Rng::new(seed ^ 0xc171_2e45),
            registered: 0,
            jobs: Vec::new(),
            workers: Vec::new(),
            population: 0,
            children: 0,
            employed: 0,
            connectivity_version: None,
            component: Vec::new(),
            arrivals: BinaryHeap::new(),
            activities: BinaryHeap::new(),
            searches: BinaryHeap::new(),
            vacant: BTreeSet::new(),
            birth_rng: Rng::new(seed ^ 0xb127_4d5e),
            births: BinaryHeap::new(),
            grown: BinaryHeap::new(),
            movers: BTreeSet::new(),
        }
    }
}
