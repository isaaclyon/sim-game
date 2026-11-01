//! Blocks: the land enclosed by roads, and how it is cut into lots (ADR 0016).
//!
//! Blocks are derived from the road graph every time they are needed and have no stable ids.
//! A block is a bounded face of the graph, traced along the sampled road centrelines.

use std::cmp::Ordering;
use std::collections::BTreeMap;

use crate::geometry::{Point, Shape, dist, lerp, project};
use crate::road::{LaneId, RoadId, RoadNetwork};
use crate::zone::{Zone, ZoneGrid};

/// Half a road's width: cells whose centre is closer to a centreline are under the road.
pub const ROAD_HALF_WIDTH: f32 = 4.0;
/// Cells farther than this from every road centreline are a block's interior and get no lot.
pub const LOT_REACH: f32 = 64.0;
/// Target frontage of one lot along its road, metres.
pub const LOT_WIDTH: f32 = 16.0;
/// Smaller groups of cells are leftovers, not lots.
const MIN_LOT_CELLS: usize = 2;
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

/// The cells sharing an edge with row-major cell `c` of a grid `cols` wide.
fn neighbours(c: u32, cols: u32) -> impl Iterator<Item = u32> {
    [
        c.checked_sub(cols),
        (!c.is_multiple_of(cols)).then(|| c - 1),
        (c % cols != cols - 1).then_some(c + 1),
        Some(c + cols),
    ]
    .into_iter()
    .flatten()
}

/// Splits `cells` into pieces whose cells connect through shared edges, each ordered by cell
/// index, pieces ordered by their lowest cell.
fn pieces(mut cells: Vec<Cell>, cols: u32) -> Vec<Vec<Cell>> {
    cells.sort_unstable_by_key(|c| c.index);
    let mut piece_of = vec![usize::MAX; cells.len()];
    let find = |i: u32| cells.binary_search_by_key(&i, |c| c.index).ok();
    let mut out: Vec<Vec<Cell>> = Vec::new();
    for start in 0..cells.len() {
        if piece_of[start] != usize::MAX {
            continue;
        }
        let p = out.len();
        piece_of[start] = p;
        let mut stack = vec![start];
        let mut piece = Vec::new();
        while let Some(k) = stack.pop() {
            piece.push(cells[k]);
            for n in neighbours(cells[k].index, cols).filter_map(find) {
                if piece_of[n] == usize::MAX {
                    piece_of[n] = p;
                    stack.push(n);
                }
            }
        }
        piece.sort_unstable_by_key(|c| c.index);
        out.push(piece);
    }
    out
}

/// One zone cell of a block, with the road it is closest to.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Cell {
    pub index: u32,
    pub at: Point,
    /// Distance to the nearest road centreline.
    pub dist: f32,
    pub road: RoadId,
    /// Whether the cell is left of the road's a→b direction.
    pub left: bool,
    /// Distance along the road to the point nearest the cell.
    pub arc: f32,
}

/// A lot about to be added: see `Lots`.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct LotPlan {
    pub road: RoadId,
    pub front: Point,
    pub facing: Point,
    /// Row-major cell indices, ascending.
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

        let mut seen = vec![false; net.lane_count()];
        b.start.push(0);
        for first in 0..net.lane_count() {
            if seen[first] || !active[first / 2] {
                continue;
            }
            let from = b.outline.len();
            let mut lane = first;
            // Every lane is on exactly one face, so a walk never exceeds the lane count.
            for _ in 0..net.lane_count() {
                seen[lane] = true;
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
            } else {
                b.outline.truncate(from);
            }
        }
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
        let nested: Vec<usize> = (0..self.len())
            .filter(|&j| {
                j != i && self.area[j] < self.area[i] && contains_rect(rect, self.rect[j], 0.0)
            })
            .collect();
        let roads: Vec<RoadId> = (0..self.road_rect.len() as RoadId)
            .filter(|&r| intersects(self.road_rect[r as usize], rect))
            .collect();
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
            let s = self.road_start[r as usize] as usize;
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
                        left: cross(sub(b, a), sub(at, a)) > 0.0,
                        arc: self.arc[s + k] + f * (self.arc[s + k + 1] - self.arc[s + k]),
                    });
                }
            }
        }
        best
    }

    /// Cuts block cells into lots: each road side within reach, in stretches of about
    /// `LOT_WIDTH`. Every lot is one piece: a stretch whose cells fall apart becomes one lot per
    /// piece, and pieces too small to be lots join a lot they touch, so corners are not left
    /// empty. Lots come out in (road, side, stretch, lowest cell) order.
    pub(crate) fn cut(&self, cells: &[Cell], cols: u32) -> Vec<LotPlan> {
        let mut stretches: BTreeMap<(RoadId, bool, u32), Vec<Cell>> = BTreeMap::new();
        for c in cells.iter().filter(|c| c.dist <= LOT_REACH) {
            let (n, w) = self.stretches(c.road);
            let k = ((c.arc / w) as u32).min(n - 1);
            stretches.entry((c.road, c.left, k)).or_default().push(*c);
        }
        let mut lots: Vec<(RoadId, bool, Vec<Cell>)> = Vec::new();
        let mut loose: Vec<u32> = Vec::new();
        for ((road, left, _), group) in stretches {
            for piece in pieces(group, cols) {
                if piece.len() >= MIN_LOT_CELLS {
                    lots.push((road, left, piece));
                } else {
                    loose.extend(piece.iter().map(|c| c.index));
                }
            }
        }
        let mut owner: BTreeMap<u32, usize> = BTreeMap::new();
        let mut joined: Vec<Vec<u32>> = vec![Vec::new(); lots.len()];
        for (l, (_, _, piece)) in lots.iter().enumerate() {
            owner.extend(piece.iter().map(|c| (c.index, l)));
        }
        loose.sort_unstable();
        // Each pass attaches the loose cells touching a lot; a few passes reach any corner.
        for _ in 0..4 {
            let mut attach = Vec::new();
            loose.retain(|&c| match neighbours(c, cols).find_map(|n| owner.get(&n)) {
                Some(&l) => {
                    attach.push((c, l));
                    false
                }
                None => true,
            });
            if attach.is_empty() {
                break;
            }
            for (c, l) in attach {
                owner.insert(c, l);
                joined[l].push(c);
            }
        }
        lots.into_iter()
            .zip(joined)
            .map(|((road, left, piece), extra)| {
                // The front is where the piece's own cells meet the road, on average.
                let arc = piece.iter().map(|c| c.arc).sum::<f32>() / piece.len() as f32;
                let (front, dir) = self.point_at(road, arc);
                // Facing the road: right of travel for lots on its left, and vice versa.
                let facing = if left {
                    (dir.1, -dir.0)
                } else {
                    (-dir.1, dir.0)
                };
                let mut cells: Vec<u32> = piece.iter().map(|c| c.index).chain(extra).collect();
                cells.sort_unstable();
                LotPlan {
                    road,
                    front,
                    facing,
                    cells,
                }
            })
            .collect()
    }

    /// How many stretches road `r`'s sides are cut into, and how long each is.
    fn stretches(&self, r: RoadId) -> (u32, f32) {
        let len = self.arc[self.road_start[r as usize + 1] as usize - 1];
        let n = ((len / LOT_WIDTH).round() as u32).max(1);
        (n, len / n as f32)
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

    /// Where a ray from `p` along `d` first meets one of `roads`.
    fn ray(&self, roads: &[RoadId], p: Point, d: Point) -> Option<Point> {
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
                if t > 0.0 && (0.0..=1.0).contains(&s) && t < best {
                    best = t;
                }
            }
        }
        (best < f32::MAX).then_some((p.0 + d.0 * best, p.1 + d.1 * best))
    }
}
