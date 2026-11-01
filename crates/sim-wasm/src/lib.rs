//! Thin wasm-bindgen wrapper. Logic belongs in `sim`; this crate only converts types.

use sim::{Command, Config, World};
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

    pub fn build_road(&mut self, ax: f32, ay: f32, bx: f32, by: f32) -> bool {
        self.world.apply(Command::BuildRoad { ax, ay, bx, by })
    }

    pub fn delete_road(&mut self, x: f32, y: f32) -> bool {
        self.world.apply(Command::DeleteRoad { x, y })
    }

    /// `[ax, ay, bx, by]` of the selected segment, or empty on a miss.
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

    /// `[ax, ay, bx, by, speed_limit]` per active road, packed in road ID order.
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
