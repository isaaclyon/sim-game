//! Grids: everything joined for power (ADR 0036). Roads conduct, so road nodes joined by a road
//! share a grid; plants join the roads beside them; lines join whatever their ends touch.

use super::{LINE_REACH, PLANT_REACH, Power};
use crate::geometry::dist;
use crate::road::RoadNetwork;

/// A line end within this of another line's end joins it, metres.
const LINE_JOIN: f32 = 1.0;

/// Grid per road node and per plant. Grid IDs are dense, numbered in node then plant order.
#[derive(Default)]
pub struct Grids {
    pub node: Vec<u32>,
    /// `u32::MAX` for removed plants.
    pub plant: Vec<u32>,
    pub count: usize,
}

struct Sets(Vec<u32>);

impl Sets {
    fn find(&mut self, mut x: usize) -> usize {
        while self.0[x] as usize != x {
            let up = self.0[self.0[x] as usize];
            self.0[x] = up;
            x = up as usize;
        }
        x
    }

    /// The lower root wins, so the result does not depend on the order of joins.
    fn join(&mut self, a: usize, b: usize) {
        let (a, b) = (self.find(a), self.find(b));
        let (lo, hi) = (a.min(b), a.max(b));
        self.0[hi] = lo as u32;
    }
}

pub fn derive(roads: &RoadNetwork, power: &Power) -> Grids {
    let nodes = roads.node_count();
    let plants = power.plant_active.len();
    let lines = power.line_active.len();
    let mut sets = Sets((0..(nodes + plants + lines) as u32).collect());
    let active: Vec<u32> = roads.active_roads().collect();
    for &road in &active {
        let (a, b) = roads.road_ends(road);
        sets.join(a as usize, b as usize);
    }
    let road_near = |road, p: (f32, f32)| {
        let shape = roads.shape(road);
        dist(shape.point(shape.nearest(p)), p) <= LINE_REACH
    };
    for p in power.active_plants() {
        let at = power.footprint(p);
        for &road in &active {
            let touches = roads
                .shape(road)
                .segments()
                .any(|(_, a, _, b)| at.hits_segment(a, b, PLANT_REACH));
            if touches {
                sets.join(nodes + p as usize, roads.road_ends(road).0 as usize);
            }
        }
    }
    for l in power.active_lines() {
        let me = nodes + plants + l as usize;
        let (a, b) = power.line(l);
        for end in [a, b] {
            for p in power.active_plants() {
                if power.footprint(p).contains(end, LINE_REACH) {
                    sets.join(me, nodes + p as usize);
                }
            }
            for &road in &active {
                if road_near(road, end) {
                    sets.join(me, roads.road_ends(road).0 as usize);
                }
            }
            for other in power.active_lines().take_while(|&o| o < l) {
                let (c, d) = power.line(other);
                if dist(end, c) <= LINE_JOIN || dist(end, d) <= LINE_JOIN {
                    sets.join(me, nodes + plants + other as usize);
                }
            }
        }
    }
    let mut id = vec![u32::MAX; nodes + plants];
    let mut count = 0;
    for x in 0..nodes + plants {
        if x >= nodes && !power.plant_active[x - nodes] {
            continue;
        }
        // Roots are the lowest member, so a set's root comes before or at `x`.
        let root = sets.find(x);
        if id[root] == u32::MAX {
            id[root] = count;
            count += 1;
        }
        id[x] = id[root];
    }
    Grids {
        node: id[..nodes].to_vec(),
        plant: id[nodes..].to_vec(),
        count: count as usize,
    }
}
