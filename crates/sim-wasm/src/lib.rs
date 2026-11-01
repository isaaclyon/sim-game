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
    pub agents: u32,
    pub parked: u32,
    pub awaiting_path: u32,
    pub entering: u32,
    pub driving: u32,
    pub trips_completed: f64,
    pub paths_failed: f64,
    pub despawned_stuck: f64,
    pub despawned_by_edit: f64,
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

    /// `[zone, height, n, x0, y0, …]` per active building; footprint points are counter-clockwise.
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
        SimStats {
            tick: s.tick as f64,
            agents: s.agents,
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
