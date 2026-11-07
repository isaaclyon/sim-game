//! Emergency vehicles at junctions (ADR 0053): a fire truck ignores signals and right of way,
//! but still waits for room on its exit lane, so it queues behind cars like anyone else.

use super::{Crossing, Junctions, Verdict, radius, ticks};
use crate::agents::{AgentId, Agents};
use crate::road::{LaneId, RoadNetwork};
use crate::traffic::{SPACING, Traffic};

impl Junctions {
    /// Clears `agent` from `lane` into `next` whatever the light and whoever has priority,
    /// if `next` has room, and holds the movement so crossing cars yield to it.
    #[allow(clippy::too_many_arguments)] // The same inputs as `request`.
    pub fn emergency(
        &mut self,
        net: &RoadNetwork,
        traffic: &Traffic,
        agents: &Agents,
        agent: AgentId,
        lane: LaneId,
        next: LaneId,
        tick: u64,
        dt: f32,
    ) -> Verdict {
        let ahead = traffic.queues[lane as usize]
            .iter()
            .take_while(|&&b| b != agent)
            .filter(|&&b| super::next_lane(agents, b) == Some(next))
            .count();
        if !self.exit_clear_for(net, traffic, agents, next, ahead) {
            return Verdict::Wait;
        }
        let n = net.lane_to[lane as usize] as usize;
        // Long enough to cross at a crawl: the stop line to past the exit's stop line.
        let d = net.lane_len[lane as usize] - self.stop[lane as usize]
            + radius(net.lane_len[next as usize])
            + SPACING;
        let t = d / net.lane_speed[lane as usize].min(5.0) + 1.0;
        self.crossings[n].retain(|c| c.until > tick);
        self.crossings[n].push(Crossing {
            from: lane,
            to: next,
            until: tick + u64::from(ticks(t, dt)),
        });
        Verdict::Go
    }
}
