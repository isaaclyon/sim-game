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
        self.queues[lane as usize].push_back(agent);
    }

    /// Moves every vehicle forward and returns lanes whose front vehicle reached the end.
    /// The caller decides what happens at the end (next lane, arrival, or waiting).
    pub fn advance(&mut self, net: &RoadNetwork, agents: &mut Agents, dt: f32) -> Vec<LaneId> {
        let mut exits = std::mem::take(&mut self.exits);
        exits.clear();
        for (lane, queue) in self.queues.iter().enumerate() {
            let Some(&front) = queue.front() else {
                continue;
            };
            let (len, limit) = (net.lane_len[lane], net.lane_speed[lane]);
            let mut max_pos = len;
            for &agent in queue {
                let a = agent as usize;
                let old = agents.pos[a];
                let wanted = (agents.speed[a] + ACCEL * dt).min(limit);
                let new = (old + wanted * dt).min(max_pos).max(old);
                agents.pos[a] = new;
                agents.speed[a] = (new - old) / dt;
                max_pos = new - SPACING;
            }
            if agents.pos[front as usize] >= len {
                exits.push(lane as LaneId);
            }
        }
        exits
    }

    /// Hands the buffer from `advance` back so its allocation is reused next tick.
    pub fn recycle_exits(&mut self, exits: Vec<LaneId>) {
        self.exits = exits;
    }
}
