//! The world: owns all sim state and advances it one fixed step at a time.

mod roads;
mod snapshot;
mod trips;
mod zoning;

use std::cmp::Reverse;
use std::collections::{BinaryHeap, VecDeque};

use crate::agents::{AgentId, AgentState, Agents};
use crate::block::Blocks;
use crate::building::Buildings;
use crate::geometry::Shape;
use crate::junction::Junctions;
use crate::lot::Lots;
use crate::path::Pathfinder;
use crate::rng::Rng;
use crate::road::{LaneId, RoadClass, RoadNetwork};
use crate::traffic::Traffic;
use crate::zone::{Zone, ZoneGrid};

/// Floats per agent in `write_agent_snapshot`.
pub const SNAPSHOT_STRIDE: usize = 4;
/// Floats per signalised approach in `write_signal_snapshot`.
pub const SIGNAL_STRIDE: usize = 4;
/// Expected delay each queued car adds to a lane, seconds. Feeds route choice so traffic
/// spreads off backed-up avenues.
const QUEUE_DELAY_S: f32 = 1.0;

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
    SpawnAgents {
        count: u32,
    },
    /// Builds a road of `class`. Where it follows an existing road, that road keeps its class.
    BuildRoad {
        ax: f32,
        ay: f32,
        bx: f32,
        by: f32,
        class: RoadClass,
    },
    BuildCurve {
        ax: f32,
        ay: f32,
        cx: f32,
        cy: f32,
        bx: f32,
        by: f32,
        class: RoadClass,
    },
    DeleteRoad {
        x: f32,
        y: f32,
    },
    /// Sets every zone cell whose centre is inside the rectangle with these corners. `Zone::None`
    /// unzones. See ADR 0014.
    PaintZone {
        ax: f32,
        ay: f32,
        bx: f32,
        by: f32,
        zone: Zone,
    },
    /// Sets every cell of the block containing `(x, y)`; outside every block it changes nothing.
    /// See ADR 0016.
    FillBlock {
        x: f32,
        y: f32,
        zone: Zone,
    },
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
    /// Nodes expanded by trip route searches (not detours), a measure of routing cost.
    pub path_nodes_expanded: u64,
    pub despawned_stuck: u64,
    /// Cars removed because a road on their remaining route was split or deleted.
    pub despawned_by_edit: u64,
    /// Cars that turned off a route blocked by a full lane.
    pub detours: u64,
}

pub struct World {
    pub cfg: Config,
    pub roads: RoadNetwork,
    pub zones: ZoneGrid,
    pub lots: Lots,
    pub buildings: Buildings,
    /// Faces of the road network, traced again when `blocks_version` falls behind the roads.
    blocks: Blocks,
    blocks_version: Option<u32>,
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
            zones: ZoneGrid::covering(roads.bounds()),
            roads,
            lots: Lots::default(),
            buildings: Buildings::default(),
            blocks: Blocks::default(),
            blocks_version: None,
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
        let versions = (self.lots.version, self.zones.version);
        let changed = match cmd {
            Command::SpawnAgents { count } => self.spawn_agents(count),
            Command::BuildRoad {
                ax,
                ay,
                bx,
                by,
                class,
            } => self.build_road(
                Shape {
                    a: (ax, ay),
                    b: (bx, by),
                    control: None,
                },
                class,
            ),
            Command::BuildCurve {
                ax,
                ay,
                cx,
                cy,
                bx,
                by,
                class,
            } => self.build_road(
                Shape {
                    a: (ax, ay),
                    b: (bx, by),
                    control: Some((cx, cy)),
                },
                class,
            ),
            Command::DeleteRoad { x, y } => self.delete_road(x, y),
            Command::PaintZone {
                ax,
                ay,
                bx,
                by,
                zone,
            } => {
                let changed = self.zones.paint(ax, ay, bx, by, zone);
                if !changed.is_empty() {
                    let rect = [ax.min(bx), ay.min(by), ax.max(bx), ay.max(by)];
                    self.after_zoning(rect, &changed);
                }
                !changed.is_empty()
            }
            Command::FillBlock { x, y, zone } => self.fill_block(x, y, zone),
        };
        if versions != (self.lots.version, self.zones.version) {
            self.buildings.reconcile(
                &self.lots,
                &self.zones,
                self.totals.tick,
                self.cfg.dt,
                self.cfg.seed,
            );
        }
        changed
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
        self.buildings
            .grow(self.totals.tick, &self.lots, &self.roads, self.cfg.seed);
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

    fn sync_roads(&mut self) {
        self.roads.rebuild_adjacency();
        self.traffic.ensure_lanes(self.roads.lane_count());
        self.junctions.sync(&self.roads);
    }
}

#[cfg(test)]
mod tests;
