//! Movement rules: turns, conflicts, right of way, and whether a car may cross (ADR 0009).

use std::f32::consts::{FRAC_PI_4, PI};

use super::{
    Control, Crossing, Junctions, Light, Turn, Verdict, next_lane, radius, reverse, ticks, wrap,
};
use crate::agents::{AgentId, Agents};
use crate::road::{LaneId, NodeId, RoadNetwork};
use crate::traffic::{ACCEL, SPACING, Traffic};

/// Gap a yielding car needs before a priority car arrives, seconds. It shrinks from the max to
/// the min over `PATIENCE_S` of waiting, so side roads can't starve.
const MAX_GAP_S: f32 = 4.0;
const MIN_GAP_S: f32 = 1.0;
const PATIENCE_S: f32 = 20.0;
/// An exit lane whose last car moves at least this fast, m/s, is flowing.
const MOVING: f32 = 3.0;
/// Extra time a crossing holds its reservation, seconds.
const CROSSING_MARGIN_S: f32 = 0.5;

impl Junctions {
    pub fn turn(&self, from: LaneId, to: LaneId) -> Turn {
        let t = wrap(self.departure[to as usize] - self.heading[from as usize] + PI) - PI;
        if t.abs() < FRAC_PI_4 {
            Turn::Straight
        } else if t < 0.0 && t > -5.0 * PI / 6.0 {
            Turn::Right
        } else {
            Turn::Left
        }
    }

    /// True if movements `a1 → b1` and `a2 → b2` through the same node can collide.
    pub(super) fn conflicts(&self, a1: LaneId, b1: LaneId, a2: LaneId, b2: LaneId) -> bool {
        if a1 == a2 {
            return false; // Same lane: the queue keeps them in order.
        }
        if b1 == b2 {
            return true; // Merging into the same exit.
        }
        let p1 = self.chord_in[a1 as usize];
        let q1 = self.chord_out[b1 as usize];
        let p2 = self.chord_in[a2 as usize];
        let q2 = self.chord_out[b2 as usize];
        // Two chords cross iff exactly one end of the second lies on the arc p1 → q1.
        let arc = wrap(q1 - p1);
        let inside = |x: f32| {
            let d = wrap(x - p1);
            d > 0.0 && d < arc
        };
        inside(p2) != inside(q2)
    }

    fn light(&self, node: NodeId, lane: LaneId, turn: Turn) -> Light {
        let n = node as usize;
        if self.control[n] != Control::Signal {
            return Light::Uncontrolled;
        }
        if self.group[lane as usize] != self.phase[n] {
            Light::Red
        } else if !self.protected[n] {
            Light::Green
        } else if turn == Turn::Straight {
            Light::Red
        } else {
            Light::Protected
        }
    }

    /// True if a car going `a → a_next` must give way to one going `b → b_next`.
    pub(super) fn must_yield(
        &self,
        net: &RoadNetwork,
        node: NodeId,
        (a, a_turn, a_light): (LaneId, Turn, Light),
        (b, b_next): (LaneId, LaneId),
    ) -> bool {
        let b_turn = self.turn(b, b_next);
        let rel = wrap(self.arrive_angle(b) - self.arrive_angle(a));
        let opposite = (rel - PI).abs() < PI / 6.0;
        let left_across = opposite && a_turn == Turn::Left && b_turn != Turn::Left;
        match self.control[node as usize] {
            Control::None => false,
            Control::Priority => {
                let (sa, sb) = (net.lane_class(a).speed(), net.lane_class(b).speed());
                if sa != sb {
                    return sb > sa; // The bigger road has right of way.
                }
                // Counter-clockwise from your own road is your right (y up, right-hand traffic).
                let from_right = rel > 0.0 && rel < 5.0 * PI / 6.0;
                from_right || left_across
            }
            Control::Signal => match self.light(node, b, b_turn) {
                Light::Red => false,
                _ => a_light == Light::Red || (a_light == Light::Green && left_across),
            },
        }
    }

    /// Decides whether the front car on `lane`, waiting at its stop line, may cross to `next`.
    /// On `Go` the crossing is reserved and the caller marks the car cleared.
    #[allow(clippy::too_many_arguments)]
    pub fn request(
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
        let node = net.lane_to[lane as usize];
        let n = node as usize;
        // Don't block the box: the exit must have room for the whole car past the junction, on
        // top of cars ahead in this lane already cleared into it.
        let ahead = traffic.queues[lane as usize]
            .iter()
            .take_while(|&&b| b != agent)
            .filter(|&&b| next_lane(agents, b) == Some(next))
            .count();
        if !self.exit_clear_for(net, traffic, agents, next, ahead) {
            return Verdict::Wait;
        }
        let turn = self.turn(lane, next);
        let light = self.light(node, lane, turn);
        if light == Light::Red && turn != Turn::Right {
            return Verdict::Red;
        }

        self.crossings[n].retain(|c| c.until > tick);
        if self.crossings[n]
            .iter()
            .any(|c| self.conflicts(lane, next, c.from, c.to))
        {
            return Verdict::Wait;
        }

        let a = agent as usize;
        let waited = agents.stuck_ticks[a];
        let patience = (waited as f32 * dt / PATIENCE_S).min(1.0);
        let gap = MAX_GAP_S - (MAX_GAP_S - MIN_GAP_S) * patience;
        let patience_ticks = ticks(PATIENCE_S, dt);
        for &out in net.out_lanes(node) {
            let other = reverse(out);
            if other == lane {
                continue;
            }
            // Cleared cars already hold a reservation, checked above.
            let Some(b) = traffic.waiting(other, agents) else {
                continue;
            };
            let bi = b as usize;
            let Some(b_next) = next_lane(agents, b) else {
                continue; // It ends its trip here.
            };
            if !self.conflicts(lane, next, other, b_next) {
                continue;
            }
            let dist = self.stop[other as usize] - agents.pos[bi];
            let b_waited = agents.stuck_ticks[bi];
            // A car that has waited past its patience at the line gets the right of way over
            // anyone who has waited less (lights still apply). Otherwise a steady stream on a
            // priority movement keeps the junction reserved and the waiting car never gets in.
            let starving = dist <= 1e-3
                && b_waited as u32 >= patience_ticks
                && b_waited > waited
                && self.light(node, other, self.turn(other, b_next)) != Light::Red;
            if !starving && !self.must_yield(net, node, (lane, turn, light), (other, b_next)) {
                continue;
            }
            if dist <= 1e-3 {
                // Both waiting at the line. A car that can't leave anyway doesn't hold anyone up,
                // and the longest waiter breaks yield-to-the-right cycles.
                if !self.exit_clear(net, traffic, agents, b_next) {
                    continue;
                }
                if b_waited > waited || (b_waited == waited && other < lane) {
                    return Verdict::Wait;
                }
            } else if dist / agents.speed[bi].max(0.5) < gap {
                return Verdict::Wait;
            }
        }

        // Hold the movement until the car has covered the stop line → junction exit.
        let d = net.lane_len[lane as usize] - self.stop[lane as usize]
            + radius(net.lane_len[next as usize]);
        let v = agents.speed[a];
        let accel_t = ((v * v + 2.0 * ACCEL * d).sqrt() - v) / ACCEL;
        let t = accel_t.max(d / net.lane_speed[lane as usize]) + CROSSING_MARGIN_S;
        self.crossings[n].push(Crossing {
            from: lane,
            to: next,
            until: tick + ticks(t, dt) as u64,
        });
        Verdict::Go
    }

    /// True if `lane` will have room for a car to clear the junction it starts at: either the
    /// last car is already far enough along, or it's moving and the lane isn't nearly full.
    pub fn exit_clear(
        &self,
        net: &RoadNetwork,
        traffic: &Traffic,
        agents: &Agents,
        lane: LaneId,
    ) -> bool {
        self.exit_clear_for(net, traffic, agents, lane, 0)
    }

    /// `exit_clear`, with room also kept for `ahead` cars already cleared into `lane` but not on
    /// it yet.
    fn exit_clear_for(
        &self,
        net: &RoadNetwork,
        traffic: &Traffic,
        agents: &Agents,
        lane: LaneId,
        ahead: usize,
    ) -> bool {
        let queue = &traffic.queues[lane as usize];
        let len = net.lane_len[lane as usize];
        let need = (1 + ahead) as f32 * SPACING + radius(len);
        let Some(&last) = queue.back() else {
            return ahead == 0 || len >= need;
        };
        let last = last as usize;
        agents.pos[last] >= need
            || (agents.speed[last] >= MOVING
                && len - (queue.len() + ahead) as f32 * SPACING >= need + SPACING)
    }
}
