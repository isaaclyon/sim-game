//! The world: owns all sim state and advances it one fixed step at a time.

use std::cmp::Reverse;
use std::collections::{BinaryHeap, VecDeque};

use crate::agents::{AgentId, AgentState, Agents};
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
}

pub struct World {
    pub cfg: Config,
    pub roads: RoadNetwork,
    pub agents: Agents,
    traffic: Traffic,
    pathfinder: Pathfinder,
    path_queue: VecDeque<AgentId>,
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
            pathfinder: Pathfinder::default(),
            path_queue: VecDeque::new(),
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
        self.plan_paths();
        self.enter_lanes();
        let exits = self.traffic.advance(&self.roads, &mut self.agents, self.cfg.dt);
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
                out.extend([x, y, heading, a.speed[i] / self.roads.lane_speed[lane as usize]]);
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
            out.extend([r.node_x[a], r.node_y[a], r.node_x[b], r.node_y[b], r.lane_speed[lane]]);
        }
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
        }
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
            let Some(agent) = self.path_queue.pop_front() else { break };
            let a = agent as usize;
            let mut path = std::mem::take(&mut self.agents.path[a]);
            let (from, to) = (self.agents.node[a], self.agents.dest[a]);
            let found = self.pathfinder.find(&self.roads, from, to, &mut path) && !path.is_empty();
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
        let stuck_limit = self.secs_to_ticks(self.cfg.stuck_limit_s).min(u16::MAX as u64) as u16;
        for &lane in exits {
            let queue = &self.traffic.queues[lane as usize];
            let agent = queue[0];
            let a = agent as usize;
            let next_idx = self.agents.path_idx[a] as usize + 1;

            if next_idx >= self.agents.path[a].len() {
                self.traffic.queues[lane as usize].pop_front();
                self.agents.node[a] = self.agents.dest[a];
                self.totals.trips_completed += 1;
                let dwell = self.rng.range(self.cfg.dwell_min_s, self.cfg.dwell_max_s);
                self.park(agent, dwell);
                continue;
            }

            let next = self.agents.path[a][next_idx];
            if self.traffic.has_room(next, &self.agents) {
                self.traffic.queues[lane as usize].pop_front();
                self.agents.path_idx[a] = next_idx as u32;
                self.traffic.enter(next, agent, &mut self.agents);
            } else {
                self.agents.stuck_ticks[a] += 1;
                if self.agents.stuck_ticks[a] >= stuck_limit {
                    self.traffic.queues[lane as usize].pop_front();
                    self.agents.node[a] = self.roads.lane_to[lane as usize];
                    self.totals.despawned_stuck += 1;
                    self.park(agent, RETRY_AFTER_S);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn small_world(seed: u64) -> World {
        let cfg = Config { seed, ..Config::default() };
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
        assert_eq!(s.parked + s.awaiting_path + s.entering + s.driving, s.agents);
    }

    #[test]
    fn build_road_snaps_to_existing_nodes() {
        let mut world = World::generate(Config::default(), 3, 3);
        let nodes = world.roads.node_count();
        let (x0, y0) = (world.roads.node_x[0], world.roads.node_y[0]);
        let (x8, y8) = (world.roads.node_x[8], world.roads.node_y[8]);
        assert!(world.apply(Command::BuildRoad { ax: x0 + 3.0, ay: y0, bx: x8, by: y8 - 3.0 }));
        assert_eq!(world.roads.node_count(), nodes, "both ends should snap");
        assert!(!world.apply(Command::BuildRoad { ax: x0, ay: y0, bx: x8, by: y8 }), "duplicate");
    }
}
