//! Event-driven residents. No idle citizen scan on the traffic tick.
mod allocation;
mod hash;
mod lifecycle;
mod schedule;
#[cfg(test)]
mod tests;

use crate::citizen::{CitizenId, HomeId};
use crate::rng::Rng;
use std::cmp::Reverse;
use std::collections::BinaryHeap;

pub(super) struct Residents {
    pub rng: Rng,
    pub registered: usize,
    pub jobs: Vec<u32>,
    pub workers: Vec<u32>,
    pub population: u32,
    /// Active residents with an assigned job; changes only where `Citizens::job` does.
    pub employed: u32,
    pub connectivity_version: Option<u32>,
    pub component: Vec<u32>,
    pub arrivals: BinaryHeap<Reverse<(u64, HomeId)>>,
    pub activities: BinaryHeap<Reverse<(u64, CitizenId)>>,
    pub searches: BinaryHeap<Reverse<(u64, CitizenId)>>,
}
impl Residents {
    pub fn new(seed: u64) -> Self {
        Self {
            rng: Rng::new(seed ^ 0xc171_2e45),
            registered: 0,
            jobs: Vec::new(),
            workers: Vec::new(),
            population: 0,
            employed: 0,
            connectivity_version: None,
            component: Vec::new(),
            arrivals: BinaryHeap::new(),
            activities: BinaryHeap::new(),
            searches: BinaryHeap::new(),
        }
    }
}
