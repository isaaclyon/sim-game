//! Intersection rules (ADR 0009).
//!
//! A node where 3+ roads meet is a junction. A car stops at a stop line `JUNCTION_RADIUS` short
//! of the node and asks the junction whether it may cross. Once cleared it drives through,
//! holding a time-limited reservation on its movement (in-lane → out-lane). Two movements
//! conflict when they share an exit or their paths cross. Paths are chords across a circle
//! around the node, so "do they cross" is an endpoint-interleaving test with no geometry.
//!
//! Junction ids are node ids, so they inherit the node's append-only id.

use std::f32::consts::{FRAC_PI_4, PI, TAU};

use crate::agents::{AgentId, Agents};
use crate::road::{LaneId, NodeId, RoadClass, RoadNetwork};
use crate::traffic::{ACCEL, SPACING, Traffic};

/// Distance from the node centre to the stop line, metres. Clamped on short lanes.
pub const JUNCTION_RADIUS: f32 = 8.0;
/// Angular offset of a lane's end from its road's centreline on the junction circle, radians.
/// Right-hand traffic: arriving lanes sit counter-clockwise of the road, leaving lanes clockwise.
const LANE_ANGLE: f32 = 0.2;
/// Gap a yielding car needs before a priority car arrives, seconds. It shrinks from the max to
/// the min over `PATIENCE_S` of waiting, so side roads can't starve.
const MAX_GAP_S: f32 = 4.0;
const MIN_GAP_S: f32 = 1.0;
const PATIENCE_S: f32 = 20.0;
/// An exit lane whose last car moves at least this fast, m/s, is flowing.
const MOVING: f32 = 3.0;
/// Extra time a crossing holds its reservation, seconds.
const CROSSING_MARGIN_S: f32 = 0.5;

const MIN_GREEN_S: f32 = 8.0;
const MAX_GREEN_S: f32 = 40.0;
/// Green is extended while a car will reach the stop line within this, seconds.
const GAP_OUT_S: f32 = 3.0;
/// A car this close to the stop line, seconds, calls for its phase.
const DEMAND_S: f32 = 15.0;
const PROTECTED_LEFT_S: f32 = 6.0;
/// A left-turner waiting this long on green earns a protected-left phase, seconds.
const LEFT_STARVED_S: f32 = 20.0;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[repr(u8)]
pub enum Control {
    None,
    Priority,
    Signal,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Turn {
    Straight,
    Right,
    /// Left turns and U-turns: both cross oncoming traffic.
    Left,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Light {
    Uncontrolled,
    Red,
    Green,
    Protected,
}

/// What a car at a stop line should do this tick.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Verdict {
    Go,
    /// Blocked by traffic. Counts toward the stuck limit.
    Wait,
    /// Stopped at a red light. Not gridlock, so it doesn't count as stuck.
    Red,
}

#[derive(Clone, Copy, Debug)]
struct Crossing {
    from: LaneId,
    to: LaneId,
    until: u64,
}

#[derive(Default)]
pub struct Junctions {
    seen_version: Option<u32>,
    control: Vec<Control>,
    // Per lane.
    heading: Vec<f32>,
    departure: Vec<f32>,
    /// Angle around the node at which the lane arrives, pointing back along its road.
    arrive: Vec<f32>,
    /// Where the lane's movement chords start (as an in-lane) and end (as an out-lane) on the
    /// junction circle.
    chord_in: Vec<f32>,
    chord_out: Vec<f32>,
    /// Where a front car that isn't cleared must stop: the lane end, or the stop line if the
    /// lane ends at a junction.
    pub stop: Vec<f32>,
    /// Signal group of the lane at the node it arrives at.
    group: Vec<u8>,
    // Per node.
    groups: Vec<u8>,
    phase: Vec<u8>,
    protected: Vec<bool>,
    elapsed: Vec<u32>,
    crossings: Vec<Vec<Crossing>>,
    /// Signalised nodes in id order.
    signals: Vec<NodeId>,
}

/// Road `k` owns lanes `2k` and `2k + 1`, so a lane's reverse is its id with the low bit flipped.
fn reverse(lane: LaneId) -> LaneId {
    lane ^ 1
}

/// Angle wrapped to `[0, TAU)`. Inputs here are sums and differences of a few angles, so two
/// branches replace a much slower `rem_euclid` in the hot yield checks.
fn wrap(a: f32) -> f32 {
    debug_assert!((-TAU..2.0 * TAU).contains(&a), "{a}");
    let a = if a < 0.0 { a + TAU } else { a };
    if a >= TAU { a - TAU } else { a }
}

fn radius(len: f32) -> f32 {
    JUNCTION_RADIUS.min(len * 0.3)
}

fn ticks(secs: f32, dt: f32) -> u32 {
    (secs / dt).round().max(1.0) as u32
}

impl Junctions {
    /// Recomputes control, stop lines and signal groups if roads changed. Signal timing is kept
    /// for nodes whose signal layout didn't change; reservations are always kept.
    pub fn sync(&mut self, net: &RoadNetwork) {
        if self.seen_version == Some(net.version) {
            return;
        }
        self.seen_version = Some(net.version);
        let (nodes, lanes) = (net.node_count(), net.lane_count());

        self.heading = (0..lanes as LaneId)
            .map(|l| net.lane_heading(l, net.lane_len[l as usize]))
            .collect();
        self.departure = (0..lanes as LaneId)
            .map(|l| net.lane_heading(l, 0.0))
            .collect();
        self.arrive = self.heading.iter().map(|&h| wrap(h + PI)).collect();
        self.chord_in = self.arrive.iter().map(|&a| wrap(a + LANE_ANGLE)).collect();
        self.chord_out = self
            .departure
            .iter()
            .map(|&h| wrap(h - LANE_ANGLE))
            .collect();
        let old_control = std::mem::take(&mut self.control);
        let old_groups = std::mem::take(&mut self.groups);
        self.control = (0..nodes as NodeId)
            .map(|n| {
                let out = net.out_lanes(n);
                let avenue = out.iter().any(|&l| net.lane_class(l) == RoadClass::Main);
                match out.len() {
                    0..=2 => Control::None,
                    3 => Control::Priority,
                    _ if avenue => Control::Signal,
                    _ => Control::Priority,
                }
            })
            .collect();
        self.stop = (0..lanes)
            .map(|l| {
                let len = net.lane_len[l];
                match self.control[net.lane_to[l] as usize] {
                    Control::None => len,
                    _ => len - radius(len),
                }
            })
            .collect();

        self.group = vec![0; lanes];
        self.groups = vec![0; nodes];
        self.signals.clear();
        for n in 0..nodes {
            if self.control[n] != Control::Signal {
                continue;
            }
            self.signals.push(n as NodeId);
            let mut arriving: Vec<LaneId> = net
                .out_lanes(n as NodeId)
                .iter()
                .map(|&l| reverse(l))
                .collect();
            arriving.sort_by(|&a, &b| {
                self.arrive_angle(a)
                    .total_cmp(&self.arrive_angle(b))
                    .then(a.cmp(&b))
            });
            // Four approaches pair up with their opposites; anything bigger gets a phase each.
            let pairs = arriving.len() == 4;
            for (rank, &lane) in arriving.iter().enumerate() {
                self.group[lane as usize] = if pairs { rank % 2 } else { rank } as u8;
            }
            self.groups[n] = if pairs { 2 } else { arriving.len() as u8 };
        }

        self.phase.resize(nodes, 0);
        self.protected.resize(nodes, false);
        self.elapsed.resize(nodes, 0);
        self.crossings.resize_with(nodes, Vec::new);
        for n in 0..nodes {
            let kept = old_control.get(n) == Some(&Control::Signal)
                && self.control[n] == Control::Signal
                && old_groups.get(n) == Some(&self.groups[n]);
            if !kept {
                self.phase[n] = 0;
                self.protected[n] = false;
                self.elapsed[n] = 0;
            }
        }
    }

    /// Keep reservations at the old far end attached to the same geometry. A car may have
    /// left the edited road already while its crossing reservation still protects it.
    pub(crate) fn remap_split(&mut self, road: crate::road::RoadId, rest: crate::road::RoadId) {
        for crossings in &mut self.crossings {
            for c in crossings {
                if c.from == 2 * road {
                    c.from = 2 * rest;
                }
                if c.to == 2 * road + 1 {
                    c.to = 2 * rest + 1;
                }
            }
        }
    }

    /// Angle around the node at which `lane` arrives (pointing back along its road).
    fn arrive_angle(&self, lane: LaneId) -> f32 {
        self.arrive[lane as usize]
    }

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
    fn conflicts(&self, a1: LaneId, b1: LaneId, a2: LaneId, b2: LaneId) -> bool {
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
    fn must_yield(
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
                let (ca, cb) = (net.lane_class(a), net.lane_class(b));
                if ca != cb {
                    return cb == RoadClass::Main; // The main road has right of way.
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
        }
    }
}

/// The lane `agent` takes after its current one, if any.
pub fn next_lane(agents: &Agents, agent: AgentId) -> Option<LaneId> {
    let a = agent as usize;
    agents.path[a].get(agents.path_idx[a] as usize + 1).copied()
}

#[cfg(test)]
mod tests {
    use super::*;
    /// A junction at the origin with arms 100 m away, east, north, west, south in that order,
    /// skipping any arm whose class is `None`. Arm `k` owns lane `2k` (out of the junction)
    /// and `2k + 1` (into it).
    fn junction(arms: [Option<RoadClass>; 4]) -> (RoadNetwork, Junctions) {
        let mut net = RoadNetwork::default();
        let centre = net.add_node(0.0, 0.0);
        for (k, class) in arms.iter().enumerate() {
            let Some(class) = *class else { continue };
            let angle = k as f32 * PI / 2.0;
            let arm = net.add_node(100.0 * angle.cos(), 100.0 * angle.sin());
            net.add_road(centre, arm, class);
        }
        net.rebuild_adjacency();
        let mut j = Junctions::default();
        j.sync(&net);
        (net, j)
    }

    const S: Option<RoadClass> = Some(RoadClass::Street);
    const A: Option<RoadClass> = Some(RoadClass::Main);
    // Lanes of the four-way junction.
    const IN_E: LaneId = 1;
    const IN_N: LaneId = 3;
    const IN_W: LaneId = 5;
    const IN_S: LaneId = 7;
    const OUT_E: LaneId = 0;
    const OUT_N: LaneId = 2;
    const OUT_W: LaneId = 4;
    const OUT_S: LaneId = 6;

    #[test]
    fn splitting_curve_preserves_reservations_at_the_old_far_end() {
        let mut net = RoadNetwork::default();
        let a = net.add_node(0.0, 0.0);
        let b = net.add_node(200.0, 0.0);
        let c = net.add_node(300.0, 0.0);
        net.add_curve(a, b, (100.0, 100.0), RoadClass::Street);
        net.add_road(b, c, RoadClass::Street);
        net.rebuild_adjacency();
        let mut j = Junctions::default();
        j.sync(&net);
        j.crossings[b as usize] = vec![
            Crossing {
                from: 0,
                to: 2,
                until: 50,
            },
            Crossing {
                from: 3,
                to: 1,
                until: 60,
            },
        ];
        let incoming = j.chord_in[0];
        let outgoing = j.chord_out[1];
        let mid = net.add_node(100.0, 50.0);
        let rest = net.split_road(0, mid);
        j.remap_split(0, rest);
        net.rebuild_adjacency();
        j.sync(&net);
        let crossings = &j.crossings[b as usize];
        assert_eq!(
            (crossings[0].from, crossings[0].to, crossings[0].until),
            (2 * rest, 2, 50)
        );
        assert_eq!(
            (crossings[1].from, crossings[1].to, crossings[1].until),
            (3, 2 * rest + 1, 60)
        );
        assert!((j.chord_in[2 * rest as usize] - incoming).abs() < 1e-5);
        assert!((j.chord_out[2 * rest as usize + 1] - outgoing).abs() < 1e-5);
    }

    #[test]
    fn control_follows_shape() {
        assert_eq!(junction([S, S, S, S]).1.control[0], Control::Priority);
        assert_eq!(junction([A, S, A, S]).1.control[0], Control::Signal);
        assert_eq!(junction([A, None, A, S]).1.control[0], Control::Priority);
        assert_eq!(junction([S, None, S, None]).1.control[0], Control::None);
    }

    #[test]
    fn turns_are_classified_for_right_hand_traffic() {
        let (_, j) = junction([S, S, S, S]);
        // Arriving from the east means driving west, so north is on the right.
        assert_eq!(j.turn(IN_E, OUT_W), Turn::Straight);
        assert_eq!(j.turn(IN_E, OUT_N), Turn::Right);
        assert_eq!(j.turn(IN_E, OUT_S), Turn::Left);
        assert_eq!(
            j.turn(IN_E, OUT_E),
            Turn::Left,
            "U-turns cross oncoming traffic"
        );
    }

    #[test]
    fn conflicts_follow_crossing_paths() {
        let (_, j) = junction([S, S, S, S]);
        assert!(j.conflicts(IN_E, OUT_W, IN_N, OUT_S), "crossing straights");
        assert!(!j.conflicts(IN_E, OUT_W, IN_W, OUT_E), "opposite straights");
        assert!(
            !j.conflicts(IN_E, OUT_N, IN_W, OUT_S),
            "opposite right turns"
        );
        assert!(
            j.conflicts(IN_E, OUT_S, IN_W, OUT_E),
            "left across oncoming"
        );
        assert!(
            j.conflicts(IN_E, OUT_W, IN_N, OUT_W),
            "merging into one exit"
        );
        assert!(
            !j.conflicts(IN_E, OUT_W, IN_E, OUT_N),
            "same lane keeps queue order"
        );
        // Symmetric.
        assert!(j.conflicts(IN_N, OUT_S, IN_E, OUT_W));
    }

    #[test]
    fn equal_roads_yield_to_the_right() {
        let (net, j) = junction([S, S, S, S]);
        let mine = |lane, next| (lane, j.turn(lane, next), Light::Uncontrolled);
        // Driving north from the south arm, the east arm is on the right.
        assert!(j.must_yield(&net, 0, mine(IN_S, OUT_N), (IN_E, OUT_W)));
        assert!(!j.must_yield(&net, 0, mine(IN_E, OUT_W), (IN_S, OUT_N)));
        // Left-turners give way to oncoming traffic.
        assert!(j.must_yield(&net, 0, mine(IN_E, OUT_S), (IN_W, OUT_E)));
        assert!(!j.must_yield(&net, 0, mine(IN_W, OUT_E), (IN_E, OUT_S)));
    }

    #[test]
    fn side_road_yields_to_the_main_road() {
        // An avenue running east-west with a street joining from the south.
        let (net, j) = junction([A, None, A, S]);
        let (in_w, out_w, in_s) = (3, 2, 5);
        let mine = |lane, next| (lane, j.turn(lane, next), Light::Uncontrolled);
        assert!(j.must_yield(&net, 0, mine(in_s, out_w), (IN_E, out_w)));
        assert!(!j.must_yield(&net, 0, mine(IN_E, out_w), (in_s, out_w)));
        assert!(!j.must_yield(&net, 0, mine(in_w, OUT_E), (in_s, OUT_E)));
    }

    #[test]
    fn signals_pair_opposite_approaches() {
        let (net, j) = junction([A, S, A, S]);
        assert_eq!(j.groups[0], 2);
        assert_eq!(j.group[IN_E as usize], j.group[IN_W as usize]);
        assert_eq!(j.group[IN_N as usize], j.group[IN_S as usize]);
        assert_ne!(j.group[IN_E as usize], j.group[IN_N as usize]);
        let mut out = Vec::new();
        j.write_signal_snapshot(&net, 6.0, &mut out);
        assert_eq!(out.len(), 4 * 4);
        let green = out.chunks(4).filter(|s| s[3] == 1.0).count();
        assert_eq!(green, 2, "one pair of approaches is green");
    }
}
