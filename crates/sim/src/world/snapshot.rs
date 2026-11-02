//! Snapshots and previews the frontend draws, and the determinism hash.

use super::{SNAPSHOT_STRIDE, World};
use crate::agents::AgentState;
use crate::geometry::Shape;
use crate::snap::{self, Join};

/// Distance from the road centreline to the middle of each lane, metres.
const LANE_OFFSET: f32 = 2.0;
/// Walkers keep this far right of the centreline, just off the asphalt; wider on main roads.
const SIDEWALK_OFFSET: f32 = 5.0;
const MAIN_SIDEWALK_OFFSET: f32 = 7.0;
/// Signal heads stand this far right of the road centreline, metres.
const SIGNAL_OFFSET: f32 = 6.0;

impl World {
    /// Writes `[x, y, heading, speed_ratio, kind]` per agent, indexed by agent id so the
    /// renderer can interpolate between snapshots. Heading is radians from +x. Kind is 0 = car,
    /// 1 = walker on the sidewalk (ADR 0026). Agents not on a road or sidewalk get
    /// `speed_ratio = -1`.
    pub fn write_agent_snapshot(&self, out: &mut Vec<f32>) {
        let a = &self.agents;
        out.clear();
        out.reserve(a.len() * SNAPSHOT_STRIDE);
        for i in 0..a.len() {
            let lane = a.lane[i];
            let (side, limit, kind) = match a.state[i] {
                AgentState::Driving => (LANE_OFFSET, self.roads.lane_speed[lane as usize], 0.0),
                AgentState::Walking => {
                    let main = self.roads.lane_class(lane) == crate::road::RoadClass::Main;
                    let side = if main {
                        MAIN_SIDEWALK_OFFSET
                    } else {
                        SIDEWALK_OFFSET
                    };
                    let c = a.citizen[i].expect("walkers are citizens") as usize;
                    (side, self.citizens.walk_speed[c], 1.0)
                }
                _ => {
                    out.extend([0.0, 0.0, 0.0, -1.0, 0.0]);
                    continue;
                }
            };
            let (x, y) = self.roads.lane_point(lane, a.pos[i], side);
            let heading = self.roads.lane_heading(lane, a.pos[i]);
            out.extend([x, y, heading, a.speed[i] / limit, kind]);
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
            mix(a.citizen[i].map_or(u64::MAX, u64::from));
            mix(a.node[i] as u64);
            mix(a.speed[i].to_bits() as u64);
            mix(a.stuck_ticks[i] as u64);
            mix(a.path_idx[i] as u64);
            mix(a.path[i].len() as u64);
            for &lane in &a.path[i] {
                mix(lane as u64);
            }
            mix(a.lane[i] as u64);
            mix(a.pos[i].to_bits() as u64);
            mix(a.dest[i] as u64);
            mix(a.cleared[i] as u64);
            mix(a.walk_end[i].to_bits() as u64);
        }
        self.hash_residents(&mut mix);
        self.hash_closures(&mut mix);
        mix(self.rng.state());
        for entry in self.wake.clone().into_sorted_vec() {
            mix(entry.0.0);
            mix(entry.0.1 as u64);
        }
        for &a in &self.path_queue {
            mix(a as u64);
        }
        for &a in &self.entering {
            mix(a as u64);
        }
        mix(self.walking.len() as u64);
        for &a in &self.walking {
            mix(a as u64);
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
}

/// How a preview stop joins the network, as the frontend reads it.
fn join_code(join: Join) -> f32 {
    match join {
        Join::New => 0.0,
        Join::Node(_) => 1.0,
        Join::Split(_) => 2.0,
    }
}
