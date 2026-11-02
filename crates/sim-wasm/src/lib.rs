//! Thin wasm-bindgen wrapper. Logic belongs in `sim`; this crate only converts types.

use sim::road::RoadClass;
use sim::{Command, Config, World, Zone};
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub struct Sim {
    world: World,
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
    /// R/C/I demand in people (ADR 0024): positive wants more buildings, negative is a surplus.
    pub demand_residential: i32,
    pub demand_commercial: i32,
    pub demand_industrial: i32,
    pub parked: u32,
    pub awaiting_path: u32,
    pub entering: u32,
    pub driving: u32,
    pub trips_completed: f64,
    pub paths_failed: f64,
    pub despawned_stuck: f64,
    pub despawned_by_edit: f64,
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
}

#[wasm_bindgen]
impl Sim {
    #[wasm_bindgen(constructor)]
    pub fn new(seed: u32, cols: u32, rows: u32) -> Sim {
        let cfg = Config {
            seed: seed as u64,
            ..Config::default()
        };
        Sim {
            world: World::generate(cfg, cols, rows),
            buf: Vec::new(),
        }
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
        self.world.apply(Command::SpawnAgents { count })
    }

    /// `class` is 0 = street, 1 = main road, 2 = alley. Unknown values build nothing.
    pub fn build_road(&mut self, ax: f32, ay: f32, bx: f32, by: f32, class: u8) -> bool {
        let Some(class) = RoadClass::from_u8(class) else {
            return false;
        };
        self.world.apply(Command::BuildRoad {
            ax,
            ay,
            bx,
            by,
            class,
        })
    }

    #[allow(clippy::too_many_arguments)] // Mirrors the coordinate-only WASM command boundary.
    pub fn build_curve(
        &mut self,
        ax: f32,
        ay: f32,
        cx: f32,
        cy: f32,
        bx: f32,
        by: f32,
        class: u8,
    ) -> bool {
        let Some(class) = RoadClass::from_u8(class) else {
            return false;
        };
        self.world.apply(Command::BuildCurve {
            ax,
            ay,
            cx,
            cy,
            bx,
            by,
            class,
        })
    }

    pub fn curve_preview(
        &mut self,
        ax: f32,
        ay: f32,
        cx: f32,
        cy: f32,
        bx: f32,
        by: f32,
    ) -> Vec<f32> {
        self.world
            .write_curve_preview(ax, ay, cx, cy, bx, by, &mut self.buf);
        self.buf.clone()
    }

    pub fn delete_road(&mut self, x: f32, y: f32) -> bool {
        self.world.apply(Command::DeleteRoad { x, y })
    }

    /// `zone` is the cell byte (ADR 0014): 0 = none, 1/2 residential, 3/4 commercial,
    /// 5/6 industrial, low/high. Unknown values change nothing.
    pub fn paint_zone(&mut self, ax: f32, ay: f32, bx: f32, by: f32, zone: u8) -> bool {
        let Some(zone) = Zone::from_u8(zone) else {
            return false;
        };
        self.world.apply(Command::PaintZone {
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
        self.world.apply(Command::FillBlock { x, y, zone })
    }

    /// `[x, y]` per outline sample of the block containing `(x, y)`, or empty outside every block.
    pub fn block_preview(&mut self, x: f32, y: f32) -> Vec<f32> {
        self.world.write_block_preview(x, y, &mut self.buf);
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

    pub fn buildings_version(&self) -> u32 {
        self.world.buildings.version
    }

    /// `[zone, height, roof, n, x0, y0, …]` per active building; footprint points are counter-clockwise.
    pub fn building_snapshot(&mut self) -> Vec<f32> {
        self.world.buildings.write_snapshot(&mut self.buf);
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

    /// `[x, y]` per centreline sample of the selected road, or empty on a miss.
    pub fn bulldoze_preview(&mut self, x: f32, y: f32) -> Vec<f32> {
        self.world.write_bulldoze_preview(x, y, &mut self.buf);
        self.buf.clone()
    }

    /// `[x, y, join]` per stop of the road `build_road` would build; see
    /// `World::write_road_preview`. Empty if it would be refused.
    pub fn road_preview(&mut self, ax: f32, ay: f32, bx: f32, by: f32) -> Vec<f32> {
        self.world.write_road_preview(ax, ay, bx, by, &mut self.buf);
        self.buf.clone()
    }

    /// `[x, y, join]` for where a road started here would start; see
    /// `World::write_snap_preview`.
    pub fn snap_preview(&mut self, x: f32, y: f32) -> Vec<f32> {
        self.world.write_snap_preview(x, y, &mut self.buf);
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
        })
    }

    /// `[x, y, heading, speed_ratio]` per agent; see `World::write_agent_snapshot`.
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

    /// `[min_x, min_y, max_x, max_y]`.
    pub fn bounds(&self) -> Vec<f32> {
        self.world.roads.bounds().to_vec()
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
            parked: s.parked,
            awaiting_path: s.awaiting_path,
            entering: s.entering,
            driving: s.driving,
            trips_completed: s.trips_completed as f64,
            paths_failed: s.paths_failed as f64,
            despawned_stuck: s.despawned_stuck as f64,
            despawned_by_edit: s.despawned_by_edit as f64,
        }
    }
}
