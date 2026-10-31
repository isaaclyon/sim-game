//! Lane-queue traffic.
//!
//! Each lane keeps its vehicles ordered front to back, and a vehicle only looks at the one
//! directly ahead. A tick is O(vehicles) with no spatial queries or collision checks, and jams
//! still emerge because a full lane refuses new entries.

use std::collections::VecDeque;

use crate::agents::{AgentId, AgentState, Agents};
use crate::road::{LaneId, RoadNetwork};

/// Front-to-front distance between queued cars, metres. Car length plus a safety gap.
pub const SPACING: f32 = 7.0;
/// Acceleration, m/s². Braking is instant, which the queue model tolerates fine.
pub const ACCEL: f32 = 3.0;
/// A car asks the junction ahead for permission this long before reaching its stop line,
/// seconds, so it can keep its speed through when the way is clear.
pub const LOOKAHEAD_S: f32 = 1.5;

#[derive(Default)]
pub struct Traffic {
    /// Per lane, front vehicle first.
    pub queues: Vec<VecDeque<AgentId>>,
    exits: Vec<LaneId>,
}

impl Traffic {
    pub fn ensure_lanes(&mut self, lane_count: usize) {
        if self.queues.len() < lane_count {
            self.queues.resize_with(lane_count, VecDeque::new);
        }
    }

    /// True if a vehicle can enter at the start of `lane` without overlapping the last one.
    pub fn has_room(&self, lane: LaneId, agents: &Agents) -> bool {
        match self.queues[lane as usize].back() {
            None => true,
            Some(&last) => agents.pos[last as usize] >= SPACING,
        }
    }

    /// Puts `agent` at the start of `lane`, keeping its current speed.
    pub fn enter(&mut self, lane: LaneId, agent: AgentId, agents: &mut Agents) {
        let a = agent as usize;
        agents.state[a] = AgentState::Driving;
        agents.lane[a] = lane;
        agents.pos[a] = 0.0;
        agents.stuck_ticks[a] = 0;
        agents.cleared[a] = false;
        self.queues[lane as usize].push_back(agent);
    }

    /// Index in `lane`'s queue of the first vehicle not yet cleared through the junction ahead.
    /// Cleared vehicles are always a prefix of the queue: only this one may ask, and a cleared
    /// vehicle stays cleared until it leaves the lane.
    pub fn first_waiting(&self, lane: LaneId, agents: &Agents) -> Option<usize> {
        self.queues[lane as usize]
            .iter()
            .position(|&b| !agents.cleared[b as usize])
    }

    /// The vehicle `first_waiting` points at.
    pub fn waiting(&self, lane: LaneId, agents: &Agents) -> Option<AgentId> {
        self.first_waiting(lane, agents)
            .map(|k| self.queues[lane as usize][k])
    }

    /// True if `agent`, not cleared, is close enough to `lane`'s stop line to ask the junction:
    /// `LOOKAHEAD_S` ahead at its current speed, so it can keep that speed through.
    pub fn wants_to_ask(
        net: &RoadNetwork,
        agents: &Agents,
        stop: &[f32],
        lane: LaneId,
        agent: AgentId,
    ) -> bool {
        let (a, l) = (agent as usize, lane as usize);
        let ask_from = if stop[l] < net.lane_len[l] {
            stop[l] - (agents.speed[a] * LOOKAHEAD_S).max(0.5)
        } else {
            stop[l]
        };
        agents.pos[a] >= ask_from
    }

    /// Moves every vehicle forward and returns lanes where the front vehicle reached the end, or
    /// the first vehicle not yet cleared is about to reach the `stop` line. The caller decides
    /// what happens next (cross, next lane, arrival, or waiting).
    pub fn advance(
        &mut self,
        net: &RoadNetwork,
        agents: &mut Agents,
        stop: &[f32],
        dt: f32,
    ) -> Vec<LaneId> {
        let mut exits = std::mem::take(&mut self.exits);
        exits.clear();
        for (lane, queue) in self.queues.iter().enumerate() {
            let Some(&front) = queue.front() else {
                continue;
            };
            let (len, limit) = (net.lane_len[lane], net.lane_speed[lane]);
            let mut max_pos = len;
            let mut asker = None;
            for &agent in queue {
                let a = agent as usize;
                if !agents.cleared[a] {
                    // Nobody creeps past the stop line without being cleared.
                    max_pos = max_pos.min(stop[lane]);
                    asker.get_or_insert(agent);
                }
                let old = agents.pos[a];
                let wanted = (agents.speed[a] + ACCEL * dt).min(limit);
                let new = (old + wanted * dt).min(max_pos).max(old);
                agents.pos[a] = new;
                agents.speed[a] = (new - old) / dt;
                max_pos = new - SPACING;
            }
            let lane = lane as LaneId;
            if agents.pos[front as usize] >= len
                || asker.is_some_and(|b| Self::wants_to_ask(net, agents, stop, lane, b))
            {
                exits.push(lane);
            }
        }
        exits
    }

    /// Hands the buffer from `advance` back so its allocation is reused next tick.
    pub fn recycle_exits(&mut self, exits: Vec<LaneId>) {
        self.exits = exits;
    }
}
