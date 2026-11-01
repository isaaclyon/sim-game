//! Intersection rules (ADR 0009).
//!
//! A node where 3+ roads meet is a junction. A car stops at a stop line `JUNCTION_RADIUS` short
//! of the node and asks the junction whether it may cross. Once cleared it drives through,
//! holding a time-limited reservation on its movement (in-lane → out-lane). Two movements
//! conflict when they share an exit or their paths cross. Paths are chords across a circle
//! around the node, so "do they cross" is an endpoint-interleaving test with no geometry.
//!
//! Junction ids are node ids, so they inherit the node's append-only id.

use std::f32::consts::{PI, TAU};

use crate::agents::{AgentId, Agents};
use crate::road::{LaneId, NodeId, RoadClass, RoadNetwork};

mod movement;
mod signal;
#[cfg(test)]
mod tests;

/// Distance from the node centre to the stop line, metres. Clamped on short lanes.
pub const JUNCTION_RADIUS: f32 = 8.0;
/// Angular offset of a lane's end from its road's centreline on the junction circle, radians.
/// Right-hand traffic: arriving lanes sit counter-clockwise of the road, leaving lanes clockwise.
const LANE_ANGLE: f32 = 0.2;

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
}

/// The lane `agent` takes after its current one, if any.
pub fn next_lane(agents: &Agents, agent: AgentId) -> Option<LaneId> {
    let a = agent as usize;
    agents.path[a].get(agents.path_idx[a] as usize + 1).copied()
}
