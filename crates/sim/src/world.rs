//! The world: owns all sim state and advances it one fixed step at a time.

use std::cmp::Reverse;
use std::collections::{BTreeSet, BinaryHeap, VecDeque};

use crate::agents::{AgentId, AgentState, Agents};
use crate::block::{self, Blocks, LOT_REACH, MIN_DEEP_CELLS, Rect};
use crate::building::Buildings;
use crate::geometry::Shape;
use crate::junction::{Junctions, Verdict};
use crate::lot::Lots;
use crate::path::Pathfinder;
use crate::rng::Rng;
use crate::road::{LaneId, NodeId, RoadClass, RoadId, RoadNetwork};
use crate::snap::{self, Join};
use crate::traffic::Traffic;
use crate::zone::{Zone, ZoneGrid};

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
/// Rounds of alley cuts one zoning command may make; each round can halve a deep block.
const ALLEY_ROUNDS: u32 = 6;

/// Which blocks a lot update looks at, by bounding box.
enum Scope<'a> {
    /// Blocks meeting this area: after a player's edit.
    Near(Rect),
    /// Blocks inside one of these blocks' bounds: the pieces of blocks just divided by alleys.
    Inside(&'a [Rect]),
}

impl Scope<'_> {
    fn has(&self, block: Rect) -> bool {
        match self {
            Scope::Near(area) => block::intersects(block, *area),
            Scope::Inside(blocks) => blocks.iter().any(|&b| block::contains_rect(b, block, 0.5)),
        }
    }
}

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
                let heading = self.roads.lane_heading(lane, a.pos[i]);
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

    /// Writes `[ax, ay, bx, by, class]` per active rendered segment (many per curve); class is
    /// 0 = street, 1 = main road, 2 = alley. Packed in road ID order, not indexed by road ID.
    pub fn write_road_segments(&self, out: &mut Vec<f32>) {
        let r = &self.roads;
        out.clear();
        out.reserve(r.road_count() * 5);
        for road in r.active_roads() {
            let class = r.road_class(road) as u8 as f32;
            for (_, a, _, b) in r.shape(road).segments() {
                out.extend([a.0, a.1, b.0, b.1, class]);
            }
        }
    }

    /// Writes `[x, y]` per centreline sample of the bulldoze target, or nothing on a miss.
    pub fn write_bulldoze_preview(&self, x: f32, y: f32, out: &mut Vec<f32>) {
        out.clear();
        if let Some(road) = snap::bulldoze_target(&self.roads, x, y) {
            let shape = self.roads.shape(road);
            out.extend([shape.a.0, shape.a.1]);
            for (_, _, _, p) in shape.segments() {
                out.extend([p.0, p.1]);
            }
        }
    }

    /// Writes `[x, y, join]` for where a road started at `(x, y)` would start, as in
    /// `write_road_preview`. Changes nothing.
    pub fn write_snap_preview(&self, x: f32, y: f32, out: &mut Vec<f32>) {
        out.clear();
        let s = snap::snap_end(&self.roads, x, y);
        out.extend([s.x, s.y, join_code(s.join)]);
    }

    /// Writes `[x, y, join]` per stop of the road `BuildRoad` would build between these points,
    /// in order; nothing if it would be refused. Join is 0 = new end, 1 = existing node,
    /// 2 = splits an existing road, -1 = interior sample when following an existing curve.
    /// Changes nothing, so the frontend can call it every frame.
    pub fn write_road_preview(&self, ax: f32, ay: f32, bx: f32, by: f32, out: &mut Vec<f32>) {
        out.clear();
        self.write_build_preview(
            Shape {
                a: (ax, ay),
                b: (bx, by),
                control: None,
            },
            out,
        );
    }

    /// Same as road preview, with join -1 for interior curve samples (not junctions).
    #[allow(clippy::too_many_arguments)] // Mirrors the coordinate-only WASM command boundary.
    pub fn write_curve_preview(
        &self,
        ax: f32,
        ay: f32,
        cx: f32,
        cy: f32,
        bx: f32,
        by: f32,
        out: &mut Vec<f32>,
    ) {
        self.write_build_preview(
            Shape {
                a: (ax, ay),
                b: (bx, by),
                control: Some((cx, cy)),
            },
            out,
        );
    }

    fn write_build_preview(&self, shape: Shape, out: &mut Vec<f32>) {
        out.clear();
        let Some(plan) = snap::plan(&self.roads, shape) else {
            return;
        };
        let first = plan.stops[0];
        out.extend([first.x, first.y, join_code(first.join)]);
        for (i, leg) in plan.legs.iter().enumerate() {
            for (_, _, t, p) in leg.segments() {
                let join = if t == 1.0 {
                    join_code(plan.stops[i + 1].join)
                } else {
                    -1.0
                };
                out.extend([p.0, p.1, join]);
            }
        }
    }

    /// Writes `[x, y]` per outline sample of the block containing `(x, y)`, or nothing outside
    /// every block. Changes no world state; it only refreshes the cached block outlines.
    pub fn write_block_preview(&mut self, x: f32, y: f32, out: &mut Vec<f32>) {
        out.clear();
        if !x.is_finite() || !y.is_finite() {
            return;
        }
        self.refresh_blocks();
        if let Some(b) = self.blocks.at((x, y)) {
            out.extend(self.blocks.outline(b).iter().flat_map(|p| [p.0, p.1]));
        }
    }

    /// Writes `[n, x0, y0, …]` per lot: its outline's point count, then its points.
    pub fn write_lot_outlines(&self, out: &mut Vec<f32>) {
        self.lots.write_outlines(out);
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
        mix(self.roads.lane_count() as u64);
        mix(self.roads.road_count() as u64);
        for road in self.roads.active_roads() {
            mix(road as u64);
        }
        let a = &self.agents;
        for i in 0..a.len() {
            mix(a.state[i] as u64);
            mix(a.lane[i] as u64);
            mix(a.pos[i].to_bits() as u64);
            mix(a.dest[i] as u64);
            mix(a.cleared[i] as u64);
        }
        self.junctions.hash_into(&mut mix);
        self.buildings.hash_into(&mut mix);
        for chunk in self.zones.cells.chunks(8) {
            let mut word = [0u8; 8];
            word[..chunk.len()].copy_from_slice(chunk);
            mix(u64::from_le_bytes(word));
        }
        mix(self.lots.len() as u64);
        for lot in self.lots.active_lots() {
            mix(lot as u64);
            for p in self.lots.outline(lot) {
                mix(((p.0.to_bits() as u64) << 32) | p.1.to_bits() as u64);
            }
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

    /// Builds a road of `class` along `snap::plan`: joining nodes near its ends, and splitting
    /// the roads it crosses or ends beside so they meet at a junction.
    fn build_road(&mut self, shape: Shape, class: RoadClass) -> bool {
        let mut split = Vec::new();
        let Some(rect) = self.add_road(shape, class, &mut split) else {
            return false;
        };
        self.sync_roads();
        self.remove_cars_using(&split);
        self.update_lots(Scope::Near(rect), true);
        true
    }

    /// Adds the roads `snap::plan` makes of `shape`, pushing the roads it splits onto `split`.
    /// Returns the area it changed. The caller syncs the network and removes affected cars.
    fn add_road(
        &mut self,
        shape: Shape,
        class: RoadClass,
        split: &mut Vec<RoadId>,
    ) -> Option<Rect> {
        let plan = snap::plan(&self.roads, shape)?;
        let nodes: Vec<NodeId> = plan
            .stops
            .iter()
            .map(|s| match s.join {
                Join::Node(n) => n,
                Join::New => self.roads.add_node(s.x, s.y),
                Join::Split(road) => {
                    // The plan splits each road at most once, so `road` still has its old ends.
                    let n = self.roads.add_node(s.x, s.y);
                    let rest = self.roads.split_road(road, n);
                    self.junctions.remap_split(road, rest);
                    split.push(road);
                    n
                }
            })
            .collect();
        let mut rect = block::shape_rect(plan.legs[0]);
        for (leg, shape) in nodes.windows(2).zip(plan.legs) {
            rect = block::union(rect, block::shape_rect(shape));
            // False where the plan follows an existing road, which is kept as it is.
            self.roads.add_shape(leg[0], leg[1], shape.control, class);
        }
        Some(rect)
    }

    fn delete_road(&mut self, x: f32, y: f32) -> bool {
        let Some(road) = snap::bulldoze_target(&self.roads, x, y) else {
            return false;
        };
        self.roads.remove_road(road);
        self.remove_cars_using(&[road]);
        self.sync_roads();
        self.update_lots(Scope::Near(block::shape_rect(self.roads.shape(road))), true);
        true
    }

    /// Sets the cells of the block containing `(x, y)`. See ADR 0016.
    fn fill_block(&mut self, x: f32, y: f32, zone: Zone) -> bool {
        if !x.is_finite() || !y.is_finite() {
            return false;
        }
        self.refresh_blocks();
        let Some(b) = self.blocks.at((x, y)) else {
            return false;
        };
        let cells: Vec<u32> = self
            .blocks
            .cells(b, &self.zones)
            .iter()
            .map(|c| c.index)
            .collect();
        let changed = self.zones.set(&cells, zone);
        if changed.is_empty() {
            return false;
        }
        self.after_zoning(self.blocks.rect(b), &changed);
        true
    }

    /// After a zoning command changed cells: alleys for deep blocks, then lots for newly zoned
    /// blocks. `changed` holds the cells whose zone changed, ascending; `rect` bounds them.
    fn after_zoning(&mut self, rect: Rect, changed: &[u32]) {
        let (divided, split) = self.add_alleys(rect, changed);
        if !divided.is_empty() {
            // Lots across the street from an alley keep their ids; the road they front was
            // only split, so they move to the piece in front of them.
            self.refresh_blocks();
            for lot in self.lots.active_lots().collect::<Vec<_>>() {
                let l = lot as usize;
                if split.contains(&self.lots.road[l])
                    && let Some(road) = self
                        .blocks
                        .road_at((self.lots.front_x[l], self.lots.front_y[l]))
                {
                    self.lots.road[l] = road;
                }
            }
            self.update_lots(Scope::Inside(&divided), true);
        }
        self.update_lots(Scope::Near(rect), false);
    }

    /// Cuts blocks near `rect` that have enough zoned cells out of a road's reach, some of them
    /// just changed, with alleys or connector streets; then the pieces, for a few rounds.
    /// Returns the bounds of the blocks it divided and the roads it split.
    fn add_alleys(&mut self, rect: Rect, changed: &[u32]) -> (Vec<Rect>, Vec<RoadId>) {
        let mut divided = Vec::new();
        let mut split = Vec::new();
        let mut cut: Vec<Rect> = Vec::new();
        for round in 0..ALLEY_ROUNDS {
            self.refresh_blocks();
            let mut plans = Vec::new();
            for b in 0..self.blocks.len() {
                let br = self.blocks.rect(b);
                let eligible = if round == 0 {
                    block::intersects(br, rect)
                } else {
                    cut.iter().any(|&r| block::contains_rect(r, br, 0.5))
                };
                if !eligible {
                    continue;
                }
                let cells = self.blocks.cells(b, &self.zones);
                let deep: Vec<u32> = cells
                    .iter()
                    .filter(|c| c.dist > LOT_REACH && self.zones.zone(c.index) != Zone::None)
                    .map(|c| c.index)
                    .collect();
                if deep.len() < MIN_DEEP_CELLS
                    || (round == 0 && !deep.iter().any(|c| changed.binary_search(c).is_ok()))
                {
                    continue;
                }
                plans.push((br, self.blocks.cuts(b, &cells, &self.zones)));
            }
            cut.clear();
            for (br, options) in plans {
                for (shape, alley) in options {
                    let class = if alley {
                        RoadClass::Alley
                    } else {
                        RoadClass::Street
                    };
                    if self.add_road(shape, class, &mut split).is_some() {
                        if round == 0 {
                            divided.push(br);
                        }
                        cut.push(br);
                        break;
                    }
                }
            }
            if cut.is_empty() {
                break;
            }
        }
        if !divided.is_empty() {
            self.sync_roads();
            self.remove_cars_using(&split);
        }
        (divided, split)
    }

    /// Recuts lots in the blocks of `scope` (ADR 0016, ADR 0018). With `retire`, after a road
    /// edit: lots there that no longer match are retired. Either way, new lots are added where
    /// at least half their cells are zoned and no lot holds them yet.
    fn update_lots(&mut self, scope: Scope, retire: bool) {
        if self.lots.active_lots().next().is_none() && !self.zones.any_zoned() {
            return;
        }
        self.refresh_blocks();
        let old = if retire {
            self.lots.active_in(|r| scope.has(r))
        } else {
            Vec::new()
        };
        let mut kept = BTreeSet::new();
        let mut plans = Vec::new();
        for b in 0..self.blocks.len() {
            let br = self.blocks.rect(b);
            if !scope.has(br) {
                continue;
            }
            let cells = self.blocks.cells(b, &self.zones);
            let has_lots = cells.iter().any(|c| self.lots.owner(c.index).is_some());
            if !has_lots && cells.iter().all(|c| self.zones.zone(c.index) == Zone::None) {
                continue;
            }
            for plan in self.blocks.cut(b, &self.zones) {
                match self.lots.matching(&plan) {
                    Some(lot) => {
                        // Its block may have grown or shrunk around it.
                        self.lots.set_block(lot, br);
                        kept.insert(lot);
                    }
                    None => {
                        let zoned = plan
                            .cells
                            .iter()
                            .filter(|&&c| self.zones.zone(c) != Zone::None)
                            .count();
                        if zoned > 0 && 2 * zoned >= plan.cells.len() {
                            plans.push((plan, br));
                        }
                    }
                }
            }
        }
        let mut changed = false;
        for lot in old {
            if !kept.contains(&lot) {
                self.lots.retire(lot);
                changed = true;
            }
        }
        for (plan, br) in plans {
            changed |= self.lots.push(plan, br, self.zones.cells.len());
        }
        if changed {
            self.lots.version = self.lots.version.wrapping_add(1);
        }
    }

    /// Traces the road network's blocks again if roads changed since the last time.
    fn refresh_blocks(&mut self) {
        if self.blocks_version == Some(self.roads.version) {
            return;
        }
        self.roads.rebuild_adjacency();
        self.blocks = Blocks::find(&self.roads);
        self.blocks_version = Some(self.roads.version);
    }

    /// Removes cars whose remaining route uses a lane of `roads`, just split or deleted.
    /// They wait and start a new trip, like a stuck despawn. Junction reservations are left
    /// to expire; both splits and tombstones keep the headings those reservations use.
    fn remove_cars_using(&mut self, roads: &[RoadId]) {
        if roads.is_empty() {
            return;
        }
        let uses = |lanes: &[LaneId]| lanes.iter().any(|l| roads.contains(&(l / 2)));
        for agent in 0..self.agents.len() as AgentId {
            let a = agent as usize;
            let state = self.agents.state[a];
            // Only cars with a current route; a parked car's old route is stale.
            if !matches!(state, AgentState::Driving | AgentState::Entering)
                || !uses(&self.agents.path[a][self.agents.path_idx[a] as usize..])
            {
                continue;
            }
            if state == AgentState::Driving {
                let lane = self.agents.lane[a] as usize;
                self.traffic.queues[lane].retain(|&b| b != agent);
                self.agents.node[a] = self.roads.lane_to[lane];
                self.agents.cleared[a] = false;
            }
            self.totals.despawned_by_edit += 1;
            self.park(agent, RETRY_AFTER_S);
        }
        let state = &self.agents.state;
        self.entering
            .retain(|&agent| state[agent as usize] == AgentState::Entering);
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
            self.totals.path_nodes_expanded += u64::from(self.pathfinder.last_expanded);
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

/// How a preview stop joins the network, as the frontend reads it.
fn join_code(join: Join) -> f32 {
    match join {
        Join::New => 0.0,
        Join::Node(_) => 1.0,
        Join::Split(_) => 2.0,
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
    fn painting_zones_is_hashed_and_survives_road_edits() {
        let mut world = roads(&[(0.0, 0.0, 100.0, 0.0), (0.0, 100.0, 100.0, 100.0)]);
        let before = world.state_hash();
        let paint = |zone| Command::PaintZone {
            ax: 0.0,
            ay: 0.0,
            bx: 100.0,
            by: 100.0,
            zone,
        };
        assert!(world.apply(paint(Zone::ResidentialHigh)));
        assert_ne!(world.state_hash(), before);
        assert!(!world.apply(paint(Zone::ResidentialHigh)));
        // Zones may cover roads; building or bulldozing a road leaves them alone.
        assert!(build(&mut world, 50.0, 0.0, 50.0, 100.0));
        assert!(world.apply(Command::DeleteRoad { x: 20.0, y: 0.0 }));
        assert_eq!(world.zones.zone_at(50.0, 0.0), Some(Zone::ResidentialHigh));
        assert_eq!(world.zones.zone_at(20.0, 50.0), Some(Zone::ResidentialHigh));
        assert!(world.apply(paint(Zone::None)));
        assert_eq!(world.zones.zone_at(20.0, 50.0), Some(Zone::None));
    }

    #[test]
    fn bulldoze_keeps_ids_and_removes_routing_and_geometry() {
        let mut world = roads(&[(0.0, 0.0, 100.0, 0.0), (0.0, 100.0, 100.0, 100.0)]);
        let before = world.state_hash();
        let mut preview = Vec::new();
        world.write_bulldoze_preview(50.0, 4.0, &mut preview);
        assert_eq!(preview, [0.0, 0.0, 100.0, 0.0]);
        assert_eq!(world.state_hash(), before);
        assert!(world.apply(Command::DeleteRoad { x: 50.0, y: 4.0 }));
        assert_ne!(world.state_hash(), before);
        let version = world.roads.version;
        assert!(!world.apply(Command::DeleteRoad { x: 50.0, y: 4.0 }));
        assert!(!world.apply(Command::DeleteRoad {
            x: f32::NAN,
            y: 0.0
        }));
        assert_eq!(world.roads.version, version);
        assert_eq!(world.roads.road_count(), 1);
        assert_eq!(world.roads.lane_count(), 4);
        assert_eq!(world.roads.road_ends(1), (2, 3));
        assert!(world.roads.out_lanes(0).is_empty());
        assert!(world.roads.out_lanes(1).is_empty());
        let mut path = Vec::new();
        assert!(!Pathfinder::default().find(&world.roads, &[], 0, 1, &mut path));
        world.write_road_segments(&mut preview);
        assert_eq!(preview, [0.0, 100.0, 100.0, 100.0, 0.0]);
        // Deleted geometry neither blocks rebuilding nor gets split by a crossing.
        assert!(build(&mut world, 50.0, -100.0, 50.0, 50.0));
        assert_eq!(world.roads.road_count(), 2);
        assert!(build(&mut world, 0.0, 0.0, 100.0, 0.0));
        assert!(world.roads.active_roads().all(|r| r != 0));
    }

    #[test]
    fn bulldoze_parks_affected_routes_and_updates_signals_while_paused() {
        let mut world = crossroads(RoadClass::Main);
        let on_deleted = wait_at_line(&mut world, 1, 2, 0) as usize;
        let using_later = wait_at_line(&mut world, 3, 0, 0) as usize;
        let unaffected = wait_at_line(&mut world, 5, 6, 0) as usize;
        let entering = world.agents.push(0) as usize;
        world.agents.path[entering] = vec![0];
        world.agents.state[entering] = AgentState::Entering;
        world.entering.push(entering as AgentId);
        assert!(world.apply(Command::DeleteRoad { x: 50.0, y: 0.0 }));
        assert_eq!(world.stats().despawned_by_edit, 3);
        for a in [on_deleted, using_later, entering] {
            assert_eq!(world.agents.state[a], AgentState::Parked);
            assert!(
                world
                    .traffic
                    .queues
                    .iter()
                    .all(|q| !q.contains(&(a as AgentId)))
            );
        }
        assert!(world.entering.is_empty());
        assert_eq!(world.agents.state[unaffected], AgentState::Driving);
        let mut signals = Vec::new();
        world.write_signal_snapshot(&mut signals);
        assert!(signals.is_empty(), "three arms no longer need signals");
        for _ in 0..600 {
            world.tick();
        }
        assert!(world.stats().trips_completed > 0);
    }

    #[test]
    fn bulldoze_picking_has_bounded_reach_and_stable_ties() {
        let mut world = roads(&[(0.0, 0.0, 100.0, 0.0), (0.0, 30.0, 100.0, 30.0)]);
        assert_eq!(snap::bulldoze_target(&world.roads, 50.0, 15.0), Some(0));
        assert_eq!(snap::bulldoze_target(&world.roads, -16.0, 0.0), None);
        assert_eq!(
            snap::bulldoze_target(&world.roads, 50.0, f32::INFINITY),
            None
        );
        assert!(world.roads.remove_road(0));
        let version = world.roads.version;
        assert!(!world.roads.remove_road(0));
        assert!(!world.roads.remove_road(u32::MAX));
        assert_eq!(world.roads.version, version);
        assert_eq!(snap::bulldoze_target(&world.roads, 50.0, 15.0), Some(1));
        let mid = world.roads.add_node(50.0, 30.0);
        let rest = world.roads.split_road(1, mid);
        assert_eq!(
            rest, 2,
            "splitting uses the next slot, not the active count"
        );
        assert_eq!(world.roads.road_ends(rest), (mid, 3));
    }

    #[test]
    fn bulldoze_every_road_is_safe_and_deterministic() {
        let run = || {
            let mut world = roads(&[(0.0, 0.0, 100.0, 0.0)]);
            world.apply(Command::SpawnAgents { count: 50 });
            for _ in 0..300 {
                world.tick();
            }
            assert!(world.apply(Command::DeleteRoad { x: 50.0, y: 0.0 }));
            for _ in 0..600 {
                world.tick();
            }
            assert_eq!(world.stats().driving, 0);
            assert!(!world.apply(Command::SpawnAgents { count: 1 }));
            assert!(build(&mut world, 0.0, 0.0, 100.0, 0.0));
            assert_eq!(world.roads.active_roads().collect::<Vec<_>>(), [1]);
            for _ in 0..600 {
                world.tick();
            }
            assert!(world.stats().trips_completed > 0);
            world.state_hash()
        };
        assert_eq!(run(), run());
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
        let mut world = roads(&[(0.0, 0.0, 100.0, 0.0), (0.0, 200.0, 100.0, 200.0)]);
        assert!(build(&mut world, 3.0, 0.0, 100.0, 197.0));
        assert_eq!(world.roads.node_count(), 4, "both ends should snap");
        assert_eq!(world.roads.road_count(), 3);
        assert!(!build(&mut world, 0.0, 0.0, 100.0, 200.0), "duplicate");
    }

    /// A world with one straight two-way road per `(ax, ay, bx, by)`, each with its own nodes.
    fn roads(segments: &[(f32, f32, f32, f32)]) -> World {
        let mut net = RoadNetwork::default();
        for &(ax, ay, bx, by) in segments {
            let (a, b) = (net.add_node(ax, ay), net.add_node(bx, by));
            net.add_road(a, b, RoadClass::Street);
        }
        World::new(Config::default(), net, Rng::new(1))
    }

    fn build(world: &mut World, ax: f32, ay: f32, bx: f32, by: f32) -> bool {
        world.apply(Command::BuildRoad {
            ax,
            ay,
            bx,
            by,
            class: RoadClass::Street,
        })
    }

    fn node_at(world: &World, x: f32, y: f32) -> NodeId {
        world
            .roads
            .nearest_node(x, y, 0.5)
            .unwrap_or_else(|| panic!("no node at ({x}, {y})"))
    }

    #[test]
    fn main_roads_keep_their_class_when_split() {
        let mut world = roads(&[]);
        assert!(world.apply(Command::BuildRoad {
            ax: -100.0,
            ay: 0.0,
            bx: 100.0,
            by: 0.0,
            class: RoadClass::Main,
        }));
        assert!(build(&mut world, 0.0, -100.0, 0.0, 100.0));
        let r = &world.roads;
        let classes: Vec<_> = r.active_roads().map(|road| r.road_class(road)).collect();
        assert_eq!(
            classes,
            [
                RoadClass::Main,
                RoadClass::Main,
                RoadClass::Street,
                RoadClass::Street
            ]
        );
        assert_eq!(r.lane_speed[0], crate::road::MAIN_SPEED);
        let mut segments = Vec::new();
        world.write_road_segments(&mut segments);
        let snapshot: Vec<f32> = segments.chunks(5).map(|s| s[4]).collect();
        assert_eq!(snapshot, [1.0, 1.0, 0.0, 0.0], "class per segment");
        // Following an existing road keeps it as it is.
        assert!(!world.apply(Command::BuildRoad {
            ax: -100.0,
            ay: 0.0,
            bx: 0.0,
            by: 0.0,
            class: RoadClass::Street,
        }));
        assert_eq!(world.roads.road_class(0), RoadClass::Main);
    }

    #[test]
    fn crossing_roads_meet_at_a_new_junction() {
        let mut world = roads(&[(-100.0, 0.0, 100.0, 0.0)]);
        assert!(build(&mut world, 0.0, -100.0, 0.0, 100.0));
        assert_eq!(world.roads.node_count(), 5);
        assert_eq!(world.roads.road_count(), 4);
        let centre = node_at(&world, 0.0, 0.0);
        assert_eq!(world.roads.out_lanes(centre).len(), 4);
        // The old road's ends are now reachable from the new road's ends.
        let (west, north) = (node_at(&world, -100.0, 0.0), node_at(&world, 0.0, 100.0));
        let mut path = Vec::new();
        assert!(Pathfinder::default().find(&world.roads, &[], west, north, &mut path));
        assert_eq!(path.len(), 2);
    }

    #[test]
    fn road_ending_beside_a_road_makes_a_t_junction() {
        let mut world = roads(&[(-100.0, 0.0, 100.0, 0.0)]);
        assert!(build(&mut world, 0.0, -100.0, 0.0, -6.0));
        assert_eq!(world.roads.node_count(), 4);
        assert_eq!(world.roads.road_count(), 3);
        assert_eq!(world.roads.out_lanes(node_at(&world, 0.0, 0.0)).len(), 3);
    }

    #[test]
    fn crossing_near_a_junction_joins_it() {
        let mut world = roads(&[(0.0, 0.0, 100.0, 0.0)]);
        assert!(build(&mut world, 5.0, -50.0, 5.0, 50.0));
        assert_eq!(world.roads.node_count(), 4, "no node 5 m from the end");
        assert_eq!(world.roads.road_count(), 3);
        assert_eq!(world.roads.out_lanes(0).len(), 3);
    }

    #[test]
    fn road_passing_through_a_junction_does_not_run_along_its_roads() {
        // Roads east and north-east from the origin. The new road crosses the east road 12 m
        // out, so it joins the origin, then would cross the north-east road 39 m out.
        let mut net = RoadNetwork::default();
        let (c, e, ne) = (
            net.add_node(0.0, 0.0),
            net.add_node(100.0, 0.0),
            net.add_node(60.0, 100.0),
        );
        net.add_road(c, e, RoadClass::Street);
        net.add_road(c, ne, RoadClass::Street);
        let mut world = World::new(Config::default(), net, Rng::new(1));
        let mut out = Vec::new();
        world.write_road_preview(0.0, -50.0, 36.0, 100.0, &mut out);
        let joins: Vec<f32> = out.chunks(3).map(|s| s[2]).collect();
        assert_eq!(joins, [0.0, 1.0, 0.0], "through the origin, no split");
        assert!(build(&mut world, 0.0, -50.0, 36.0, 100.0));
        assert_eq!(world.roads.road_count(), 4);
        assert_eq!(world.roads.out_lanes(c).len(), 4);
    }

    #[test]
    fn road_on_top_of_a_road_is_refused() {
        let mut world = roads(&[(-100.0, 0.0, 100.0, 0.0)]);
        assert!(!build(&mut world, -50.0, 1.0, 50.0, 1.0), "both ends on it");
        assert!(!build(&mut world, -50.0, 3.0, 150.0, 3.0), "runs along it");
        assert!(!build(&mut world, -100.0, 0.0, 100.0, 0.0), "duplicate");
        assert_eq!(world.roads.node_count(), 2);
        assert_eq!(world.roads.road_count(), 1);
    }

    #[test]
    fn crossings_too_close_together_are_refused() {
        let mut world = roads(&[(-100.0, 0.0, 100.0, 0.0), (-100.0, 6.0, 100.0, 6.0)]);
        assert!(!build(&mut world, 0.0, -100.0, 0.0, 100.0));
        assert_eq!(world.roads.road_count(), 2);
    }

    #[test]
    fn splitting_a_road_removes_cars_that_use_it() {
        // West road (lanes 0 east, 1 west) and east road (lanes 2 east, 3 west), meeting at x=0.
        let mut net = RoadNetwork::default();
        let (w, c, e) = (
            net.add_node(-200.0, 0.0),
            net.add_node(0.0, 0.0),
            net.add_node(200.0, 0.0),
        );
        net.add_road(w, c, RoadClass::Street);
        net.add_road(c, e, RoadClass::Street);
        let mut world = World::new(Config::default(), net, Rng::new(1));
        let drive = |world: &mut World, path: Vec<LaneId>| {
            let agent = world.agents.push(world.roads.lane_from[path[0] as usize]);
            let a = agent as usize;
            world.agents.dest[a] = world.roads.lane_to[*path.last().unwrap() as usize];
            world.agents.path[a] = path.clone();
            world.traffic.enter(path[0], agent, &mut world.agents);
            agent as usize
        };
        let on_west = drive(&mut world, vec![0, 2]);
        let heading_west = drive(&mut world, vec![3, 1]);
        let east_only = drive(&mut world, vec![2]);
        assert!(build(&mut world, -100.0, -100.0, -100.0, 100.0));
        let s = world.stats();
        assert_eq!(s.despawned_by_edit, 2, "{s:?}");
        assert_eq!(world.agents.state[on_west], AgentState::Parked);
        assert_eq!(world.agents.state[heading_west], AgentState::Parked);
        assert_eq!(world.agents.state[east_only], AgentState::Driving);
        assert!(
            world
                .traffic
                .queues
                .iter()
                .all(|q| !q.contains(&(on_west as AgentId)))
        );
        for _ in 0..600 {
            world.tick();
        }
        assert!(world.stats().trips_completed >= 1);
    }

    #[test]
    fn building_across_busy_traffic_keeps_it_running() {
        let mut world = small_world(3);
        for _ in 0..1_500 {
            world.tick();
        }
        let b = world.roads.bounds();
        let (w, h) = (b[2] - b[0], b[3] - b[1]);
        // Long diagonals, crossing many streets each. A few may cross a jittered street at too
        // sharp an angle and be refused, which is fine; most must build.
        let mut built = 0;
        for k in 0..4 {
            let f = k as f32 / 8.0;
            let (y0, y1) = (b[1] + f * h, b[1] + (f + 0.5) * h);
            let (x0, x1) = (b[0] + f * w, b[0] + (f + 0.5) * w);
            built += build(&mut world, b[0] + 7.0, y0, b[2] - 7.0, y1) as u32;
            built += build(&mut world, x0, b[1] + 7.0, x1, b[3] - 7.0) as u32;
            for _ in 0..300 {
                world.tick();
            }
        }
        assert!(built >= 5, "built {built} of 8");
        let trips = world.stats().trips_completed;
        for _ in 0..1_500 {
            world.tick();
        }
        let s = world.stats();
        assert!(s.despawned_by_edit > 0, "{s:?}");
        assert!(s.trips_completed > trips + 200, "{s:?}");
        assert_eq!(
            s.parked + s.awaiting_path + s.entering + s.driving,
            s.agents
        );
        // Every queued car is on the lane it thinks it is on.
        for (lane, q) in world.traffic.queues.iter().enumerate() {
            for &agent in q {
                assert_eq!(world.agents.lane[agent as usize] as usize, lane);
                assert_eq!(world.agents.state[agent as usize], AgentState::Driving);
            }
        }
    }

    #[test]
    fn hover_shows_where_a_road_would_start() {
        let world = roads(&[(-100.0, 0.0, 100.0, 0.0)]);
        let mut out = Vec::new();
        world.write_snap_preview(0.0, 6.0, &mut out);
        assert_eq!(out, [0.0, 0.0, 2.0], "on the road, splitting it");
        world.write_snap_preview(-95.0, 4.0, &mut out);
        assert_eq!(out, [-100.0, 0.0, 1.0], "at its end");
        world.write_snap_preview(0.0, 50.0, &mut out);
        assert_eq!(out, [0.0, 50.0, 0.0], "a new end");
    }

    #[test]
    fn preview_shows_what_gets_built() {
        let world = roads(&[(-100.0, 0.0, 100.0, 0.0)]);
        let mut out = Vec::new();
        world.write_road_preview(0.0, -100.0, 0.0, 100.0, &mut out);
        assert_eq!(
            out,
            [0.0, -100.0, 0.0, 0.0, 0.0, 2.0, 0.0, 100.0, 0.0],
            "[x, y, join] per stop: new end, split, new end"
        );
        world.write_road_preview(-50.0, 1.0, 50.0, 1.0, &mut out);
        assert!(out.is_empty(), "refused");
        let before = world.roads.version;
        world.write_road_preview(0.0, -100.0, 0.0, 100.0, &mut out);
        assert_eq!(world.roads.version, before, "preview changes nothing");
    }

    /// A four-way junction at the origin with 100 m arms east, north, west and south. Arm `k`
    /// owns lane `2k` (out of the junction) and `2k + 1` (into it).
    fn crossroads(ew: RoadClass) -> World {
        let mut net = RoadNetwork::default();
        let centre = net.add_node(0.0, 0.0);
        for k in 0..4 {
            let angle = k as f32 * std::f32::consts::FRAC_PI_2;
            let arm = net.add_node(100.0 * angle.cos(), 100.0 * angle.sin());
            net.add_road(centre, arm, if k % 2 == 0 { ew } else { RoadClass::Street });
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
            let mut world = crossroads(RoadClass::Street);
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
        let mut world = crossroads(RoadClass::Main);
        let mut signals = Vec::new();
        world.write_signal_snapshot(&mut signals);
        assert_eq!(signals.len(), 4 * SIGNAL_STRIDE);
        // Arm k's incoming lane is 2k + 1; its straight-on exit is the opposite arm's lane.
        let roads = world.roads.clone();
        let state = |signals: &[f32], lane: LaneId| {
            let heading = roads.lane_heading(lane, roads.lane_len[lane as usize]);
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
        let mut world = crossroads(RoadClass::Street);
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
