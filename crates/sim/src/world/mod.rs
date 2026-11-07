//! The world: owns all sim state and advances it one fixed step at a time.

use serde::{Deserialize, Serialize};
mod closure;
mod demand;
mod demolish;
mod fire;
mod inspect;
mod money;
mod persist;
mod pollution;
mod power;
mod residents;
mod roads;
mod snapshot;
mod trips;
mod walking;
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
use crate::pollution::Pollution;
use crate::power::{PlantKind, Power};
use crate::rng::Rng;
use crate::road::{LaneId, RoadClass, RoadNetwork};
use crate::traffic::Traffic;
use crate::zone::{Zone, ZoneGrid};

pub use demand::Demand;
pub use fire::{FIRE_STRIDE, FireStats, STATION_STRIDE};
pub use inspect::{BuildingInfo, CitizenInfo, Place};
pub use money::{GRANT, JOB_TAX, Money, RESIDENT_TAX, START_FUNDS};
pub use persist::{SAVE_VERSION, SaveError};
pub use power::{PLANT_STRIDE, POWER_BUILDING_STRIDE, PowerStats};

/// Floats per agent in `write_agent_snapshot`.
pub const SNAPSHOT_STRIDE: usize = 5;
/// Floats per signalised approach in `write_signal_snapshot`.
pub const SIGNAL_STRIDE: usize = 4;
/// Expected delay each queued car adds to a lane, seconds. Feeds route choice so traffic
/// spreads off backed-up avenues.
const QUEUE_DELAY_S: f32 = 1.0;

#[derive(Clone, Debug, Serialize, Deserialize)]
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
    /// Builds every due lot whatever the demand (ADR 0025) and powers every building (ADR
    /// 0041). For tests and benchmarks that need full blocks; the game leaves it off.
    pub ignore_demand: bool,
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
            ignore_demand: false,
        }
    }
}

/// Player actions. Everything that changes the world from outside goes through `World::apply`,
/// so the seed plus a command log is enough to replay a session (`crate::replay`, ADR 0034).
#[derive(Clone, Debug, PartialEq)]
pub enum Command {
    SpawnAgents {
        count: u32,
    },
    /// Builds a road of `class`. Roads it follows take that class (ADR 0048).
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
    /// Removes the fire station or plant under the point, else the nearest line in reach,
    /// else the building under the point (ADR 0045), else ruins (ADR 0053), else the nearest
    /// road.
    DeleteRoad {
        x: f32,
        y: f32,
    },
    /// Places a power plant at the point; coal snaps beside a road and faces it (ADR 0042).
    /// Refused over roads and other plants, and for coal away from roads.
    PlacePlant {
        kind: PlantKind,
        x: f32,
        y: f32,
    },
    /// Draws a transmission line; each end snaps to a line end, plant or road in reach.
    BuildLine {
        ax: f32,
        ay: f32,
        bx: f32,
        by: f32,
    },
    /// Places a fire station beside the nearest road, facing it (ADR 0053). Refused away from
    /// roads, over roads, plants and stations, and when the city cannot pay.
    PlaceStation {
        x: f32,
        y: f32,
    },
    /// Turns this city's fires on or off (ADR 0053).
    SetFires {
        on: bool,
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

#[derive(Clone, Copy, Default, Debug, Serialize, Deserialize)]
pub struct Stats {
    pub tick: u64,
    /// Allocated travel slots, including dormant citizen reservations and departed tombstones.
    pub agents: u32,
    /// Resident individuals, including those not travelling.
    pub population: u32,
    /// Resident individuals without an assigned job. Disconnected jobs still count as jobs.
    pub unemployed: u32,
    /// `unemployed` as a percentage of `population`; 0 when nobody lives in town.
    pub unemployment_percent: f32,
    /// What the town wants built next (ADR 0025).
    pub demand: Demand,
    /// Supply and use of power (ADR 0041).
    pub power: PowerStats,
    /// Balance, taxes, grant and upkeep (ADR 0036).
    pub money: Money,
    /// Stations, trucks and fires (ADR 0053).
    pub fire: FireStats,
    /// Synthetic vehicles waiting between random trips; dormant citizen slots are excluded.
    pub parked: u32,
    pub awaiting_path: u32,
    pub entering: u32,
    pub driving: u32,
    /// Citizens on foot now (ADR 0026).
    pub walking: u32,
    /// Includes walks.
    pub trips_completed: u64,
    /// Trips finished on foot.
    pub walks_completed: u64,
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

#[derive(Serialize, Deserialize)]
pub struct World {
    pub cfg: Config,
    pub roads: RoadNetwork,
    pub zones: ZoneGrid,
    pub lots: Lots,
    pub buildings: Buildings,
    pub citizens: crate::citizen::Citizens,
    pub households: crate::citizen::Households,
    pub homes: crate::citizen::Homes,
    /// Plants and transmission lines (ADR 0041).
    pub power: Power,
    /// Grids and who is powered, derived from the rest.
    #[serde(skip)]
    power_cache: power::PowerCache,
    residents: residents::Residents,
    closures: closure::Closures,
    supply: demand::Supply,
    treasury: money::Treasury,
    /// Faces of the road network, traced again when `blocks_version` falls behind the roads.
    #[serde(skip)]
    blocks: Blocks,
    #[serde(skip)]
    blocks_version: Option<u32>,
    pub agents: Agents,
    traffic: Traffic,
    pub junctions: Junctions,
    #[serde(skip)]
    pathfinder: Pathfinder,
    path_queue: VecDeque<AgentId>,
    #[serde(skip)]
    lane_delay: Vec<f32>,
    #[serde(skip)]
    detour: Vec<LaneId>,
    /// Scratch for `resolve_exits`: front cars that stay put this tick.
    #[serde(skip)]
    held: Vec<(AgentId, LaneId, bool)>,
    entering: Vec<AgentId>,
    /// Citizens on foot, in the order their walks started.
    walking: Vec<AgentId>,
    /// Parked agents keyed by wake-up tick. Ties pop in agent-id order, so it stays deterministic.
    wake: BinaryHeap<Reverse<(u64, AgentId)>>,
    rng: Rng,
    totals: Stats,
    /// Air, ground, noise, traffic flow and land value (ADR 0037).
    pub pollution: Pollution,
    /// Fire stations, trucks, fires and ruins (ADR 0053).
    fire: fire::Fire,
}

impl World {
    pub fn new(cfg: Config, roads: RoadNetwork, rng: Rng) -> Self {
        let mut world = Self {
            residents: residents::Residents::new(cfg.seed),
            fire: fire::Fire::new(cfg.seed),
            closures: Default::default(),
            supply: Default::default(),
            treasury: Default::default(),
            citizens: Default::default(),
            households: Default::default(),
            homes: Default::default(),
            power: Power::default(),
            power_cache: Default::default(),
            cfg,
            zones: ZoneGrid::covering(roads.bounds()),
            pollution: Pollution::default(),
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
            walking: Vec::new(),
            wake: BinaryHeap::new(),
            rng,
            totals: Stats::default(),
        };
        world.pollution = Pollution::covering(&world.zones);
        world.sync_roads();
        world.ensure_power();
        world.refresh_upkeep();
        world
    }

    /// A fresh city: no roads, on a map the size of a `cols` × `rows` street grid.
    pub fn empty(cfg: Config, cols: u32, rows: u32) -> Self {
        let size = |n: u32| n.saturating_sub(1) as f32 * cfg.grid_spacing;
        let zones = ZoneGrid::covering([0.0, 0.0, size(cols), size(rows)]);
        let rng = Rng::new(cfg.seed);
        let mut world = Self::new(cfg, RoadNetwork::default(), rng);
        world.pollution = Pollution::covering(&zones);
        world.zones = zones;
        world
    }

    /// `[min_x, min_y, max_x, max_y]` of the map: the land the zone grid covers.
    pub fn bounds(&self) -> [f32; 4] {
        let [x0, y0, cell, cols, rows] = self.zones.layout();
        [x0, y0, x0 + cell * cols, y0 + cell * rows]
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
        let versions = (
            self.lots.version,
            self.zones.version,
            self.power.version,
            self.buildings.version,
            self.fire.version,
        );
        let roads = self.roads.version;
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
            Command::DeleteRoad { x, y } => {
                self.bulldoze_station(x, y)
                    || self.bulldoze_power(x, y)
                    || self.demolish(x, y)
                    || self.clear_ruin(x, y)
                    || self.delete_road(x, y)
            }
            Command::PlacePlant { kind, x, y } => self.place_plant(kind, x, y),
            Command::BuildLine { ax, ay, bx, by } => self.build_line(ax, ay, bx, by),
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
            Command::PlaceStation { x, y } => self.place_station(x, y),
            Command::SetFires { on } => self.set_fires(on),
        };
        let now = (
            self.lots.version,
            self.zones.version,
            self.power.version,
            self.buildings.version,
            self.fire.version,
        );
        if versions != now {
            self.buildings.reconcile(
                &self.lots,
                &self.zones,
                self.totals.tick,
                self.cfg.dt,
                self.cfg.seed,
            );
            self.reconcile_closures();
            self.reconcile_residents();
        }
        self.ensure_power();
        if roads != self.roads.version {
            self.refresh_upkeep();
        }
        self.ensure_fire();
        changed
    }

    pub fn tick(&mut self) {
        self.sync_roads();
        self.ensure_fire();
        self.tick_closures();
        self.ensure_power();
        self.tick_residents();
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
        self.advance_walkers();
        let exits = self.traffic.advance(
            &self.roads,
            &mut self.agents,
            &self.junctions.stop,
            self.cfg.dt,
        );
        self.resolve_exits(&exits);
        self.traffic.recycle_exits(exits);
        self.totals.tick += 1;
        self.tick_pollution();
        self.settle_money();
        self.tick_fire();
        self.grow_buildings();
        // Current at every tick boundary, so a loaded world reads the same allocation.
        self.ensure_power();
    }

    pub fn stats(&self) -> Stats {
        let mut s = self.totals;
        s.agents = self.agents.len() as u32;
        s.population = self.residents.population;
        s.unemployed = self.residents.population - self.residents.employed;
        s.demand = self.demand();
        s.power = self.power_stats();
        s.money = self.money();
        s.fire = self.fire_stats();
        if s.population > 0 {
            s.unemployment_percent = s.unemployed as f32 * 100.0 / s.population as f32;
        }
        for state in &self.agents.state {
            match state {
                AgentState::Dormant => {}
                AgentState::Parked => s.parked += 1,
                AgentState::AwaitingPath => s.awaiting_path += 1,
                AgentState::Entering => s.entering += 1,
                AgentState::Driving => s.driving += 1,
                AgentState::Walking => s.walking += 1,
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
