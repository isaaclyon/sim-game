//! Networks that follow roads: power grids (ADR 0041) and water networks (ADR 0052). Roads
//! conduct, so road nodes joined by a road share a network; placed pieces (plants, pumps,
//! towers) join the roads beside them; power lines join whatever their ends touch.

use super::{Footprint, LINE_REACH, PLANT_REACH};
use crate::geometry::{Point, dist};
use crate::road::RoadNetwork;

/// A line end within this of another line's end joins it, metres.
const LINE_JOIN: f32 = 1.0;

/// Network per road node and per piece. IDs are dense, numbered in node then piece order.
#[derive(Default)]
pub struct Grids {
    pub node: Vec<u32>,
    /// `u32::MAX` for removed pieces.
    pub piece: Vec<u32>,
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

/// Networks of `roads` with `pieces` and `lines` by ID; `None` marks a removed one.
pub fn derive(
    roads: &RoadNetwork,
    pieces: &[Option<Footprint>],
    lines: &[Option<(Point, Point)>],
) -> Grids {
    let nodes = roads.node_count();
    let plants = pieces.len();
    let mut sets = Sets((0..(nodes + plants + lines.len()) as u32).collect());
    let active: Vec<u32> = roads.active_roads().collect();
    for &road in &active {
        let (a, b) = roads.road_ends(road);
        sets.join(a as usize, b as usize);
    }
    let road_near = |road, p: (f32, f32)| {
        let shape = roads.shape(road);
        dist(shape.point(shape.nearest(p)), p) <= LINE_REACH
    };
    let placed = || {
        pieces
            .iter()
            .enumerate()
            .filter_map(|(p, at)| at.map(|at| (p, at)))
    };
    for (p, at) in placed() {
        for &road in &active {
            let touches = roads
                .shape(road)
                .segments()
                .any(|(_, a, _, b)| at.hits_segment(a, b, PLANT_REACH));
            if touches {
                sets.join(nodes + p, roads.road_ends(road).0 as usize);
            }
        }
    }
    let drawn = || {
        lines
            .iter()
            .enumerate()
            .filter_map(|(l, ends)| ends.map(|ends| (l, ends)))
    };
    for (l, (a, b)) in drawn() {
        let me = nodes + plants + l;
        for end in [a, b] {
            for (p, at) in placed() {
                if at.contains(end, LINE_REACH) {
                    sets.join(me, nodes + p);
                }
            }
            for &road in &active {
                if road_near(road, end) {
                    sets.join(me, roads.road_ends(road).0 as usize);
                }
            }
            for (other, (c, d)) in drawn().take_while(|&(o, _)| o < l) {
                if dist(end, c) <= LINE_JOIN || dist(end, d) <= LINE_JOIN {
                    sets.join(me, nodes + plants + other);
                }
            }
        }
    }
    let mut id = vec![u32::MAX; nodes + plants];
    let mut count = 0;
    for x in 0..nodes + plants {
        if x >= nodes && pieces[x - nodes].is_none() {
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
        piece: id[nodes..].to_vec(),
        count: count as usize,
    }
}
