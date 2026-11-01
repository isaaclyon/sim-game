//! Blocks: the land enclosed by roads, and how it is cut into lots (ADR 0016, ADR 0018).
//!
//! Blocks are derived from the road graph every time they are needed and have no stable ids.
//! A block is a bounded face of the graph, traced along the sampled road centrelines.

mod alleys;
mod faces;
mod industrial;
mod lots;
mod offsets;
mod query;

use crate::geometry::{Point, Shape, dist, lerp};
use crate::road::{LaneId, RoadId};

/// Half a road's width: cells whose centre is closer to a centreline are under the road.
pub const ROAD_HALF_WIDTH: f32 = 4.0;
/// Cells farther than this from every road centreline are a block's interior and get no lot.
pub const LOT_REACH: f32 = 64.0;
/// Target frontage of one lot along its road, metres.
pub const LOT_WIDTH: f32 = 16.0;
/// Smaller pieces of land, square metres, such as the corner triangles left where two roads'
/// lots meet, join a neighbouring lot. About 40% of a typical 16 m by 30 m lot.
pub const MIN_LOT_AREA: f32 = 200.0;
/// Gaps between lots smaller than this, square metres, join a lot; bigger ones are interior.
pub const MAX_GAP_AREA: f32 = 1000.0;
/// Fewer zoned interior cells than this are left as open space instead of getting an alley.
pub const MIN_DEEP_CELLS: usize = 6;

/// `[min_x, min_y, max_x, max_y]`.
pub type Rect = [f32; 4];

pub fn intersects(a: Rect, b: Rect) -> bool {
    a[0] <= b[2] && b[0] <= a[2] && a[1] <= b[3] && b[1] <= a[3]
}

pub fn union(a: Rect, b: Rect) -> Rect {
    [
        a[0].min(b[0]),
        a[1].min(b[1]),
        a[2].max(b[2]),
        a[3].max(b[3]),
    ]
}

/// Whether `inner` lies inside `outer` grown by `margin` on every side.
pub(crate) fn contains_rect(outer: Rect, inner: Rect, margin: f32) -> bool {
    outer[0] - margin <= inner[0]
        && outer[1] - margin <= inner[1]
        && inner[2] <= outer[2] + margin
        && inner[3] <= outer[3] + margin
}

/// Bounding box of a shape, including a curve's control point.
pub(crate) fn shape_rect(s: Shape) -> Rect {
    let mut r = [
        s.a.0.min(s.b.0),
        s.a.1.min(s.b.1),
        s.a.0.max(s.b.0),
        s.a.1.max(s.b.1),
    ];
    if let Some(c) = s.control {
        r = union(r, [c.0, c.1, c.0, c.1]);
    }
    r
}

fn cross(a: Point, b: Point) -> f32 {
    a.0 * b.1 - a.1 * b.0
}
fn sub(a: Point, b: Point) -> Point {
    (a.0 - b.0, a.1 - b.1)
}

fn unit(p: Point) -> Point {
    let len = p.0.hypot(p.1).max(1e-9);
    (p.0 / len, p.1 / len)
}

/// Left of direction `d`.
fn left_of(d: Point) -> Point {
    (-d.1, d.0)
}

/// One zone cell of a block, with the road it is closest to.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Cell {
    pub index: u32,
    pub at: Point,
    /// Distance to the nearest road centreline.
    pub dist: f32,
    pub road: RoadId,
}

/// A lot about to be added: see `Lots`.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct LotPlan {
    pub road: RoadId,
    pub front: Point,
    pub facing: Point,
    /// Length of the stretch of road the lot was cut from, metres.
    pub frontage: f32,
    /// Counter-clockwise, without repeating the first point.
    pub outline: Vec<Point>,
    /// Row-major indices of the zone cells whose centre is inside the lot, ascending.
    pub cells: Vec<u32>,
}

/// Every road's sampled centreline with distances along it, and the network's bounded faces.
#[derive(Default)]
pub struct Blocks {
    // Per road slot, CSR into `pts`; tombstoned roads have empty ranges.
    road_start: Vec<u32>,
    pts: Vec<Point>,
    arc: Vec<f32>,
    road_rect: Vec<Rect>,
    // Per block, CSR into `outline`.
    start: Vec<u32>,
    outline: Vec<Point>,
    rect: Vec<Rect>,
    area: Vec<f32>,
    // Per block, CSR into `lanes`: the lanes around it, each with the block on its left.
    lane_start: Vec<u32>,
    lanes: Vec<LaneId>,
    /// Faces that are not blocks (the outside of each separate road network), each with a
    /// point just outside its first lane, to find the block they sit in.
    outer: Vec<(Point, Vec<LaneId>)>,
    /// Per lane, the lane continuing its face after it, and the one before it.
    next: Vec<LaneId>,
    prev: Vec<LaneId>,
}

impl Blocks {
    pub fn len(&self) -> usize {
        self.area.len()
    }

    pub fn is_empty(&self) -> bool {
        self.area.is_empty()
    }

    /// Centreline samples around block `i`, counter-clockwise, without repeating the first.
    pub fn outline(&self, i: usize) -> &[Point] {
        &self.outline[self.start[i] as usize..self.start[i + 1] as usize]
    }

    pub fn rect(&self, i: usize) -> Rect {
        self.rect[i]
    }

    fn road_points(&self, r: RoadId) -> &[Point] {
        &self.pts[self.road_start[r as usize] as usize..self.road_start[r as usize + 1] as usize]
    }

    fn lane_points(&self, lane: LaneId) -> Vec<Point> {
        let mut pts = self.road_points(lane / 2).to_vec();
        if !lane.is_multiple_of(2) {
            pts.reverse();
        }
        pts
    }

    /// Direction of the first sampled piece of `lane`.
    fn leaving(&self, lane: LaneId) -> Point {
        let pts = self.road_points(lane / 2);
        let n = pts.len();
        if lane.is_multiple_of(2) {
            sub(pts[1], pts[0])
        } else {
            sub(pts[n - 2], pts[n - 1])
        }
    }

    fn road_len(&self, r: RoadId) -> f32 {
        self.arc[self.road_start[r as usize + 1] as usize - 1]
    }

    /// The point `s` metres along road `r` and the unit direction of travel there.
    fn point_at(&self, r: RoadId, s: f32) -> (Point, Point) {
        let start = self.road_start[r as usize] as usize;
        let end = self.road_start[r as usize + 1] as usize;
        let arc = &self.arc[start..end];
        let k = arc.partition_point(|&v| v < s).clamp(1, arc.len() - 1);
        let (a, b) = (self.pts[start + k - 1], self.pts[start + k]);
        let f = ((s - arc[k - 1]) / (arc[k] - arc[k - 1]).max(1e-6)).clamp(0.0, 1.0);
        let len = dist(a, b).max(1e-6);
        (lerp(a, b, f), ((b.0 - a.0) / len, (b.1 - a.1) / len))
    }
}
