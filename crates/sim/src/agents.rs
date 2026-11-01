//! Vehicle agents, stored struct-of-arrays and indexed by `AgentId`.
//!
//! Synthetic agents cycle between random trips and parking. Citizens lazily reserve a stable
//! vehicle slot, dormant between trips and never reassigned to another citizen (ADR 0021).
//! Population comes from the separate resident table, not these allocated slots.

use crate::road::{LaneId, NodeId};

pub type AgentId = u32;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[repr(u8)]
pub enum AgentState {
    /// Sitting at `node` until its wake-up tick. Costs nothing per tick.
    Parked,
    /// Reserved citizen vehicle slot, not currently travelling; no random wake timer.
    Dormant,
    /// Has a destination and is waiting for the pathfinding budget.
    AwaitingPath,
    /// Has a path and is waiting for room on its first lane.
    Entering,
    /// On `lane`, `pos` metres from its start.
    Driving,
}

#[derive(Default)]
pub struct Agents {
    pub state: Vec<AgentState>,
    pub citizen: Vec<Option<crate::citizen::CitizenId>>,
    /// Where the agent is parked, or where its current trip started.
    pub node: Vec<NodeId>,
    pub dest: Vec<NodeId>,
    pub lane: Vec<LaneId>,
    pub pos: Vec<f32>,
    pub speed: Vec<f32>,
    /// Lanes for the current trip. Each agent keeps its Vec so capacity is reused across trips.
    pub path: Vec<Vec<LaneId>>,
    pub path_idx: Vec<u32>,
    /// Consecutive ticks spent blocked at the end of a lane or a stop line.
    pub stuck_ticks: Vec<u16>,
    /// Allowed through the junction at the end of the current lane.
    pub cleared: Vec<bool>,
}

impl Agents {
    pub fn len(&self) -> usize {
        self.state.len()
    }

    pub fn is_empty(&self) -> bool {
        self.state.is_empty()
    }

    pub fn push(&mut self, node: NodeId) -> AgentId {
        self.state.push(AgentState::Parked);
        self.citizen.push(None);
        self.node.push(node);
        self.dest.push(node);
        self.lane.push(0);
        self.pos.push(0.0);
        self.speed.push(0.0);
        self.path.push(Vec::new());
        self.path_idx.push(0);
        self.stuck_ticks.push(0);
        self.cleared.push(false);
        (self.state.len() - 1) as AgentId
    }
}
