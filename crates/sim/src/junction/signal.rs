//! Actuated signal timing and the signal snapshot (ADR 0009).

use super::{Junctions, Turn, next_lane, reverse, ticks};
use crate::agents::Agents;
use crate::road::{LaneId, NodeId, RoadNetwork};
use crate::traffic::Traffic;

const MIN_GREEN_S: f32 = 8.0;
const MAX_GREEN_S: f32 = 40.0;
/// Green is extended while a car will reach the stop line within this, seconds.
const GAP_OUT_S: f32 = 3.0;
/// A car this close to the stop line, seconds, calls for its phase.
const DEMAND_S: f32 = 15.0;
const PROTECTED_LEFT_S: f32 = 6.0;
/// A left-turner waiting this long on green earns a protected-left phase, seconds.
const LEFT_STARVED_S: f32 = 20.0;

impl Junctions {
    /// Advances every signal one tick: actuated green between a min and a max, empty phases
    /// skipped, and a protected left inserted when left-turners have waited too long.
    pub fn update_signals(
        &mut self,
        net: &RoadNetwork,
        traffic: &Traffic,
        agents: &Agents,
        dt: f32,
    ) {
        for i in 0..self.signals.len() {
            let node = self.signals[i];
            let n = node as usize;
            self.elapsed[n] += 1;
            let elapsed = self.elapsed[n];
            let g = self.phase[n];
            let groups = self.groups[n];
            if self.protected[n] {
                if elapsed >= ticks(PROTECTED_LEFT_S, dt) {
                    let next = (1..groups)
                        .map(|k| (g + k) % groups)
                        .find(|&h| self.demand(net, traffic, agents, node, h, DEMAND_S))
                        .unwrap_or((g + 1) % groups);
                    self.switch(n, next, false);
                }
                continue;
            }
            if elapsed < ticks(MIN_GREEN_S, dt) {
                continue;
            }
            let Some(next) = (1..groups)
                .map(|k| (g + k) % groups)
                .find(|&h| self.demand(net, traffic, agents, node, h, DEMAND_S))
            else {
                continue; // Nobody else is waiting: rest in green.
            };
            let extend = self.demand(net, traffic, agents, node, g, GAP_OUT_S);
            if extend && elapsed < ticks(MAX_GREEN_S, dt) {
                continue;
            }
            if self.left_starved(net, traffic, agents, node, g, dt) {
                self.switch(n, g, true);
            } else {
                self.switch(n, next, false);
            }
        }
    }

    fn switch(&mut self, n: usize, phase: u8, protected: bool) {
        self.phase[n] = phase;
        self.protected[n] = protected;
        self.elapsed[n] = 0;
        if !protected {
            self.walk_call[n] &= !group_bit(phase);
        }
    }

    /// Whether a walker on the sidewalk of `lane` may cross at the node it leads to. Walkers
    /// cross with parallel traffic: at a signal only while `lane`'s own group has a plain
    /// green. Other junctions never hold walkers, and cars do not yield to them (ADR 0026).
    pub fn walk_allowed(&self, net: &RoadNetwork, lane: LaneId) -> bool {
        let n = net.lane_to[lane as usize] as usize;
        self.control[n] != super::Control::Signal
            || (self.group[lane as usize] == self.phase[n] && !self.protected[n])
    }

    /// A walker waits to cross from `lane`: its group calls for a green like a waiting car.
    pub fn call_walk(&mut self, net: &RoadNetwork, lane: LaneId) {
        let n = net.lane_to[lane as usize] as usize;
        self.walk_call[n] |= group_bit(self.group[lane as usize]);
    }

    /// Arriving lanes of signal group `group` at `node`.
    fn group_lanes<'a>(
        &'a self,
        net: &'a RoadNetwork,
        node: NodeId,
        group: u8,
    ) -> impl Iterator<Item = LaneId> + 'a {
        net.out_lanes(node)
            .iter()
            .map(|&l| reverse(l))
            .filter(move |&l| self.group[l as usize] == group)
    }

    /// True if a car on `group` will reach its stop line within `horizon` seconds and has
    /// somewhere to go. Green for a car whose exit is full would be wasted.
    fn demand(
        &self,
        net: &RoadNetwork,
        traffic: &Traffic,
        agents: &Agents,
        node: NodeId,
        group: u8,
        horizon: f32,
    ) -> bool {
        if self.walk_call[node as usize] & group_bit(group) != 0 {
            return true;
        }
        self.group_lanes(net, node, group).any(|lane| {
            traffic.waiting(lane, agents).is_some_and(|b| {
                (self.stop[lane as usize] - agents.pos[b as usize])
                    / agents.speed[b as usize].max(0.5)
                    < horizon
                    && next_lane(agents, b)
                        .is_none_or(|nx| self.exit_clear(net, traffic, agents, nx))
            })
        })
    }

    fn left_starved(
        &self,
        net: &RoadNetwork,
        traffic: &Traffic,
        agents: &Agents,
        node: NodeId,
        group: u8,
        dt: f32,
    ) -> bool {
        let limit = ticks(LEFT_STARVED_S, dt);
        self.group_lanes(net, node, group).any(|lane| {
            traffic.waiting(lane, agents).is_some_and(|b| {
                agents.stuck_ticks[b as usize] as u32 >= limit
                    && next_lane(agents, b).is_some_and(|nx| self.turn(lane, nx) == Turn::Left)
            })
        })
    }

    /// Writes `[x, y, heading, state]` per signalised approach; state 0 = red, 1 = green,
    /// 2 = protected-left green.
    pub fn write_signal_snapshot(&self, net: &RoadNetwork, side: f32, out: &mut Vec<f32>) {
        out.clear();
        for &node in &self.signals {
            let n = node as usize;
            for &l in net.out_lanes(node) {
                let lane = reverse(l);
                let (x, y) = net.lane_point(lane, self.stop[lane as usize], side);
                let state = match (
                    self.group[lane as usize] == self.phase[n],
                    self.protected[n],
                ) {
                    (false, _) => 0.0,
                    (true, false) => 1.0,
                    (true, true) => 2.0,
                };
                out.extend([
                    x,
                    y,
                    net.lane_heading(lane, self.stop[lane as usize]),
                    state,
                ]);
            }
        }
    }

    /// Mixes signal state into a state hash.
    pub fn hash_into(&self, mix: &mut impl FnMut(u64)) {
        for &node in &self.signals {
            let n = node as usize;
            mix(self.phase[n] as u64 | (self.protected[n] as u64) << 8);
            mix(self.elapsed[n] as u64);
            mix(self.walk_call[n] as u64);
        }
    }
}

/// A signal group's bit in `walk_call`. Nodes with more than 32 approaches share the top bit.
fn group_bit(group: u8) -> u32 {
    1 << group.min(31)
}
