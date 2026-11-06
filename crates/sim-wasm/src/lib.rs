//! Thin wasm-bindgen wrapper. Logic belongs in `sim`; this crate only converts types.

use sim::power::PlantKind;
use sim::{Command, CommandLog, World, Zone};
use wasm_bindgen::prelude::*;

mod roads;

#[wasm_bindgen]
pub struct Sim {
    world: World,
    /// Commands since a fresh start (ADR 0034); a loaded city has no log.
    log: Option<CommandLog>,
    buf: Vec<f32>,
}

/// Counters are f64 because JS numbers can't hold a u64 without BigInt.
#[wasm_bindgen]
#[derive(Clone, Copy)]
pub struct SimStats {
    pub tick: f64,
    /// Day clock (see `sim::clock`): 1-based day and seconds since midnight.
    pub day: u32,
    pub second_of_day: u32,
    pub agents: u32,
    pub population: u32,
    pub unemployed: u32,
    pub unemployment_percent: f32,
    /// R/C/I demand in people (ADR 0025): positive wants more buildings, negative is a surplus.
    pub demand_residential: i32,
    pub demand_commercial: i32,
    pub demand_industrial: i32,
    /// Power (ADR 0041): plant output now, starter power, demand of powered and of all
    /// buildings, unpowered buildings and lots waiting for power.
    pub power_supply: u32,
    pub power_starter: u32,
    pub power_used: u32,
    pub power_demand: u32,
    pub power_unpowered: u32,
    pub power_waiting: u32,
    pub parked: u32,
    pub awaiting_path: u32,
    pub entering: u32,
    pub driving: u32,
    pub walking: u32,
    pub trips_completed: f64,
    pub walks_completed: f64,
    pub paths_failed: f64,
    pub despawned_stuck: f64,
    pub despawned_by_edit: f64,
    /// Money in whole dollars (ADR 0036); taxes, grant and upkeep are per clock hour.
    pub balance: f64,
    pub taxes: f64,
    pub grant: f64,
    pub upkeep: f64,
}

/// One citizen's details for the inspector (ADR 0022); see `World::citizen_info`. Places are
/// building IDs with their original use as a zone byte; `None` when absent or removed.
/// `activity` is 0 = home, 1 = work, 2 = shop.
#[wasm_bindgen(getter_with_clone)]
pub struct CitizenView {
    pub name: String,
    pub resident: bool,
    pub home: Option<u32>,
    pub home_zone: Option<u8>,
    pub job: Option<u32>,
    pub job_zone: Option<u8>,
    pub activity: u8,
    pub location: Option<u32>,
    pub location_zone: Option<u8>,
    pub destination: Option<u32>,
    pub destination_zone: Option<u8>,
    pub agent: Option<u32>,
    pub driving: bool,
    pub on_foot: bool,
}

/// Sim-owned counts for one standing building; no attendance or visitor counts.
#[wasm_bindgen]
#[derive(Clone, Copy)]
pub struct BuildingView {
    pub building: u32,
    pub zone: u8,
    pub capacity: u32,
    pub occupied: u32,
    pub closing: bool,
    pub powered: bool,
}

impl Sim {
    fn apply(&mut self, cmd: Command) -> bool {
        match &mut self.log {
            Some(log) => log.apply(&mut self.world, cmd),
            None => self.world.apply(cmd),
        }
    }
}

#[wasm_bindgen]
impl Sim {
    /// A city on the generated `cols` × `rows` street grid, for tests and benchmarks.
    #[wasm_bindgen(constructor)]
    pub fn new(seed: u32, cols: u32, rows: u32) -> Sim {
        Self::start(CommandLog::new(seed as u64, cols, rows))
    }

    /// A fresh city as the game starts one: no roads, on a map the size of that grid.
    pub fn empty(seed: u32, cols: u32, rows: u32) -> Sim {
        Self::start(CommandLog::empty(seed as u64, cols, rows))
    }

    fn start(log: CommandLog) -> Sim {
        Sim {
            world: log.world(),
            log: Some(log),
            buf: Vec::new(),
        }
    }

    /// The command log text from the fresh start to now, or undefined for a loaded city.
    pub fn command_log(&self) -> Option<String> {
        let mut log = self.log.clone()?;
        log.end = self.world.tick_count();
        Some(log.to_string())
    }

    /// The complete city as opaque save bytes (ADR 0033). Changes nothing.
    pub fn save(&self) -> Result<Vec<u8>, JsValue> {
        self.world.save().map_err(save_error)
    }

    /// A city from `save` bytes. Throws an `Error` with a readable message for missing,
    /// damaged or other-version saves; nothing is loaded then.
    pub fn load(bytes: &[u8]) -> Result<Sim, JsValue> {
        Ok(Sim {
            world: World::load(bytes).map_err(save_error)?,
            log: None,
            buf: Vec::new(),
        })
    }

    /// Sim seconds per tick, so the frontend clock can't drift from the sim's.
    pub fn dt(&self) -> f32 {
        self.world.cfg.dt
    }

    pub fn tick(&mut self, count: u32) {
        for _ in 0..count {
            self.world.tick();
        }
    }

    pub fn spawn_agents(&mut self, count: u32) -> bool {
        self.apply(Command::SpawnAgents { count })
    }

    pub fn delete_road(&mut self, x: f32, y: f32) -> bool {
        self.apply(Command::DeleteRoad { x, y })
    }

    /// `zone` is the cell byte (ADR 0014): 0 = none, 1/2 residential, 3/4 commercial,
    /// 5/6 industrial, low/high. Unknown values change nothing.
    pub fn paint_zone(&mut self, ax: f32, ay: f32, bx: f32, by: f32, zone: u8) -> bool {
        let Some(zone) = Zone::from_u8(zone) else {
            return false;
        };
        self.apply(Command::PaintZone {
            ax,
            ay,
            bx,
            by,
            zone,
        })
    }

    /// Sets every cell of the block containing `(x, y)` (ADR 0016). Same `zone` bytes as
    /// `paint_zone`.
    pub fn fill_block(&mut self, x: f32, y: f32, zone: u8) -> bool {
        let Some(zone) = Zone::from_u8(zone) else {
            return false;
        };
        self.apply(Command::FillBlock { x, y, zone })
    }

    /// `[x, y]` per outline sample of the block containing `(x, y)`, or empty outside every block.
    pub fn block_preview(&mut self, x: f32, y: f32) -> Vec<f32> {
        self.world.write_block_preview(x, y, &mut self.buf);
        self.buf.clone()
    }

    /// `kind` is 0 = coal, 1 = solar, 2 = wind (ADR 0041). Unknown values build nothing.
    pub fn place_plant(&mut self, kind: u8, x: f32, y: f32) -> bool {
        let Some(kind) = PlantKind::from_u8(kind) else {
            return false;
        };
        self.apply(Command::PlacePlant { kind, x, y })
    }

    pub fn build_line(&mut self, ax: f32, ay: f32, bx: f32, by: f32) -> bool {
        self.apply(Command::BuildLine { ax, ay, bx, by })
    }

    /// `[ok, x0, y0, …]`; see `World::write_plant_preview`. Empty for an unknown kind.
    pub fn plant_preview(&mut self, kind: u8, x: f32, y: f32) -> Vec<f32> {
        self.buf.clear();
        if let Some(kind) = PlantKind::from_u8(kind) {
            self.world.write_plant_preview(kind, x, y, &mut self.buf);
        }
        self.buf.clone()
    }

    /// `[ax, ay, a_join, bx, by, b_join]`; see `World::write_line_preview`.
    pub fn line_preview(&mut self, ax: f32, ay: f32, bx: f32, by: f32) -> Vec<f32> {
        self.world.write_line_preview(ax, ay, bx, by, &mut self.buf);
        self.buf.clone()
    }

    pub fn power_version(&self) -> u32 {
        self.world.power_version()
    }

    /// `[kind, x, y, half_size, front_x, front_y, terminal_x, terminal_y, output, grid]` per
    /// plant; see `World::write_plant_snapshot`.
    pub fn plant_snapshot(&mut self) -> Vec<f32> {
        self.world.write_plant_snapshot(&mut self.buf);
        self.buf.clone()
    }

    /// `[ax, ay, bx, by]` per transmission line.
    pub fn line_snapshot(&mut self) -> Vec<f32> {
        self.world.write_line_snapshot(&mut self.buf);
        self.buf.clone()
    }

    /// `[x, y, height, grid, powered]` per active building; see `World::write_power_buildings`.
    pub fn power_buildings(&mut self) -> Vec<f32> {
        self.world.write_power_buildings(&mut self.buf);
        self.buf.clone()
    }

    pub fn lots_version(&self) -> u32 {
        self.world.lots.version
    }

    /// `[n, x0, y0, …]` per lot: its outline's point count, then its points (ADR 0018).
    pub fn lot_outlines(&mut self) -> Vec<f32> {
        self.world.write_lot_outlines(&mut self.buf);
        self.buf.clone()
    }

    /// Stable IDs alongside the packed building snapshot, in the same active-building order.
    pub fn building_ids(&self) -> Vec<u32> {
        self.world.buildings.active_buildings().collect()
    }

    pub fn building_info(&self, id: u32) -> Option<BuildingView> {
        let b = self.world.building_info(id)?;
        Some(BuildingView {
            building: b.building,
            zone: b.zone as u8,
            capacity: b.capacity,
            occupied: b.occupied,
            closing: b.closing,
            powered: self.world.building_powered(id),
        })
    }

    pub fn buildings_version(&self) -> u32 {
        self.world.buildings.version
    }

    /// `[zone, height, roof, front, n, x0, y0, …]` per active building; footprint points are
    /// counter-clockwise and `front` is the edge facing the lot's road.
    pub fn building_snapshot(&mut self) -> Vec<f32> {
        self.world
            .buildings
            .write_snapshot(&self.world.lots, &mut self.buf);
        self.buf.clone()
    }

    pub fn zones_version(&self) -> u32 {
        self.world.zones.version
    }

    /// One byte per zone cell, row-major from the grid origin; see `zone_layout`.
    pub fn zone_cells(&self) -> Vec<u8> {
        self.world.zones.cells.clone()
    }

    /// `[origin_x, origin_y, cell_size, cols, rows]` of the zone grid.
    pub fn zone_layout(&self) -> Vec<f32> {
        self.world.zones.layout().to_vec()
    }

    pub fn pollution_version(&self) -> u32 {
        self.world.pollution.version
    }

    /// `[air, ground, noise, land_value]` bytes per pollution cell, row-major; see
    /// `pollution_layout` (ADR 0037).
    pub fn pollution_cells(&mut self) -> Vec<u8> {
        let mut out = Vec::new();
        self.world.pollution.write_snapshot(&mut out);
        out
    }

    /// `[origin_x, origin_y, cell_size, cols, rows]` of the pollution grid.
    pub fn pollution_layout(&self) -> Vec<f32> {
        self.world.pollution.layout().to_vec()
    }

    /// `[x, y]` per centreline sample of the selected road, or empty on a miss.
    pub fn bulldoze_preview(&mut self, x: f32, y: f32) -> Vec<f32> {
        self.world.write_bulldoze_preview(x, y, &mut self.buf);
        self.buf.clone()
    }

    pub fn roads_version(&self) -> u32 {
        self.world.roads.version
    }

    /// The resident who owns vehicle slot `agent`; `None` for synthetic or unknown slots.
    pub fn citizen_of_agent(&self, agent: u32) -> Option<u32> {
        self.world.citizen_of_agent(agent)
    }

    /// Read-only details of citizen `id`, or `None` for an unknown ID.
    pub fn citizen_info(&self, id: u32) -> Option<CitizenView> {
        let c = self.world.citizen_info(id)?;
        let split = |p: Option<sim::world::Place>| (p.map(|p| p.building), p.map(|p| p.zone as u8));
        let (home, home_zone) = split(c.home);
        let (job, job_zone) = split(c.job);
        let (location, location_zone) = split(c.location);
        let (destination, destination_zone) = split(c.destination);
        Some(CitizenView {
            name: c.name,
            resident: c.resident,
            home,
            home_zone,
            job,
            job_zone,
            activity: c.activity as u8,
            location,
            location_zone,
            destination,
            destination_zone,
            agent: c.agent,
            driving: c.driving,
            on_foot: c.on_foot,
        })
    }

    /// `[x, y, heading, speed_ratio, kind]` per agent; see `World::write_agent_snapshot`.
    pub fn agent_snapshot(&mut self) -> Vec<f32> {
        self.world.write_agent_snapshot(&mut self.buf);
        self.buf.clone()
    }

    /// `[x, y, heading, state]` per signalised approach; see `World::write_signal_snapshot`.
    pub fn signal_snapshot(&mut self) -> Vec<f32> {
        self.world.write_signal_snapshot(&mut self.buf);
        self.buf.clone()
    }

    /// `[ax, ay, bx, by, class]` per active rendered segment (class 0 = street, 1 = main road,
    /// 2 = alley);
    /// curves have multiple segments.
    pub fn road_segments(&mut self) -> Vec<f32> {
        self.world.write_road_segments(&mut self.buf);
        self.buf.clone()
    }

    /// `[min_x, min_y, max_x, max_y]` of the map.
    pub fn bounds(&self) -> Vec<f32> {
        self.world.bounds().to_vec()
    }

    pub fn stats(&self) -> SimStats {
        let s = self.world.stats();
        let clock = sim::clock::clock_at(s.tick, self.world.cfg.dt);
        SimStats {
            tick: s.tick as f64,
            day: clock.day,
            second_of_day: clock.second_of_day,
            agents: s.agents,
            population: s.population,
            unemployed: s.unemployed,
            unemployment_percent: s.unemployment_percent,
            demand_residential: s.demand.residential,
            demand_commercial: s.demand.commercial,
            demand_industrial: s.demand.industrial,
            power_supply: s.power.supply,
            power_starter: s.power.starter,
            power_used: s.power.used,
            power_demand: s.power.demand,
            power_unpowered: s.power.unpowered,
            power_waiting: s.power.waiting,
            parked: s.parked,
            awaiting_path: s.awaiting_path,
            entering: s.entering,
            driving: s.driving,
            walking: s.walking,
            trips_completed: s.trips_completed as f64,
            walks_completed: s.walks_completed as f64,
            paths_failed: s.paths_failed as f64,
            despawned_stuck: s.despawned_stuck as f64,
            despawned_by_edit: s.despawned_by_edit as f64,
            balance: s.money.balance as f64,
            taxes: s.money.taxes as f64,
            grant: s.money.grant as f64,
            upkeep: s.money.upkeep as f64,
        }
    }
}

/// A JS `Error` whose message says why the save failed.
fn save_error(e: sim::SaveError) -> JsValue {
    JsError::new(&e.to_string()).into()
}
