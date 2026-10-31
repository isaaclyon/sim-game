//! The world: owns all sim state and advances it one fixed step at a time.

use std::cmp::Reverse;
use std::collections::{BinaryHeap, VecDeque};

use crate::agents::{AgentId, AgentState, Agents};
use crate::junction::{Junctions, Verdict};
use crate::path::Pathfinder;
use crate::rng::Rng;
use crate::road::{LaneId, RoadNetwork, STREET_SPEED};
use crate::traffic::Traffic;

/// Road endpoints within this many metres of an existing node connect to it.
const SNAP_RADIUS: f32 = 15.0;
/// Distance from the road centreline to the middle of each lane, metres.
const LANE_OFFSET: f32 = 2.0;
/// Floats per agent in `write_agent_snapshot`.
pub const SNAPSHOT_STRIDE: usize = 4;
/// Floats per signalised approach in `write_signal_snapshot`.
pub const SIGNAL_STRIDE: usize = 4;
/// Signal heads stand this far right of the road centreline, metres.
const SIGNAL_OFFSET: f32 = 6.0;
/// Expected delay each queued car adds to a lane, seconds. Feeds route choice so traffic
/// spreads off backed-up avenues.
const QUEUE_DELAY_S: f32 = 1.0;
/// A car held this long by a full exit looks for another way, then retries this often, seconds.
const DETOUR_AFTER_S: f32 = 10.0;
const DETOUR_EVERY_S: f32 = 5.0;
/// A detour search gives up after expanding this many nodes.
const DETOUR_MAX_NODES: u32 = 256;
/// Wait before retrying after a failed path or a stuck despawn, seconds.
const RETRY_AFTER_S: f32 = 10.0;

#[derive(Clone, Debug)]
pub struct Config {
    pub seed: u64,
    /// Sim seconds per tick. The frontend's tick clock must match.
    pub dt: f32,
    /// Max pathfinding requests served per tick; the rest wait in a queue.
    pub path_budget: u32,
    pub dwell_min_s: f32,
    pub dwell_max_s: f32,
    /// A vehicle blocked this long at a lane end is despawned, the way gridlock is cleared in
    /// most city builders.
    pub stuck_limit_s: f32,
    pub grid_spacing: f32,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            seed: 1,
            dt: 0.1,
            path_budget: 1000,
            dwell_min_s: 20.0,
            dwell_max_s: 240.0,
            stuck_limit_s: 60.0,
            grid_spacing: 120.0,
        }
    }
}

/// Player actions. Everything that changes the world from outside goes through `World::apply`,
/// so the seed plus a command log is enough to replay a session.
#[derive(Clone, Debug)]
pub enum Command {
    SpawnAgents { count: u32 },
    BuildRoad { ax: f32, ay: f32, bx: f32, by: f32 },
}

#[derive(Clone, Copy, Default, Debug)]
pub struct Stats {
    pub tick: u64,
    pub agents: u32,
    pub parked: u32,
    pub awaiting_path: u32,
    pub entering: u32,
    pub driving: u32,
    pub trips_completed: u64,
    pub paths_found: u64,
    pub paths_failed: u64,
    pub despawned_stuck: u64,
    /// Cars that turned off a route blocked by a full lane.
    pub detours: u64,
}

pub struct World {
    pub cfg: Config,
    pub roads: RoadNetwork,
    pub agents: Agents,
    traffic: Traffic,
    pub junctions: Junctions,
    pathfinder: Pathfinder,
    path_queue: VecDeque<AgentId>,
    lane_delay: Vec<f32>,
    detour: Vec<LaneId>,
    /// Scratch for `resolve_exits`: front cars that stay put this tick.
    held: Vec<(AgentId, LaneId, bool)>,
    entering: Vec<AgentId>,
    /// Parked agents keyed by wake-up tick. Ties pop in agent-id order, so it stays deterministic.
    wake: BinaryHeap<Reverse<(u64, AgentId)>>,
    rng: Rng,
    totals: Stats,
}

impl World {
    pub fn new(cfg: Config, roads: RoadNetwork, rng: Rng) -> Self {
        let mut world = Self {
            cfg,
            roads,
            agents: Agents::default(),
            traffic: Traffic::default(),
            junctions: Junctions::default(),
            pathfinder: Pathfinder::default(),
            path_queue: VecDeque::new(),
            lane_delay: Vec::new(),
            detour: Vec::new(),
            held: Vec::new(),
            entering: Vec::new(),
            wake: BinaryHeap::new(),
            rng,
            totals: Stats::default(),
        };
        world.sync_roads();
        world
    }

    /// A world on a generated `cols` × `rows` street grid.
    pub fn generate(cfg: Config, cols: u32, rows: u32) -> Self {
        let mut rng = Rng::new(cfg.seed);
        let roads = RoadNetwork::generate_grid(cols, rows, cfg.grid_spacing, &mut rng);
        Self::new(cfg, roads, rng)
    }

    pub fn tick_count(&self) -> u64 {
        self.totals.tick
    }

    pub fn apply(&mut self, cmd: Command) -> bool {
        match cmd {
            Command::SpawnAgents { count } => self.spawn_agents(count),
            Command::BuildRoad { ax, ay, bx, by } => self.build_road(ax, ay, bx, by),
        }
    }

    pub fn tick(&mut self) {
        self.sync_roads();
        self.wake_parked();
        self.lane_delay.clear();
        self.lane_delay.extend(
            self.traffic
                .queues
                .iter()
                .map(|q| q.len() as f32 * QUEUE_DELAY_S),
        );
        self.plan_paths();
        self.enter_lanes();
        self.junctions
            .update_signals(&self.roads, &self.traffic, &self.agents, self.cfg.dt);
        let exits = self.traffic.advance(
            &self.roads,
            &mut self.agents,
            &self.junctions.stop,
            self.cfg.dt,
        );
        self.resolve_exits(&exits);
        self.traffic.recycle_exits(exits);
        self.totals.tick += 1;
    }

    pub fn stats(&self) -> Stats {
        let mut s = self.totals;
        s.agents = self.agents.len() as u32;
        for state in &self.agents.state {
            match state {
                AgentState::Parked => s.parked += 1,
                AgentState::AwaitingPath => s.awaiting_path += 1,
                AgentState::Entering => s.entering += 1,
                AgentState::Driving => s.driving += 1,
            }
        }
        s
    }

    /// Writes `[x, y, heading, speed_ratio]` per agent, indexed by agent id so the renderer can
    /// interpolate between snapshots. Heading is radians from +x. Agents not on a road get
    /// `speed_ratio = -1`.
    pub fn write_agent_snapshot(&self, out: &mut Vec<f32>) {
        let a = &self.agents;
        out.clear();
        out.reserve(a.len() * SNAPSHOT_STRIDE);
        for i in 0..a.len() {
            if a.state[i] == AgentState::Driving {
                let lane = a.lane[i];
                let (x, y) = self.roads.lane_point(lane, a.pos[i], LANE_OFFSET);
                let heading = self.roads.lane_heading(lane);
                out.extend([
                    x,
                    y,
                    heading,
                    a.speed[i] / self.roads.lane_speed[lane as usize],
                ]);
            } else {
                out.extend([0.0, 0.0, 0.0, -1.0]);
            }
        }
    }

    /// Writes `[ax, ay, bx, by, speed_limit]` per road.
    pub fn write_road_segments(&self, out: &mut Vec<f32>) {
        let r = &self.roads;
        out.clear();
        out.reserve(r.road_count() * 5);
        for road in 0..r.road_count() {
            let lane = road * 2;
            let (a, b) = (r.lane_from[lane] as usize, r.lane_to[lane] as usize);
            out.extend([
                r.node_x[a],
                r.node_y[a],
                r.node_x[b],
                r.node_y[b],
                r.lane_speed[lane],
            ]);
        }
    }

    /// Writes `[x, y, heading, state]` per signalised approach, at its stop line. State is
    /// 0 = red, 1 = green, 2 = protected-left green; heading is the approach's travel direction.
    pub fn write_signal_snapshot(&self, out: &mut Vec<f32>) {
        self.junctions
            .write_signal_snapshot(&self.roads, SIGNAL_OFFSET, out);
    }

    /// FNV-1a over the dynamic state. Two worlds with equal hashes are (almost surely) identical.
    pub fn state_hash(&self) -> u64 {
        let mut h: u64 = 0xcbf2_9ce4_8422_2325;
        let mut mix = |v: u64| {
            h ^= v;
            h = h.wrapping_mul(0x0000_0100_0000_01b3);
        };
        mix(self.totals.tick);
        let a = &self.agents;
        for i in 0..a.len() {
            mix(a.state[i] as u64);
            mix(a.lane[i] as u64);
            mix(a.pos[i].to_bits() as u64);
            mix(a.dest[i] as u64);
            mix(a.cleared[i] as u64);
        }
        self.junctions.hash_into(&mut mix);
        h
    }

    fn spawn_agents(&mut self, count: u32) -> bool {
        self.sync_roads();
        let starts: Vec<_> = (0..self.roads.node_count() as u32)
            .filter(|&n| !self.roads.out_lanes(n).is_empty())
            .collect();
        if starts.len() < 2 {
            return false;
        }
        for _ in 0..count {
            let node = starts[self.rng.below(starts.len() as u32) as usize];
            let agent = self.agents.push(node);
            // Stagger first departures so traffic ramps up instead of arriving in one wave.
            let delay = self.rng.range(0.0, self.cfg.dwell_max_s);
            self.park(agent, delay);
        }
        true
    }

    fn build_road(&mut self, ax: f32, ay: f32, bx: f32, by: f32) -> bool {
        let a = self.roads.nearest_node(ax, ay, SNAP_RADIUS);
        let b = self.roads.nearest_node(bx, by, SNAP_RADIUS);
        if a.is_some() && a == b {
            return false;
        }
        let a = a.unwrap_or_else(|| self.roads.add_node(ax, ay));
        let b = b.unwrap_or_else(|| self.roads.add_node(bx, by));
        let built = self.roads.add_road(a, b, STREET_SPEED);
        self.sync_roads();
        built
    }

    fn sync_roads(&mut self) {
        self.roads.rebuild_adjacency();
        self.traffic.ensure_lanes(self.roads.lane_count());
        self.junctions.sync(&self.roads);
    }

    fn secs_to_ticks(&self, secs: f32) -> u64 {
        (secs / self.cfg.dt).round().max(1.0) as u64
    }

    fn park(&mut self, agent: AgentId, secs: f32) {
        self.agents.state[agent as usize] = AgentState::Parked;
        let wake_at = self.totals.tick + self.secs_to_ticks(secs);
        self.wake.push(Reverse((wake_at, agent)));
    }

    fn wake_parked(&mut self) {
        let nodes = self.roads.node_count() as u32;
        while let Some(&Reverse((at, agent))) = self.wake.peek() {
            if at > self.totals.tick {
                break;
            }
            self.wake.pop();
            let a = agent as usize;
            let mut dest = self.rng.below(nodes);
            if dest == self.agents.node[a] {
                dest = (dest + 1) % nodes;
            }
            self.agents.dest[a] = dest;
            self.agents.state[a] = AgentState::AwaitingPath;
            self.path_queue.push_back(agent);
        }
    }

    fn plan_paths(&mut self) {
        for _ in 0..self.cfg.path_budget {
            let Some(agent) = self.path_queue.pop_front() else {
                break;
            };
            let a = agent as usize;
            let mut path = std::mem::take(&mut self.agents.path[a]);
            let (from, to) = (self.agents.node[a], self.agents.dest[a]);
            let found = self
                .pathfinder
                .find(&self.roads, &self.lane_delay, from, to, &mut path)
                && !path.is_empty();
            self.agents.path[a] = path;
            if found {
                self.totals.paths_found += 1;
                self.agents.path_idx[a] = 0;
                self.agents.speed[a] = 0.0;
                self.agents.state[a] = AgentState::Entering;
                self.entering.push(agent);
            } else {
                self.totals.paths_failed += 1;
                self.park(agent, RETRY_AFTER_S);
            }
        }
    }

    fn enter_lanes(&mut self) {
        let (traffic, agents) = (&mut self.traffic, &mut self.agents);
        self.entering.retain(|&agent| {
            let lane = agents.path[agent as usize][0];
            if traffic.has_room(lane, agents) {
                traffic.enter(lane, agent, agents);
                false
            } else {
                true
            }
        });
    }

    fn resolve_exits(&mut self, exits: &[LaneId]) {
        let stuck_limit = self
            .secs_to_ticks(self.cfg.stuck_limit_s)
            .min(u16::MAX as u64) as u16;
        let detour_after = self.secs_to_ticks(DETOUR_AFTER_S) as u16;
        let detour_every = self.secs_to_ticks(DETOUR_EVERY_S).max(1) as u16;
        for &lane in exits {
            let l = lane as usize;
            let len = self.roads.lane_len[l];
            // The front car reached the lane end: it moves on, arrives, or waits for room.
            let front = self.traffic.queues[l][0];
            let f = front as usize;
            if self.agents.pos[f] >= len {
                let next_idx = self.agents.path_idx[f] as usize + 1;
                if next_idx >= self.agents.path[f].len() {
                    self.arrive(lane, front);
                } else {
                    let next = self.agents.path[f][next_idx];
                    if self.traffic.has_room(next, &self.agents) {
                        self.traffic.queues[l].pop_front();
                        self.agents.path_idx[f] = next_idx as u32;
                        self.traffic.enter(next, front, &mut self.agents);
                    } else {
                        self.held.push((front, lane, true));
                    }
                }
            }

            // The first car not yet cleared may be approaching the stop line, even while cars
            // ahead of it are still crossing, so a moving queue keeps moving.
            let Some(k) = self.traffic.first_waiting(lane, &self.agents) else {
                continue;
            };
            let agent = self.traffic.queues[l][k];
            let a = agent as usize;
            if self.agents.pos[a] >= len
                || !Traffic::wants_to_ask(
                    &self.roads,
                    &self.agents,
                    &self.junctions.stop,
                    lane,
                    agent,
                )
            {
                continue; // Handled above, or not there yet.
            }
            let at_line = self.agents.pos[a] >= self.junctions.stop[l];
            let next_idx = self.agents.path_idx[a] as usize + 1;
            let Some(&next) = self.agents.path[a].get(next_idx) else {
                if at_line && k == 0 {
                    self.arrive(lane, agent);
                }
                continue;
            };
            let verdict = self.junctions.request(
                &self.roads,
                &self.traffic,
                &self.agents,
                agent,
                lane,
                next,
                self.totals.tick,
                self.cfg.dt,
            );
            match verdict {
                Verdict::Go => self.agents.cleared[a] = true,
                Verdict::Wait if at_line => self.held.push((agent, lane, false)),
                Verdict::Wait | Verdict::Red => {}
            }
        }

        // Wait times only grow once every car has decided, so cars compare them as they were
        // at the start of the tick, whatever order their lanes are visited in. Otherwise two
        // cars that each think the other waited longer can wait for each other forever.
        let held = std::mem::take(&mut self.held);
        for &(agent, lane, at_end) in &held {
            let a = agent as usize;
            let next = self.agents.path[a][self.agents.path_idx[a] as usize + 1];
            self.agents.stuck_ticks[a] += 1;
            let waited = self.agents.stuck_ticks[a];
            if !at_end
                && waited >= detour_after
                && (waited - detour_after).is_multiple_of(detour_every)
                && !self
                    .junctions
                    .exit_clear(&self.roads, &self.traffic, &self.agents, next)
            {
                self.try_detour(agent, lane, next);
            }
            if waited >= stuck_limit {
                let queue = &mut self.traffic.queues[lane as usize];
                let k = queue
                    .iter()
                    .position(|&b| b == agent)
                    .expect("held car is queued");
                queue.remove(k);
                self.agents.node[a] = self.roads.lane_to[lane as usize];
                self.agents.cleared[a] = false;
                self.totals.despawned_stuck += 1;
                self.park(agent, RETRY_AFTER_S);
            }
        }
        self.held = held;
        self.held.clear();
    }
}

impl World {
    /// Ends the trip of `agent`, the front car on `lane`.
    fn arrive(&mut self, lane: LaneId, agent: AgentId) {
        debug_assert_eq!(self.traffic.queues[lane as usize].front(), Some(&agent));
        self.traffic.queues[lane as usize].pop_front();
        let a = agent as usize;
        self.agents.node[a] = self.agents.dest[a];
        self.totals.trips_completed += 1;
        let dwell = self.rng.range(self.cfg.dwell_min_s, self.cfg.dwell_max_s);
        self.park(agent, dwell);
    }

    /// Reroutes a car at `lane`'s stop line around its blocked `next` lane, keeping its place in
    /// the queue. Rings of full lanes need every car to want the lane ahead; one turning off
    /// breaks the ring. The search is local (around the block, back onto the old route), so it
    /// costs a few dozen nodes instead of a city-wide search. Keeps the old route if nothing
    /// nearby works.
    fn try_detour(&mut self, agent: AgentId, lane: LaneId, next: LaneId) {
        let a = agent as usize;
        let idx = self.agents.path_idx[a] as usize;
        let route = &self.agents.path[a][idx + 1..];
        debug_assert_eq!(route.first(), Some(&next));
        debug_assert_eq!(
            self.roads.lane_to[lane as usize],
            self.roads.lane_from[next as usize]
        );
        let (roads, traffic, agents, junctions) =
            (&self.roads, &self.traffic, &self.agents, &self.junctions);
        let Some(j) = self.pathfinder.rejoin(
            roads,
            &self.lane_delay,
            route,
            |first| junctions.exit_clear(roads, traffic, agents, first),
            DETOUR_MAX_NODES,
            &mut self.detour,
        ) else {
            return;
        };
        self.detour.extend_from_slice(&route[j + 1..]);
        let path = &mut self.agents.path[a];
        path.truncate(idx + 1);
        path.extend_from_slice(&self.detour);
        self.totals.detours += 1;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::road::RoadNetwork;
    use crate::traffic::SPACING;

    fn small_world(seed: u64) -> World {
        let cfg = Config {
            seed,
            ..Config::default()
        };
        let mut world = World::generate(cfg, 12, 12);
        world.apply(Command::SpawnAgents { count: 2_000 });
        world
    }

    #[test]
    fn same_seed_same_city() {
        let (mut a, mut b) = (small_world(7), small_world(7));
        for _ in 0..1_500 {
            a.tick();
            b.tick();
        }
        assert_eq!(a.state_hash(), b.state_hash());
        assert_ne!(a.state_hash(), {
            let mut c = small_world(8);
            (0..1_500).for_each(|_| c.tick());
            c.state_hash()
        });
    }

    #[test]
    fn agents_complete_trips() {
        let mut world = small_world(1);
        for _ in 0..3_000 {
            world.tick();
        }
        let s = world.stats();
        assert!(s.trips_completed > 500, "{s:?}");
        assert_eq!(
            s.parked + s.awaiting_path + s.entering + s.driving,
            s.agents
        );
    }

    #[test]
    fn build_road_snaps_to_existing_nodes() {
        let mut world = World::generate(Config::default(), 3, 3);
        let nodes = world.roads.node_count();
        let (x0, y0) = (world.roads.node_x[0], world.roads.node_y[0]);
        let (x8, y8) = (world.roads.node_x[8], world.roads.node_y[8]);
        assert!(world.apply(Command::BuildRoad {
            ax: x0 + 3.0,
            ay: y0,
            bx: x8,
            by: y8 - 3.0
        }));
        assert_eq!(world.roads.node_count(), nodes, "both ends should snap");
        assert!(
            !world.apply(Command::BuildRoad {
                ax: x0,
                ay: y0,
                bx: x8,
                by: y8
            }),
            "duplicate"
        );
    }

    /// A four-way junction at the origin with 100 m arms east, north, west and south. Arm `k`
    /// owns lane `2k` (out of the junction) and `2k + 1` (into it).
    fn crossroads(ew_speed: f32) -> World {
        let mut net = RoadNetwork::default();
        let centre = net.add_node(0.0, 0.0);
        for k in 0..4 {
            let angle = k as f32 * std::f32::consts::FRAC_PI_2;
            let arm = net.add_node(100.0 * angle.cos(), 100.0 * angle.sin());
            net.add_road(
                centre,
                arm,
                if k % 2 == 0 { ew_speed } else { STREET_SPEED },
            );
        }
        World::new(Config::default(), net, Rng::new(1))
    }

    /// Puts a car at the stop line of `from`, about to drive on to `to`, having waited `waited`
    /// ticks already.
    fn wait_at_line(world: &mut World, from: LaneId, to: LaneId, waited: u16) -> AgentId {
        let agent = world.agents.push(world.roads.lane_from[from as usize]);
        let a = agent as usize;
        world.agents.dest[a] = world.roads.lane_to[to as usize];
        world.agents.path[a] = vec![from, to];
        world.traffic.enter(from, agent, &mut world.agents);
        world.agents.pos[a] = world.junctions.stop[from as usize];
        world.agents.stuck_ticks[a] = waited;
        agent
    }

    #[test]
    fn waiting_cars_never_wait_for_each_other() {
        // Long waits (past patience) a tick or two apart, compared in lane order. Counting a
        // car's wait as soon as its lane was visited once made pairs of cars that each thought
        // the other had waited longer, and both sat until the stuck limit.
        const LONG: u16 = 300;
        let check = |cars: &[(LaneId, LaneId, u16)]| {
            let mut world = crossroads(STREET_SPEED);
            for &(from, to, waited) in cars {
                wait_at_line(&mut world, from, to, waited);
            }
            for _ in 0..600 {
                world.tick();
            }
            let s = world.stats();
            assert_eq!(
                (s.trips_completed, s.despawned_stuck),
                (cars.len() as u64, 0),
                "{cars:?}"
            );
        };
        // Lanes: arm k (east, north, west, south) enters on 2k + 1 and leaves on 2k.
        let (in_e, in_s, out_w) = (1, 7, 4);
        for offset in 0..5 {
            // Merging into one exit: the car from the south yields to the one on its right.
            check(&[(in_e, out_w, LONG), (in_s, out_w, LONG - 2 + offset)]);
        }
        for code in 0..81 {
            // Four cars going straight on, each yielding to the one on its right.
            let waits = [code % 3, code / 3 % 3, code / 9 % 3, code / 27];
            let cars: Vec<_> = (0..4u32)
                .map(|k| (2 * k + 1, 2 * ((k + 2) % 4), LONG + waits[k as usize]))
                .collect();
            check(&cars);
        }
    }

    #[test]
    fn signal_turns_green_for_waiting_traffic() {
        let mut world = crossroads(crate::road::AVENUE_SPEED);
        let mut signals = Vec::new();
        world.write_signal_snapshot(&mut signals);
        assert_eq!(signals.len(), 4 * SIGNAL_STRIDE);
        // Arm k's incoming lane is 2k + 1; its straight-on exit is the opposite arm's lane.
        let roads = world.roads.clone();
        let state = |signals: &[f32], lane: LaneId| {
            let heading = roads.lane_heading(lane);
            signals
                .chunks(SIGNAL_STRIDE)
                .find(|s| (s[2] - heading).abs() < 1e-3)
                .expect("a signal head per approach")[3]
        };
        let from = (0..4)
            .map(|k| 2 * k + 1)
            .find(|&l| state(&signals, l) == 0.0)
            .expect("a red approach");
        let to = (from + 3) % 8;
        let car = wait_at_line(&mut world, from, to, 0) as usize;
        let red_ticks = (0..300)
            .take_while(|_| {
                world.tick();
                world.agents.lane[car] == from && !world.agents.cleared[car]
            })
            .count();
        // Green stays with the other pair for its minimum time, then switches on demand.
        assert!((70..150).contains(&red_ticks), "red for {red_ticks} ticks");
        assert_eq!(
            world.agents.stuck_ticks[car], 0,
            "a red light is not gridlock"
        );
        world.write_signal_snapshot(&mut signals);
        assert_eq!(state(&signals, from), 1.0, "green for the waiting car");
    }

    #[test]
    fn queue_discharges_without_stopping_each_car() {
        // Ten cars queued at an empty junction, all going straight on. Once the queue is moving,
        // nobody should stop again. Letting only the front car ask, and only once the car ahead
        // had left the junction, made every car stop at the line and start from rest.
        let mut world = crossroads(STREET_SPEED);
        let (from, to) = (5, 0); // West arm in, east arm out.
        let cars: Vec<_> = (0..10)
            .map(|k| {
                let car = wait_at_line(&mut world, from, to, 0);
                world.agents.pos[car as usize] -= k as f32 * SPACING;
                car
            })
            .collect();
        let mut started = [false; 10];
        let ticks = (0..1_000)
            .take_while(|_| {
                world.tick();
                for (k, &c) in cars.iter().enumerate() {
                    let c = c as usize;
                    if world.agents.lane[c] != from {
                        continue;
                    }
                    let speed = world.agents.speed[c];
                    assert!(
                        !started[k] || speed > 1.0,
                        "car {k} stopped again at {} m",
                        world.agents.pos[c]
                    );
                    started[k] |= speed > 2.0;
                }
                cars.iter().any(|&c| world.agents.lane[c as usize] == from)
            })
            .count();
        let secs = ticks as f32 * world.cfg.dt;
        assert!(secs < 15.0, "queue took {secs} s to clear");
    }

    #[test]
    fn dense_traffic_does_not_gridlock() {
        // 6,000 cars fill most of this small town's road space. Before intersection rules it
        // lost hundreds of them to the stuck limit in these ten minutes.
        let mut world = small_world(1);
        world.apply(Command::SpawnAgents { count: 4_000 });
        for _ in 0..6_000 {
            world.tick();
        }
        let s = world.stats();
        assert!(s.trips_completed > 1_500, "{s:?}");
        assert!(s.despawned_stuck <= 5, "{s:?}");
    }
}
