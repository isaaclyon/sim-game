//! Drawing guides (ADR 0038): where a clicked point lands once angle, guide-line and length
//! snapping apply. Read-only, like `snap::plan`, which still joins junctions and roads after.
//! Angle steps are fixed unit vectors, so guided points need no trigonometry.

use std::f32::consts::FRAC_1_SQRT_2;

use crate::geometry::{Point, Shape, dist};
use crate::road::RoadNetwork;
use crate::snap::{self, Join, SNAP_RADIUS};
use crate::zone::ZONE_CELL;

/// Snap flags, one bit per kind. Joining junctions and roads is always on.
pub const ANGLE: u8 = 1;
pub const GUIDES: u8 = 2;
pub const LENGTH: u8 = 4;
/// Continue a curve using the previous control point in the third raw point.
pub const CONTINUE: u8 = 8;
pub const ALL: u8 = ANGLE | GUIDES | LENGTH;

/// Road ends further than this from the cursor offer no guide lines, metres.
const GUIDE_RANGE: f32 = 400.0;
/// A guide line captures the cursor within this distance, metres, unless the request asks for
/// more (a zoomed-out view; see `draft_within`).
const GUIDE_SNAP: f32 = 6.0;
/// Lengths snap to whole zone cells.
const LENGTH_STEP: f32 = ZONE_CELL;
/// Lines closer to parallel than this (sine of the angle between them) do not meet.
const PARALLEL_SIN: f32 = 0.2;
/// Cosine and sine of 0°, 15°, … 75°; the other quadrants follow by rotation.
const QUADRANT: [Point; 6] = [
    (1.0, 0.0),
    (0.965_925_8, 0.258_819_04),
    (0.866_025_4, 0.5),
    (FRAC_1_SQRT_2, FRAC_1_SQRT_2),
    (0.5, 0.866_025_4),
    (0.258_819_04, 0.965_925_8),
];
/// Largest angular miss that still snaps, as a sine: about 8° for right angles, 5° for 45°
/// steps and 3° for other 15° steps.
const TOLERANCE: [f32; 3] = [0.139, 0.087, 0.052];

/// The guided points of a road being drawn, and what guided the last one.
#[derive(Clone, Debug, Default)]
pub struct Draft {
    /// Guided points in order; continuation replaces its source control with a new control.
    pub points: Vec<(f32, f32)>,
    /// `[x0, y0, x1, y1]` per guide line used for the last point, from its origin to the point.
    pub lines: Vec<[f32; 4]>,
    /// Metres along the road drawn so far: straight for two points, the curve for three.
    pub length: f32,
    /// Degrees between the road and the nearest road its start joins, for two points; NaN
    /// when the start joins no road. Display only.
    pub angle: f32,
    /// The angle the road makes where its start joins a road, once it has a second point.
    pub start_mark: Option<Mark>,
    /// The angle where its last point joins a road, for two or three points. Display only.
    pub end_mark: Option<Mark>,
}

/// An angle drawn on the map: `[x, y, rx, ry, dx, dy]`, the junction, then unit directions
/// away from it along the road measured from and along the new road.
pub type Mark = [f32; 6];

#[derive(Clone, Copy)]
struct Line {
    origin: Point,
    dir: Point,
    /// Distance from the cursor, and the distance at which the line lets go of it.
    off: f32,
    reach: f32,
}

/// Guides `[x, y, snap]` points in drawing order: start, then a curve's control, then the end.
/// Each point is guided from the guided points before it. With `CONTINUE` on the first
/// point, the three inputs are start, end and previous control. The result is start, new
/// control and end, or no points if the source direction or projection is invalid.
pub fn draft(net: &RoadNetwork, raw: &[(f32, f32, u8)]) -> Draft {
    draft_within(net, raw, 0.0)
}

/// `draft` with a snap `reach` in metres: about ten screen pixels at the player's zoom. Guide
/// lines and junctions capture the cursor within it (never less than their own 6 m and 15 m),
/// and an angle step within it, scaled down for steps other than right angles, even where the
/// road is too short for the angle tolerance. Non-finite or nonpositive reaches add nothing;
/// reaches past `GUIDE_RANGE` count as that.
pub fn draft_within(net: &RoadNetwork, raw: &[(f32, f32, u8)], reach: f32) -> Draft {
    let reach = if reach.is_finite() {
        reach.clamp(0.0, GUIDE_RANGE)
    } else {
        0.0
    };
    let continuation = raw.first().is_some_and(|p| p.2 & CONTINUE != 0);
    if continuation && raw.len() != 3 {
        return Draft::default();
    }
    let mut points = Vec::with_capacity(raw.len());
    let mut lines = Vec::new();
    let guided = if continuation { &raw[..2] } else { raw };
    for &(x, y, snap) in guided {
        let (p, l) = guide(net, &points, (x, y), snap, reach);
        points.push(p);
        lines = l;
    }
    if continuation {
        // The planner snaps endpoints again, including guide crossings onto curved roads.
        let stop = snap::snap_end(net, points[1].0, points[1].1);
        let end = (stop.x, stop.y);
        if end != points[1] {
            points[1] = end;
            lines.clear();
        }
        let start = points[0];
        let previous = (raw[2].0, raw[2].1);
        let direction = (start.0 - previous.0, start.1 - previous.1);
        let length = direction.0.hypot(direction.1);
        if !length.is_finite() || length < 1e-6 {
            return Draft::default();
        }
        let t = (direction.0 / length, direction.1 / length);
        let matches = net.active_roads().any(|road| {
            let shape = net.shape(road);
            let incoming = if shape.b == start {
                shape.tangent(1.0)
            } else if shape.a == start {
                let t = shape.tangent(0.0);
                (-t.0, -t.1)
            } else {
                return false;
            };
            let len = incoming.0.hypot(incoming.1);
            if !len.is_finite() || len < 1e-6 {
                return false;
            }
            let incoming = (incoming.0 / len, incoming.1 / len);
            dot(t, incoming) > 0.0 && cross(t, incoming).abs() < 1e-4
        });
        let projection = dot((end.0 - start.0, end.1 - start.1), t);
        if !matches || !projection.is_finite() || projection <= 0.0 {
            return Draft::default();
        }
        points.insert(
            1,
            (
                start.0 + t.0 * projection * 0.5,
                start.1 + t.1 * projection * 0.5,
            ),
        );
    }
    let length = match points[..] {
        [a, b] => dist(a, b),
        [a, c, b] => {
            let shape = Shape {
                a,
                control: Some(c),
                b,
            };
            if shape.valid() {
                shape.lengths().last().copied().unwrap_or(0.0)
            } else {
                dist(a, b)
            }
        }
        _ => 0.0,
    };
    // A curve leaves its start toward the control point and meets its end from it.
    let (start_mark, end_mark) = match points[..] {
        [a, b] => (mark(net, a, b), mark(net, b, a)),
        [a, c, b] => (mark(net, a, c), mark(net, b, c)),
        _ => (None, None),
    };
    let angle = match (&points[..], start_mark) {
        ([_, _], Some(m)) => angle_of(m),
        _ => f32::NAN,
    };
    Draft {
        points,
        lines,
        length,
        angle,
        start_mark,
        end_mark,
    }
}

fn guide(
    net: &RoadNetwork,
    before: &[Point],
    cursor: Point,
    snap: u8,
    reach: f32,
) -> (Point, Vec<[f32; 4]>) {
    if !cursor.0.is_finite() || !cursor.1.is_finite() {
        return (cursor, Vec::new());
    }
    // A cursor on a junction keeps it, as without guides.
    if let Some(n) = net.nearest_junction(cursor.0, cursor.1, SNAP_RADIUS.max(reach)) {
        return ((net.node_x[n as usize], net.node_y[n as usize]), Vec::new());
    }
    let anchor = before.last().copied();
    let mut lines = Vec::new();
    if snap & ANGLE != 0
        && let Some(a) = anchor
    {
        let refs = match before {
            [.., p, c] => vec![unit((p.0 - c.0, p.1 - c.1))],
            _ => joined_roads(net, a),
        };
        let refs = if refs.is_empty() {
            vec![(1.0, 0.0)]
        } else {
            refs
        };
        lines.extend(angle_line(a, &refs, cursor, reach));
    }
    if snap & GUIDES != 0 {
        alignment_lines(net, anchor, cursor, GUIDE_SNAP.max(reach), &mut lines);
    }
    // Stable, so equal candidates keep road order.
    lines.sort_by(|l, m| (l.off / l.reach).total_cmp(&(m.off / m.reach)));
    let mut point = cursor;
    let mut used: Vec<Line> = Vec::new();
    if let Some(&first) = lines.first() {
        point = project(first, cursor);
        used.push(first);
        let mut best: Option<(Point, Line, f32)> = None;
        for &l in &lines[1..] {
            if let Some(x) = meet(first, l) {
                let d = dist(x, cursor);
                if d <= first.reach.max(l.reach) && best.is_none_or(|b| d < b.2) {
                    best = Some((x, l, d));
                }
            }
        }
        if let Some((x, l, _)) = best {
            point = x;
            used.push(l);
        }
    }
    if snap & LENGTH != 0
        && let Some(a) = anchor
        && used.len() < 2
        && used.first().is_none_or(|l| l.origin == a)
    {
        let len = dist(a, point);
        if len > 1e-3 {
            let snapped = (len / LENGTH_STEP).round().max(1.0) * LENGTH_STEP;
            let k = snapped / len;
            point = (a.0 + (point.0 - a.0) * k, a.1 + (point.1 - a.1) * k);
        }
    }
    // The planner joins what the point lands near; keep a guide where it crosses that road.
    let stop = snap::snap_end(net, point.0, point.1);
    match stop.join {
        Join::New => {}
        Join::Node(_) => {
            point = (stop.x, stop.y);
            used.clear();
        }
        Join::Split(road) => {
            match used
                .first()
                .and_then(|&l| crossing(net.shape(road), l, point))
            {
                Some(x) => {
                    point = x;
                    used.truncate(1);
                }
                None => {
                    point = (stop.x, stop.y);
                    used.clear();
                }
            }
        }
    }
    let drawn = used
        .iter()
        .map(|l| [l.origin.0, l.origin.1, point.0, point.1])
        .collect();
    (point, drawn)
}

/// The 15° step from `a` closest to the cursor, within its tolerance: an angle, or `floor`
/// metres for right angles, less for other steps in the same proportion.
fn angle_line(a: Point, refs: &[Point], cursor: Point, floor: f32) -> Option<Line> {
    let v = (cursor.0 - a.0, cursor.1 - a.1);
    let len = dist(a, cursor);
    if len < 1.0 {
        return None;
    }
    let mut best: Option<Line> = None;
    for &r in refs {
        for k in 0..24 {
            let d = rotate(r, k);
            if dot(v, d) <= 0.0 {
                continue;
            }
            let off = cross(v, d).abs();
            let tolerance = TOLERANCE[if k % 6 == 0 {
                0
            } else if k % 3 == 0 {
                1
            } else {
                2
            }];
            let reach = (len * tolerance).max(floor * tolerance / TOLERANCE[0]);
            if off <= reach && best.is_none_or(|b| off < b.off) {
                best = Some(Line {
                    origin: a,
                    dir: d,
                    off,
                    reach,
                });
            }
        }
    }
    best
}

/// Extensions of road ends past their junction, and lines square to them, near the cursor.
fn alignment_lines(
    net: &RoadNetwork,
    anchor: Option<Point>,
    cursor: Point,
    snap: f32,
    out: &mut Vec<Line>,
) {
    for road in net.active_roads() {
        let shape = net.shape(road);
        let t0 = shape.tangent(0.0);
        for (end, outward) in [(shape.a, (-t0.0, -t0.1)), (shape.b, shape.tangent(1.0))] {
            if dist(end, cursor) > GUIDE_RANGE || anchor.is_some_and(|a| dist(a, end) < 0.5) {
                continue;
            }
            let outward = unit(outward);
            let v = (cursor.0 - end.0, cursor.1 - end.1);
            let square = (-outward.1, outward.0);
            for (dir, ahead_only) in [(outward, true), (square, false)] {
                let off = cross(v, dir).abs();
                if off < snap && (!ahead_only || dot(v, dir) > 0.0) {
                    out.push(Line {
                        origin: end,
                        dir,
                        off,
                        reach: snap,
                    });
                }
            }
        }
    }
}

/// Unit directions from `at` along every road it joins, pointing away from it.
fn joined_roads(net: &RoadNetwork, at: Point) -> Vec<Point> {
    match snap::snap_end(net, at.0, at.1).join {
        Join::New => Vec::new(),
        Join::Node(n) => net
            .active_roads()
            .filter_map(|road| {
                let (a, b) = net.road_ends(road);
                let shape = net.shape(road);
                if a == n {
                    Some(unit(shape.tangent(0.0)))
                } else if b == n {
                    let t = shape.tangent(1.0);
                    Some(unit((-t.0, -t.1)))
                } else {
                    None
                }
            })
            .collect(),
        Join::Split(road) => {
            let shape = net.shape(road);
            let t = unit(shape.tangent(shape.nearest(at)));
            vec![t, (-t.0, -t.1)]
        }
    }
}

/// The angle at `at` between the road heading toward `toward` and the road it joins there.
/// At a junction of roads that are not square to each other, it is measured from the road the
/// snap lined up with: the one nearest a whole 15° step, then the smallest angle.
fn mark(net: &RoadNetwork, at: Point, toward: Point) -> Option<Mark> {
    if dist(at, toward) < 1e-3 {
        return None;
    }
    let d = unit((toward.0 - at.0, toward.1 - at.1));
    let off_step = |deg: f32| (deg - (deg / 15.0).round() * 15.0).abs();
    joined_roads(net, at)
        .into_iter()
        .map(|r| [at.0, at.1, r.0, r.1, d.0, d.1])
        .reduce(|x, y| {
            let (ax, ay) = (angle_of(x), angle_of(y));
            let (ox, oy) = (off_step(ax), off_step(ay));
            if oy < ox - 0.01 || ((oy - ox).abs() <= 0.01 && ay < ax) {
                y
            } else {
                x
            }
        })
}

/// Degrees between a mark's two directions, 0 to 180.
pub fn angle_of(m: Mark) -> f32 {
    dot((m[2], m[3]), (m[4], m[5]))
        .clamp(-1.0, 1.0)
        .acos()
        .to_degrees()
}

/// Where `line` crosses `shape`'s chords closest to `near`, within the join radius.
fn crossing(shape: Shape, line: Line, near: Point) -> Option<Point> {
    let mut best: Option<(Point, f32)> = None;
    for (_, p, _, q) in shape.segments() {
        let e = (q.0 - p.0, q.1 - p.1);
        let denom = cross(e, line.dir);
        if denom.abs() < 1e-6 {
            continue;
        }
        let s = cross((line.origin.0 - p.0, line.origin.1 - p.1), line.dir) / denom;
        if !(0.0..=1.0).contains(&s) {
            continue;
        }
        let x = (p.0 + e.0 * s, p.1 + e.1 * s);
        let d = dist(x, near);
        if d < SNAP_RADIUS && best.is_none_or(|b| d < b.1) {
            best = Some((x, d));
        }
    }
    best.map(|b| b.0)
}

fn meet(l: Line, m: Line) -> Option<Point> {
    let denom = cross(l.dir, m.dir);
    if denom.abs() < PARALLEL_SIN {
        return None;
    }
    let w = (m.origin.0 - l.origin.0, m.origin.1 - l.origin.1);
    let t = cross(w, m.dir) / denom;
    Some((l.origin.0 + l.dir.0 * t, l.origin.1 + l.dir.1 * t))
}
fn project(l: Line, p: Point) -> Point {
    let t = dot((p.0 - l.origin.0, p.1 - l.origin.1), l.dir);
    (l.origin.0 + l.dir.0 * t, l.origin.1 + l.dir.1 * t)
}
/// `r` turned by `k` × 15° counter-clockwise.
fn rotate(r: Point, k: usize) -> Point {
    let (c, s) = QUADRANT[k % 6];
    let (c, s) = match k / 6 {
        0 => (c, s),
        1 => (-s, c),
        2 => (-c, -s),
        _ => (s, -c),
    };
    (r.0 * c - r.1 * s, r.0 * s + r.1 * c)
}
fn unit(v: Point) -> Point {
    let len = v.0.hypot(v.1).max(1e-9);
    (v.0 / len, v.1 / len)
}
fn dot(a: Point, b: Point) -> f32 {
    a.0 * b.0 + a.1 * b.1
}
fn cross(a: Point, b: Point) -> f32 {
    a.0 * b.1 - a.1 * b.0
}
