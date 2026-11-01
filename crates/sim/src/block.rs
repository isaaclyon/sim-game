//! Blocks: the land enclosed by roads, and how it is cut into lots (ADR 0016, ADR 0017).
//!
//! Blocks are derived from the road graph every time they are needed and have no stable ids.
//! A block is a bounded face of the graph, traced along the sampled road centrelines.

use std::cmp::Ordering;
use std::collections::BTreeSet;

use crate::geometry::{Point, Shape, dist, lerp, project};
use crate::poly::{self, Ring};
use crate::road::{LaneId, RoadId, RoadNetwork};
use crate::zone::{Zone, ZoneGrid};

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
/// Outline corners sharper than this cosine (about 10°) cutting off less than `SPIKE_AREA`
/// square metres are spikes: hair-thin strips where two lots' back lines nearly agree.
const SPIKE_COS: f32 = 0.985;
const SPIKE_AREA: f32 = 2.0;
/// Cosine of the largest turn, 30°, between pieces of one straight block side.
const STRAIGHT_COS: f32 = 0.866;
/// Cosine of the largest turn away from 90°, 30°, at a corner of a rectangle-like block.
const RIGHT_COS: f32 = 0.5;
/// Spacing, metres, of the points along a road side that trace the back of its lots.
const BACK_STEP: f32 = 2.0;
/// Corners turning back sharper than this (`1 + cos` of the turn) end lots square instead of
/// along the corner's bisector: dead ends and hairpins.
const MIN_MITRE: f32 = 0.05;
/// Fewer zoned interior cells than this are left as open space instead of getting an alley.
pub const MIN_DEEP_CELLS: usize = 6;
/// Faces with less area, square metres, are slivers or tree-shaped pieces with no inside.
const MIN_BLOCK_AREA: f32 = 1.0;

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

fn rect_of(points: &[Point]) -> Rect {
    points
        .iter()
        .fold([f32::MAX, f32::MAX, f32::MIN, f32::MIN], |r, p| {
            union(r, [p.0, p.1, p.0, p.1])
        })
}

fn cross(a: Point, b: Point) -> f32 {
    a.0 * b.1 - a.1 * b.0
}
fn sub(a: Point, b: Point) -> Point {
    (a.0 - b.0, a.1 - b.1)
}

/// Orders directions counter-clockwise from +x without trigonometry, so it is exact.
fn ccw(a: Point, b: Point) -> Ordering {
    let half = |p: Point| p.1 < 0.0 || (p.1 == 0.0 && p.0 < 0.0);
    half(a).cmp(&half(b)).then_with(|| {
        let c = cross(a, b);
        if c > 0.0 {
            Ordering::Less
        } else if c < 0.0 {
            Ordering::Greater
        } else {
            Ordering::Equal
        }
    })
}

/// Joins `piece` to the lot of `lots` it shares the longest edge with, if any; false if none.
fn join(lots: &mut [Piece], piece: &Ring) -> bool {
    let pr = poly::bounds(piece);
    let mut best: Option<(f32, usize, Ring)> = None;
    for (j, p) in lots.iter().enumerate() {
        if !intersects(p.rect, pr) {
            continue;
        }
        let mut joined = poly::union(std::slice::from_ref(&p.ring), std::slice::from_ref(piece));
        // One piece with no hole: a piece wrapping around another lot can't join this one.
        if joined.len() != 1 || joined[0].len() != 1 {
            continue;
        }
        let ring = joined[0].swap_remove(0);
        let shared =
            (poly::perimeter(&p.ring) + poly::perimeter(piece) - poly::perimeter(&ring)) / 2.0;
        if shared > 0.1 && best.as_ref().is_none_or(|b| shared > b.0) {
            best = Some((shared, j, ring));
        }
    }
    let Some((_, j, ring)) = best else {
        return false;
    };
    lots[j].rect = poly::bounds(&ring);
    lots[j].ring = ring;
    true
}

/// `ring` without spikes and repeated points.
fn despike(mut ring: Vec<Point>) -> Vec<Point> {
    let mut i = 0;
    let mut since = 0;
    while ring.len() > 3 && since < ring.len() {
        let n = ring.len();
        let (p, c, q) = (ring[(i + n - 1) % n], ring[i % n], ring[(i + 1) % n]);
        let (a, b) = (sub(p, c), sub(q, c));
        let (la, lb) = (a.0.hypot(a.1), b.0.hypot(b.1));
        let sharp = la * lb < 1e-9
            || ((a.0 * b.0 + a.1 * b.1) > SPIKE_COS * la * lb
                && cross(a, b).abs() / 2.0 < SPIKE_AREA);
        if sharp {
            ring.remove(i % n);
            i = (i + n - 2) % (n - 1);
            since = 0;
        } else {
            i = (i + 1) % n;
            since += 1;
        }
    }
    ring
}

/// The part of polygon `pts` on the side of the line through `p` along `dir` that `keep`
/// (a direction from `p`) points to (Sutherland–Hodgman).
fn clip_half_plane(pts: &[Point], p: Point, dir: Point, keep: Point) -> Vec<Point> {
    let sign = cross(dir, keep).signum();
    let side = |q: Point| cross(dir, sub(q, p)) * sign;
    let mut out = Vec::with_capacity(pts.len() + 2);
    for i in 0..pts.len() {
        let (a, b) = (pts[i], pts[(i + 1) % pts.len()]);
        let (sa, sb) = (side(a), side(b));
        if sa >= 0.0 {
            out.push(a);
        }
        if (sa >= 0.0) != (sb >= 0.0) {
            out.push(lerp(a, b, sa / (sa - sb)));
        }
    }
    out
}

fn unit(p: Point) -> Point {
    let len = p.0.hypot(p.1).max(1e-9);
    (p.0 / len, p.1 / len)
}

/// Left of direction `d`.
fn left_of(d: Point) -> Point {
    (-d.1, d.0)
}

/// The offset direction where two edges with inward normals `n1` and `n2` meet: moving `d`
/// along it moves `d` away from both edges. Square to `n1` at dead ends and hairpins.
fn mitre(n1: Point, n2: Point) -> Point {
    let c = 1.0 + n1.0 * n2.0 + n1.1 * n2.1;
    if c < MIN_MITRE {
        n1
    } else {
        ((n1.0 + n2.0) / c, (n1.1 + n2.1) / c)
    }
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

/// A lot being cut, with its outline still on the polygon grid.
struct Piece {
    road: RoadId,
    left: bool,
    /// The stretch of road the lot fronts, metres from the road's start.
    span: (f32, f32),
    ring: Ring,
    rect: Rect,
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
    /// Traces the bounded faces of `net`. Needs current adjacency.
    pub fn find(net: &RoadNetwork) -> Self {
        let mut b = Self::default();
        let slots = net.lane_count() / 2;
        b.road_start.push(0);
        let active: Vec<bool> = {
            let mut v = vec![false; slots];
            for r in net.active_roads() {
                v[r as usize] = true;
            }
            v
        };
        for (r, &on) in active.iter().enumerate() {
            if on {
                let shape = net.shape(r as RoadId);
                let mut len = 0.0;
                b.pts.push(shape.a);
                b.arc.push(0.0);
                for (_, p, _, q) in shape.segments() {
                    len += dist(p, q);
                    b.pts.push(q);
                    b.arc.push(len);
                }
                let s = *b.road_start.last().unwrap() as usize;
                b.road_rect.push(rect_of(&b.pts[s..]));
            } else {
                b.road_rect.push([f32::MAX, f32::MAX, f32::MIN, f32::MIN]);
            }
            b.road_start.push(b.pts.len() as u32);
        }

        // next[l]: the lane that continues the face on the left of lane l, the sharpest left
        // turn where l ends. A dead end turns back along its own road.
        let mut next = vec![LaneId::MAX; net.lane_count()];
        let mut out: Vec<(Point, LaneId)> = Vec::new();
        for n in 0..net.node_count() {
            out.clear();
            out.extend(net.out_lanes(n as u32).iter().map(|&l| (b.leaving(l), l)));
            out.sort_by(|x, y| ccw(x.0, y.0).then(x.1.cmp(&y.1)));
            let k = out.len();
            for i in 0..k {
                next[(out[i].1 ^ 1) as usize] = out[(i + k - 1) % k].1;
            }
        }

        let mut prev = vec![LaneId::MAX; net.lane_count()];
        for (l, &n) in next.iter().enumerate() {
            if n != LaneId::MAX {
                prev[n as usize] = l as LaneId;
            }
        }
        let mut seen = vec![false; net.lane_count()];
        b.start.push(0);
        b.lane_start.push(0);
        for first in 0..net.lane_count() {
            if seen[first] || !active[first / 2] {
                continue;
            }
            let from = b.outline.len();
            let lanes_from = b.lanes.len();
            let mut lane = first;
            // Every lane is on exactly one face, so a walk never exceeds the lane count.
            for _ in 0..net.lane_count() {
                seen[lane] = true;
                b.lanes.push(lane as LaneId);
                let pts = b.lane_points(lane as LaneId);
                b.outline.extend_from_slice(&pts[..pts.len() - 1]);
                lane = next[lane] as usize;
                if lane == first || lane >= seen.len() {
                    break;
                }
            }
            let ring = &b.outline[from..];
            let area = (0..ring.len())
                .map(|i| cross(ring[i], ring[(i + 1) % ring.len()]))
                .sum::<f32>()
                / 2.0;
            if area > MIN_BLOCK_AREA {
                b.rect.push(rect_of(ring));
                b.area.push(area);
                b.start.push(b.outline.len() as u32);
                b.lane_start.push(b.lanes.len() as u32);
            } else {
                let pts = b.lane_points(first as LaneId);
                let (n, m) = (
                    left_of(unit(sub(pts[1], pts[0]))),
                    lerp(pts[0], pts[1], 0.5),
                );
                let probe = (m.0 + n.0 * 0.5, m.1 + n.1 * 0.5);
                b.outer.push((probe, b.lanes.split_off(lanes_from)));
                b.outline.truncate(from);
            }
        }
        b.next = next;
        b.prev = prev;
        b
    }

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

    pub fn contains(&self, i: usize, p: Point) -> bool {
        let r = self.rect[i];
        if !(p.0 >= r[0] && p.0 <= r[2] && p.1 >= r[1] && p.1 <= r[3]) {
            return false;
        }
        let ring = self.outline(i);
        let mut inside = false;
        for j in 0..ring.len() {
            let (a, b) = (ring[j], ring[(j + 1) % ring.len()]);
            if (a.1 > p.1) != (b.1 > p.1) && p.0 < a.0 + (p.1 - a.1) * (b.0 - a.0) / (b.1 - a.1) {
                inside = !inside;
            }
        }
        inside
    }

    /// The smallest block containing `p`, so blocks of a road network inside another block win.
    pub fn at(&self, p: Point) -> Option<usize> {
        (0..self.len())
            .filter(|&i| self.contains(i, p))
            .min_by(|&i, &j| self.area[i].total_cmp(&self.area[j]).then(i.cmp(&j)))
    }

    /// Block `i`'s cells: inside it, not inside a smaller block within it, and not under a road.
    pub(crate) fn cells(&self, i: usize, grid: &ZoneGrid) -> Vec<Cell> {
        let rect = self.rect[i];
        let nested = self.nested(i);
        let roads = self.roads_near(rect);
        let mut cells = Vec::new();
        for index in grid.cells_in(rect) {
            let at = grid.centre(index);
            if !self.contains(i, at) || nested.iter().any(|&j| self.contains(j, at)) {
                continue;
            }
            let Some(cell) = self.nearest(&roads, index, at) else {
                continue;
            };
            if cell.dist >= ROAD_HALF_WIDTH {
                cells.push(cell);
            }
        }
        cells
    }

    /// Smaller blocks inside block `i`'s bounds: candidates for blocks within it.
    fn nested(&self, i: usize) -> Vec<usize> {
        (0..self.len())
            .filter(|&j| {
                j != i
                    && self.area[j] < self.area[i]
                    && contains_rect(self.rect[i], self.rect[j], 0.0)
            })
            .collect()
    }

    fn roads_near(&self, rect: Rect) -> Vec<RoadId> {
        (0..self.road_rect.len() as RoadId)
            .filter(|&r| intersects(self.road_rect[r as usize], rect))
            .collect()
    }

    /// The road whose centreline passes nearest `p`, among roads whose bounds are within a
    /// metre of it.
    pub(crate) fn road_at(&self, p: Point) -> Option<RoadId> {
        let near = [p.0 - 1.0, p.1 - 1.0, p.0 + 1.0, p.1 + 1.0];
        let roads: Vec<RoadId> = (0..self.road_rect.len() as RoadId)
            .filter(|&r| intersects(self.road_rect[r as usize], near))
            .collect();
        self.nearest(&roads, 0, p).map(|c| c.road)
    }

    fn nearest(&self, roads: &[RoadId], index: u32, at: Point) -> Option<Cell> {
        let mut best: Option<Cell> = None;
        for &r in roads {
            let pts = self.road_points(r);
            for k in 0..pts.len() - 1 {
                let (a, b) = (pts[k], pts[k + 1]);
                let f = project(a, b, at);
                let d = dist(lerp(a, b, f), at);
                if best.is_none_or(|c| d < c.dist) {
                    best = Some(Cell {
                        index,
                        at,
                        dist: d,
                        road: r,
                    });
                }
            }
        }
        best
    }

    /// Cuts block `i` into lots (ADR 0017): each road side around it, in stretches of about
    /// `LOT_WIDTH`, runs straight back from the road to half way to the road behind (at most
    /// `LOT_REACH`), divided from its neighbours along corner bisectors and clipped to the
    /// land off the roads. Earlier lots keep land later ones would overlap. Slivers join the
    /// lot they share the longest edge with. Lots come out in (road, side, stretch) order.
    pub(crate) fn cut(&self, i: usize, grid: &ZoneGrid) -> Vec<LotPlan> {
        let rect = self.rect[i];
        let roads = self.roads_near(rect);
        let land = self.land(i, &roads);
        if land.is_empty() {
            return Vec::new();
        }
        let mut lanes =
            self.lanes[self.lane_start[i] as usize..self.lane_start[i + 1] as usize].to_vec();
        for (probe, face) in &self.outer {
            let pr = [probe.0, probe.1, probe.0, probe.1];
            if contains_rect(rect, pr, 0.0) && self.at(*probe) == Some(i) {
                lanes.extend(face);
            }
        }
        lanes.sort_unstable();
        lanes.dedup();
        let flanks = self.flanks(i);
        // Lots facing the fronts reach back to the facing front, past the flank roads and
        // roads beyond the block's corners.
        let rivals: Vec<RoadId> = if flanks.is_empty() {
            roads.clone()
        } else {
            let mut r: Vec<RoadId> = lanes
                .iter()
                .filter(|l| !flanks.contains(l))
                .map(|l| l / 2)
                .collect();
            r.dedup();
            r
        };
        let mut lots: Vec<Piece> = Vec::new();
        let mut slivers: Vec<Piece> = Vec::new();
        for &lane in &lanes {
            if flanks.contains(&lane) {
                continue;
            }
            let road = lane / 2;
            let edges = self.edges(lane, &flanks);
            let offsets = self.offsets(lane, &rivals, &edges, &flanks);
            for k in 0..edges.len() - 1 {
                let ribbon = self.ribbon(lane, &edges, k, &offsets, &flanks);
                let rr = poly::bounds(&ribbon);
                let mut pieces = poly::intersect(&[ribbon], &land);
                let near: Vec<Ring> = lots
                    .iter()
                    .map(|p| &p.ring)
                    .chain(slivers.iter().map(|p| &p.ring))
                    .filter(|r| intersects(poly::bounds(r), rr))
                    .cloned()
                    .collect();
                if !near.is_empty() && !pieces.is_empty() {
                    let flat: Vec<Ring> = pieces.into_iter().flatten().collect();
                    pieces = poly::subtract(&flat, &near);
                }
                for mut piece in pieces {
                    let ring = piece.swap_remove(0);
                    let big = poly::area(&ring) >= MIN_LOT_AREA;
                    let rect = poly::bounds(&ring);
                    let piece = Piece {
                        road,
                        left: lane.is_multiple_of(2),
                        span: (edges[k], edges[k + 1]),
                        ring,
                        rect,
                    };
                    if big {
                        lots.push(piece);
                    } else {
                        slivers.push(piece);
                    }
                }
            }
        }
        // Small pieces join a lot, retrying those that only touch other small pieces. When
        // none can join, the biggest left (such as a whole tiny block) becomes a lot itself.
        while !slivers.is_empty() {
            let before = slivers.len();
            slivers.retain(|s| !join(&mut lots, &s.ring));
            if slivers.len() == before {
                let big = (0..slivers.len())
                    .max_by(|&a, &b| {
                        poly::area(&slivers[a].ring).total_cmp(&poly::area(&slivers[b].ring))
                    })
                    .unwrap_or(0);
                lots.push(slivers.remove(big));
            }
        }
        // Gaps the lots leave, such as inside curves, join a lot too unless they are big
        // enough to be the block's interior.
        let all: Vec<Ring> = lots.iter().map(|p| p.ring.clone()).collect();
        for gap in poly::subtract(&land, &all) {
            if gap.len() == 1 && poly::area(&gap[0]) < MAX_GAP_AREA {
                join(&mut lots, &gap[0]);
            }
        }
        let mut claimed = BTreeSet::new();
        lots.into_iter()
            .map(|p| {
                let outline = despike(p.ring.iter().map(|&q| poly::to_point(q)).collect());
                let mut cells: Vec<u32> = grid
                    .cells_in(p.rect)
                    .filter(|&c| poly::contains(&outline, grid.centre(c)) && claimed.insert(c))
                    .collect();
                cells.sort_unstable();
                let (s0, s1) = p.span;
                let (front, dir) = self.point_at(p.road, (s0 + s1) / 2.0);
                // Facing the road: right of travel for lots on its left, and vice versa.
                let facing = if p.left {
                    (dir.1, -dir.0)
                } else {
                    (-dir.1, dir.0)
                };
                LotPlan {
                    road: p.road,
                    front,
                    facing,
                    frontage: s1 - s0,
                    outline,
                    cells,
                }
            })
            .collect()
    }

    /// Block `i`'s land off the roads: its outline without road strips and the blocks inside it.
    fn land(&self, i: usize, roads: &[RoadId]) -> Vec<Ring> {
        let outline = poly::ring(self.outline(i).iter().copied());
        let mut clip: Vec<Ring> = self
            .nested(i)
            .into_iter()
            .map(|j| poly::ring(self.outline(j).iter().copied()))
            .collect();
        // Just under half a road, so cells exactly `ROAD_HALF_WIDTH` away stay on the land.
        let h = ROAD_HALF_WIDTH - 0.01;
        for &r in roads {
            for w in self.road_points(r).windows(2) {
                let d = unit(sub(w[1], w[0]));
                let n = left_of(d);
                let (a, b) = (
                    (w[0].0 - d.0 * h, w[0].1 - d.1 * h),
                    (w[1].0 + d.0 * h, w[1].1 + d.1 * h),
                );
                clip.push(poly::ring([
                    (a.0 - n.0 * h, a.1 - n.1 * h),
                    (b.0 - n.0 * h, b.1 - n.1 * h),
                    (b.0 + n.0 * h, b.1 + n.1 * h),
                    (a.0 + n.0 * h, a.1 + n.1 * h),
                ]));
            }
        }
        poly::subtract(&[outline], &clip)
            .into_iter()
            .flatten()
            .collect()
    }

    /// Offsets back from `lane`'s side of its road: `(arc, point, unit direction, depth)` at
    /// the road's ends, its samples, its stretch ends and every `BACK_STEP` between, in order
    /// along the road. Each goes back as far as its land is nearer this road than any other
    /// (so lots from facing roads meet on one line), at most `LOT_REACH` from its road, and
    /// stops where it first crosses another offset of the same side, so lots on curves come
    /// to a point instead of folding over each other.
    fn offsets(
        &self,
        lane: LaneId,
        roads: &[RoadId],
        edges: &[f32],
        flanks: &[LaneId],
    ) -> Vec<(f32, Point, Point, f32)> {
        let road = lane / 2;
        let left = lane.is_multiple_of(2);
        let len = self.road_len(road);
        let start = self.road_start[road as usize] as usize;
        let pts = self.road_points(road);
        let arc = &self.arc[start..start + pts.len()];
        // Inward normal of the road piece from pts[j] to pts[j + 1].
        let normal = |j: usize| {
            let d = unit(sub(pts[j + 1], pts[j]));
            if left { left_of(d) } else { (d.1, -d.0) }
        };
        // Unit inward normal at each road point: the pieces' normals blended at curve points.
        let vertex: Vec<Point> = (0..pts.len())
            .map(|j| match j {
                0 => normal(0),
                j if j == pts.len() - 1 => normal(j - 1),
                j => unit(mitre(normal(j - 1), normal(j))),
            })
            .collect();
        // Where offsets start: the road's points, its stretch ends and every `BACK_STEP`.
        let mut at: Vec<f32> = arc.to_vec();
        at.extend_from_slice(&edges[1..edges.len() - 1]);
        at.sort_by(f32::total_cmp);
        at.dedup_by(|b, a| *b - *a <= 1e-3);
        let mut dense = Vec::with_capacity(at.len() * 4);
        for (i, &s0) in at.iter().enumerate() {
            dense.push(s0);
            if let Some(&s1) = at.get(i + 1) {
                let extra = ((s1 - s0) / BACK_STEP).ceil() as u32;
                dense.extend((1..extra).map(|e| s0 + (s1 - s0) * e as f32 / extra as f32));
            }
        }
        let last = dense.len() - 1;
        let dense = dense.into_iter().enumerate().map(|(i, s)| {
            if i == 0 || i == last {
                let s = if i == 0 { 0.0 } else { len };
                return (
                    s,
                    self.point_at(road, s).0,
                    self.offset_at(lane, s, Some(flanks)),
                );
            }
            let j = arc.partition_point(|&v| v <= s).clamp(1, arc.len() - 1);
            let f = ((s - arc[j - 1]) / (arc[j] - arc[j - 1]).max(1e-6)).clamp(0.0, 1.0);
            (
                s,
                self.point_at(road, s).0,
                unit(lerp(vertex[j - 1], vertex[j], f)),
            )
        });
        let mut out: Vec<(f32, Point, Point, f32)> = dense
            .map(|(s, p, m)| {
                let m_len = m.0.hypot(m.1);
                let u = (m.0 / m_len, m.1 / m_len);
                let reach = LOT_REACH * m_len;
                (s, p, u, self.nearer(roads, p, u, 1.0 / m_len).min(reach))
            })
            .collect();
        let full: Vec<f32> = out.iter().map(|o| o.3).collect();
        for a in 0..out.len() {
            for b in a + 1..out.len() {
                let ((_, p, u, _), (_, q, v, _)) = (out[a], out[b]);
                let denom = cross(u, v);
                if denom.abs() < 1e-6 {
                    continue;
                }
                let g = sub(q, p);
                let (ta, tb) = (cross(g, v) / denom, cross(g, u) / denom);
                if ta > 0.0 && tb > 0.0 && ta < full[a] && tb < full[b] {
                    out[a].3 = out[a].3.min(ta);
                    out[b].3 = out[b].3.min(tb);
                }
            }
        }
        out
    }

    /// The land stretch `k` (between `edges[k]` and `edges[k + 1]`) of `lane`'s side of its
    /// road could cover, from `offsets`: from the centreline back along each offset, within
    /// the bisectors of the corners at the road's ends, or along the flank roads there.
    fn ribbon(
        &self,
        lane: LaneId,
        edges: &[f32],
        k: usize,
        offsets: &[(f32, Point, Point, f32)],
        flanks: &[LaneId],
    ) -> Ring {
        let road = lane / 2;
        let len = self.road_len(road);
        // The end stretches run to the road's ends, past any margin along a flank.
        let s0 = if k == 0 { 0.0 } else { edges[k] };
        let s1 = if k + 2 == edges.len() {
            len
        } else {
            edges[k + 1]
        };
        let span: Vec<_> = offsets
            .iter()
            .filter(|o| o.0 >= s0 - 1e-3 && o.0 <= s1 + 1e-3)
            .collect();
        let back = span
            .iter()
            .rev()
            .map(|&&(_, p, u, t)| (p.0 + u.0 * t, p.1 + u.1 * t));
        let mut pts: Vec<Point> = span.iter().map(|o| o.1).chain(back).collect();
        // Land past either corner's bisector belongs to the road side around that corner.
        for at in [0.0, len] {
            let (p, d) = self.point_at(road, at);
            let m = self.offset_at(lane, at, Some(flanks));
            if m != self.offset_at(lane, at, None) {
                let keep = if at == 0.0 { d } else { (-d.0, -d.1) };
                pts = clip_half_plane(&pts, p, m, keep);
            }
        }
        poly::ring(pts)
    }

    /// The offset direction of `lane`'s side of its road `s` metres along it, scaled so it
    /// moves one metre away from the road per unit: at a road end (`flanks` given), along
    /// the flank road there if the lane around the corner is one of `flanks`, else along the
    /// corner bisector; elsewhere square to the road.
    fn offset_at(&self, lane: LaneId, s: f32, flanks: Option<&[LaneId]>) -> Point {
        let road = lane / 2;
        let left = lane.is_multiple_of(2);
        let (_, d) = self.point_at(road, s);
        let n1 = if left { left_of(d) } else { (d.1, -d.0) };
        let Some(flanks) = flanks else {
            return n1;
        };
        let Some((other, along, n2)) = self.around(lane, s <= 0.0) else {
            return n1;
        };
        if flanks.contains(&other) {
            let c = along.0 * n1.0 + along.1 * n1.1;
            if c > MIN_MITRE {
                return (along.0 / c, along.1 / c);
            }
        }
        mitre(n1, n2)
    }

    /// The lane around the block from `lane` at its road's start (`at_start`) or end, the unit
    /// direction along it away from that corner, and its unit inward normal there. `None` at
    /// a dead end.
    fn around(&self, lane: LaneId, at_start: bool) -> Option<(LaneId, Point, Point)> {
        // Lane `lane` travels a→b on the left side and b→a on the right; the road start is
        // where the left side's lane starts and the right side's lane ends.
        if at_start != lane.is_multiple_of(2) {
            let m = self.next[lane as usize];
            (m != LaneId::MAX).then(|| {
                let d = unit(self.leaving(m));
                (m, d, left_of(d))
            })
        } else {
            let p = self.prev[lane as usize];
            (p != LaneId::MAX).then(|| {
                let d = unit(self.leaving(p ^ 1));
                (p, d, left_of((-d.0, -d.1)))
            })
        }
    }

    /// Where `lane`'s stretches start and end, metres along its road from the road's start:
    /// about `LOT_WIDTH` apart along the road, or along the kerb where the road meets one of
    /// `flanks`, so lots there are as wide as the rest.
    fn edges(&self, lane: LaneId, flanks: &[LaneId]) -> Vec<f32> {
        let len = self.road_len(lane / 2);
        let margin = |at_start: bool| {
            let s = if at_start { 0.0 } else { len };
            match self.around(lane, at_start) {
                Some((other, along, _)) if flanks.contains(&other) => {
                    let (_, d) = self.point_at(lane / 2, s);
                    let sin = cross(d, along).abs().max(0.5);
                    ROAD_HALF_WIDTH / sin
                }
                _ => 0.0,
            }
        };
        let (m0, m1) = (margin(true), margin(false));
        let usable = (len - m0 - m1).max(1.0);
        let n = ((usable / LOT_WIDTH).round() as u32).max(1);
        let w = usable / n as f32;
        let mut edges = vec![0.0];
        edges.extend((1..n).map(|k| m0 + k as f32 * w));
        edges.push(len);
        edges
    }

    /// Lanes around block `i` that get no lots: in a four-sided block with corners between 60°
    /// and 120° and straight sides, the shorter pair of opposite sides (the more north-south
    /// pair if they are about equal), so lots face the other pair and reach back to the middle
    /// as rectangles. Empty for other blocks, or when lots couldn't reach across.
    fn flanks(&self, i: usize) -> Vec<LaneId> {
        let ring = &self.lanes[self.lane_start[i] as usize..self.lane_start[i + 1] as usize];
        let ends: Vec<(Point, Point, Point, Point)> = ring
            .iter()
            .map(|&l| {
                let p = self.lane_points(l);
                let n = p.len();
                (
                    p[0],
                    unit(sub(p[1], p[0])),
                    p[n - 1],
                    unit(sub(p[n - 1], p[n - 2])),
                )
            })
            .collect();
        let dot = |a: Point, b: Point| a.0 * b.0 + a.1 * b.1;
        let n = ring.len();
        // A corner is where the next lane turns more than 30° from the one before it.
        let corners: Vec<usize> = (0..n)
            .filter(|&j| dot(ends[(j + n - 1) % n].3, ends[j].1) < STRAIGHT_COS)
            .collect();
        if corners.len() != 4
            || corners
                .iter()
                .any(|&j| dot(ends[(j + n - 1) % n].3, ends[j].1).abs() > RIGHT_COS)
        {
            return Vec::new();
        }
        let mut sides = Vec::with_capacity(4);
        for c in 0..4 {
            let (from, to) = (corners[c], corners[(c + 1) % 4]);
            let lanes: Vec<usize> = (0..n)
                .map(|k| (from + k) % n)
                .take((to + n - from) % n)
                .collect();
            let (first, last) = (lanes[0], lanes[lanes.len() - 1]);
            // A side that bends, along a curve or at a junction, isn't straight.
            if lanes
                .iter()
                .any(|&j| dot(ends[j].1, ends[j].3) < STRAIGHT_COS)
                || dot(ends[first].1, ends[last].3) < STRAIGHT_COS
            {
                return Vec::new();
            }
            let len: f32 = lanes.iter().map(|&j| self.road_len(ring[j] / 2)).sum();
            sides.push((lanes, len, sub(ends[last].2, ends[first].0)));
        }
        let (a, b) = (sides[0].1 + sides[2].1, sides[1].1 + sides[3].1);
        let chord = sides[0].2;
        let flank = if a > b * 1.05 {
            1
        } else if b > a * 1.05 || chord.1.abs() > chord.0.abs() {
            0
        } else {
            1
        };
        let reach = 2.0 * (LOT_REACH + ROAD_HALF_WIDTH);
        if sides[flank].1 > reach || sides[flank + 2].1 > reach {
            return Vec::new();
        }
        let mut out: Vec<LaneId> = sides[flank]
            .0
            .iter()
            .chain(&sides[flank + 2].0)
            .map(|&j| ring[j])
            .collect();
        out.sort_unstable();
        out
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

    /// Straight roads that would cut block `i` through its middle, best first, each with
    /// whether it is an alley (along the block) rather than a connector street (across it).
    /// An alley is chosen when the block is more than twice as long as it is wide.
    pub(crate) fn cuts(&self, i: usize, cells: &[Cell], grid: &ZoneGrid) -> Vec<(Shape, bool)> {
        if cells.is_empty() {
            return Vec::new();
        }
        // Principal axes of the block's cells, in f64 so large coordinates stay accurate.
        let n = cells.len() as f64;
        let mean = cells.iter().fold((0.0, 0.0), |m, c| {
            (m.0 + c.at.0 as f64 / n, m.1 + c.at.1 as f64 / n)
        });
        let (mut sxx, mut sxy, mut syy) = (0.0, 0.0, 0.0);
        for c in cells {
            let (dx, dy) = (c.at.0 as f64 - mean.0, c.at.1 as f64 - mean.1);
            sxx += dx * dx;
            sxy += dx * dy;
            syy += dy * dy;
        }
        let big = (sxx + syy) / 2.0 + (((sxx - syy) / 2.0).powi(2) + sxy * sxy).sqrt();
        let u = if sxy.abs() > 1e-9 {
            (sxy, big - sxx)
        } else if sxx >= syy {
            (1.0, 0.0)
        } else {
            (0.0, 1.0)
        };
        let len = u.0.hypot(u.1);
        let u = (u.0 / len, u.1 / len);
        let v = (-u.1, u.0);
        let extent = |axis: (f64, f64)| {
            cells.iter().fold((f64::MAX, f64::MIN), |(lo, hi), c| {
                let t = (c.at.0 as f64 - mean.0) * axis.0 + (c.at.1 as f64 - mean.1) * axis.1;
                (lo.min(t), hi.max(t))
            })
        };
        let ((u0, u1), (v0, v1)) = (extent(u), extent(v));
        let mid = (
            mean.0 + u.0 * (u0 + u1) / 2.0 + v.0 * (v0 + v1) / 2.0,
            mean.1 + u.1 * (u0 + u1) / 2.0 + v.1 * (v0 + v1) / 2.0,
        );
        let centre = cells
            .iter()
            .min_by(|a, b| {
                let d = |c: &Cell| (c.at.0 as f64 - mid.0).hypot(c.at.1 as f64 - mid.1);
                d(a).total_cmp(&d(b))
            })
            .map(|c| c.at);
        let deepest = cells
            .iter()
            .filter(|c| c.dist > LOT_REACH && grid.zone(c.index) != Zone::None)
            .max_by(|a, b| a.dist.total_cmp(&b.dist).then(b.index.cmp(&a.index)))
            .map(|c| c.at);
        let (u, v) = ((u.0 as f32, u.1 as f32), (v.0 as f32, v.1 as f32));
        let dirs = if u1 - u0 > 2.0 * (v1 - v0) {
            [(u, true), (v, false)]
        } else {
            [(v, false), (u, true)]
        };
        let roads: Vec<RoadId> = (0..self.road_rect.len() as RoadId)
            .filter(|&r| intersects(self.road_rect[r as usize], self.rect[i]))
            .collect();
        let mut out = Vec::new();
        for anchor in [centre, deepest].into_iter().flatten() {
            for (d, alley) in dirs {
                let hit = |d: Point| self.ray(&roads, anchor, d);
                if let (Some(a), Some(b)) = (hit((-d.0, -d.1)), hit(d)) {
                    out.push((
                        Shape {
                            a,
                            b,
                            control: None,
                        },
                        alley,
                    ));
                }
            }
        }
        out
    }

    /// How far a ray from `p` along unit `u` goes before a point on it is as near one of
    /// `roads` as it is to the road through `p`, whose distance grows `k` metres per metre
    /// along the ray. `f32::MAX` if never.
    fn nearer(&self, roads: &[RoadId], p: Point, u: Point, k: f32) -> f32 {
        let mut best = f32::MAX;
        let mut try_t = |t: f32, a: Point, b: Point| {
            if t > 1e-3 && t < best {
                let q = (p.0 + u.0 * t, p.1 + u.1 * t);
                let d = dist(q, lerp(a, b, project(a, b, q)));
                if (d - k * t).abs() <= 1e-3 * t.max(1.0) {
                    best = t;
                }
            }
        };
        for &r in roads {
            for w in self.road_points(r).windows(2) {
                let (a, b) = (w[0], w[1]);
                let len = dist(a, b);
                if len < 1e-6 {
                    continue;
                }
                // The segment's line: n·(p + u t - a) = ±k t.
                let n = left_of(((b.0 - a.0) / len, (b.1 - a.1) / len));
                let (c, nu) = (n.0 * (p.0 - a.0) + n.1 * (p.1 - a.1), n.0 * u.0 + n.1 * u.1);
                for sign in [1.0, -1.0] {
                    let den = sign * k - nu;
                    // Near zero, the ray runs along the line's bisector: no crossing.
                    if den.abs() > 1e-4 {
                        try_t(c / den, a, b);
                    }
                }
                // Its ends: |p + u t - e|² = k² t².
                for e in [a, b] {
                    let g = sub(p, e);
                    let (qa, qb, qc) = (1.0 - k * k, u.0 * g.0 + u.1 * g.1, g.0 * g.0 + g.1 * g.1);
                    if qa.abs() < 1e-6 {
                        if qb < 0.0 {
                            try_t(-qc / (2.0 * qb), a, b);
                        }
                    } else {
                        let disc = qb * qb - qa * qc;
                        if disc >= 0.0 {
                            let r = disc.sqrt();
                            try_t((-qb - r) / qa, a, b);
                            try_t((-qb + r) / qa, a, b);
                        }
                    }
                }
            }
        }
        best
    }

    /// Where a ray from `p` along `d` first meets one of `roads`.
    fn ray(&self, roads: &[RoadId], p: Point, d: Point) -> Option<Point> {
        self.ray_from(roads, p, d, 0.0)
            .map(|t| (p.0 + d.0 * t, p.1 + d.1 * t))
    }

    /// How far along unit `d` a ray from `p` first meets one of `roads`, past `min` metres.
    fn ray_from(&self, roads: &[RoadId], p: Point, d: Point, min: f32) -> Option<f32> {
        let mut best = f32::MAX;
        for &r in roads {
            for w in self.road_points(r).windows(2) {
                let e = sub(w[1], w[0]);
                let denom = cross(d, e);
                if denom.abs() < 1e-9 {
                    continue;
                }
                let g = sub(w[0], p);
                let (t, s) = (cross(g, e) / denom, cross(g, d) / denom);
                if t > min && (0.0..=1.0).contains(&s) && t < best {
                    best = t;
                }
            }
        }
        (best < f32::MAX).then_some(best)
    }
}
