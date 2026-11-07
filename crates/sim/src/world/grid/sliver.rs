//! Sliver clearance for the grid tool (ADR 0049). A sliver is a strip narrower than
//! `SLIVER` between two roughly parallel roads: its lots would be shallower than one lot is
//! wide. Where existing roads run that close alongside a lattice edge, the edge gives way:
//! it is cut back to where they stop, or left out, and those roads take its place.

use crate::block;
use crate::geometry::{Point, dist};
use crate::road::{RoadId, RoadNetwork};

/// Narrowest strip between roughly parallel roads, centreline to centreline: 5 zone cells.
pub const SLIVER: f32 = 40.0;
/// Shortest stretch alongside that counts: one lot's width. A shorter one leaves a corner
/// too small for a lot of its own.
pub const OVERLAP: f32 = 16.0;
/// Roads closer to parallel than this (sine of the angle between them, about 11.5°) run
/// alongside each other, as in the planner's overlap test.
const PARALLEL_SIN: f32 = 0.2;
/// Lattice edges give way to roads up to about 23.5° off: planning can still turn an edge's
/// legs, as it joins their ends to nearby junctions, by up to about 12°.
const GIVE_WAY_SIN: f32 = 0.4;

/// What happens to one lattice edge.
pub(super) enum Fit {
    /// Nothing runs alongside: planned as drawn.
    Keep,
    /// Roads run alongside all of it, its middle, or an end and then turn away:
    /// left out, and these roads (ascending, no repeats) take its place.
    Drop(Vec<RoadId>),
    /// Roads that end beside it run alongside one or both ends: planned between these points
    /// instead, each either the lattice end or the point where the road ends.
    Trim(Point, Point),
}

/// One stretch of an edge with a road chord alongside: from `lo` to `hi` metres along the
/// edge, and the road's points across from them.
struct Run {
    lo: f32,
    hi: f32,
    at_lo: Point,
    at_hi: Point,
    road: RoadId,
}

/// How the edge from `a` to `b` fits beside the roads already in `net`.
pub(super) fn fit(net: &RoadNetwork, a: Point, b: Point) -> Fit {
    let len = dist(a, b);
    let d = ((b.0 - a.0) / len, (b.1 - a.1) / len);
    let mut runs: Vec<Run> = Vec::new();
    for road in net.active_roads().filter(|&r| within_reach(net, r, a, b)) {
        for (_, r0, _, r1) in net.shape(road).segments() {
            runs.extend(run(a, d, len, r0, r1, road, GIVE_WAY_SIN));
        }
    }
    if runs.is_empty() {
        return Fit::Keep;
    }
    runs.sort_by(|p, q| p.lo.total_cmp(&q.lo).then(p.road.cmp(&q.road)));
    // Merge touching runs; each merged run remembers the road point at either end.
    let mut merged: Vec<Run> = Vec::new();
    for r in runs {
        match merged.last_mut() {
            Some(m) if r.lo <= m.hi + 1e-3 => {
                if r.hi > m.hi {
                    (m.hi, m.at_hi) = (r.hi, r.at_hi);
                }
            }
            _ => merged.push(r),
        }
    }
    let mut roads: Vec<RoadId> = Vec::new();
    let (mut start, mut end) = (a, b);
    let (mut from, mut to) = (0.0, len);
    // Only a road that ends beside the edge can carry it on. Where a run ends because the
    // road turns away, the cut end would jog back across the strip.
    let near = |p: Point| cross(d, (p.0 - a.0, p.1 - a.1)).abs() < SLIVER - 1.0;
    for m in merged.iter().filter(|m| m.hi - m.lo >= OVERLAP) {
        if m.lo < OVERLAP && near(m.at_hi) {
            (start, from) = (m.at_hi, m.hi);
        } else if m.hi > len - OVERLAP && near(m.at_lo) {
            (end, to) = (m.at_lo, m.lo);
        } else {
            to = from;
        }
    }
    if (from, to) == (0.0, len) {
        return Fit::Keep;
    }
    if to - from < OVERLAP {
        // Nothing worth a road is left between the roads alongside.
        for road in net.active_roads().filter(|&r| within_reach(net, r, a, b)) {
            let segs = net.shape(road).segments();
            if segs
                .into_iter()
                .any(|(_, r0, _, r1)| run(a, d, len, r0, r1, road, GIVE_WAY_SIN).is_some())
            {
                roads.push(road);
            }
        }
        return Fit::Drop(roads);
    }
    Fit::Trim(start, end)
}

/// Whether a new straight leg from `a` to `b` would make a sliver with a road in `net`: run
/// alongside it, within `PARALLEL_SIN`, for at least `OVERLAP`. The last check on every leg.
pub(super) fn makes_sliver(net: &RoadNetwork, a: Point, b: Point) -> bool {
    let len = dist(a, b);
    let d = ((b.0 - a.0) / len, (b.1 - a.1) / len);
    net.active_roads().any(|road| {
        if !within_reach(net, road, a, b) {
            return false;
        }
        let segs = net.shape(road).segments();
        let beside: f32 = segs
            .into_iter()
            .filter_map(|(_, r0, _, r1)| run(a, d, len, r0, r1, road, PARALLEL_SIN))
            .map(|r| r.hi - r.lo)
            .sum();
        beside >= OVERLAP
    })
}

/// The stretch of the edge from `a` along unit `d` for `len` that the road chord `r0`–`r1`
/// runs beside within `SLIVER`, if the chord is within `max_sin` of parallel and the stretch
/// not empty.
fn run(
    a: Point,
    d: Point,
    len: f32,
    r0: Point,
    r1: Point,
    road: RoadId,
    max_sin: f32,
) -> Option<Run> {
    let r_len = dist(r0, r1);
    if r_len < 1e-3 {
        return None;
    }
    let rd = ((r1.0 - r0.0) / r_len, (r1.1 - r0.1) / r_len);
    if cross(d, rd).abs() >= max_sin {
        return None;
    }
    // Along the edge (s) and across it (o) for both chord ends, in increasing s.
    let at = |p: Point| {
        let w = (p.0 - a.0, p.1 - a.1);
        (dot(w, d), cross(d, w), p)
    };
    let (mut p, mut q) = (at(r0), at(r1));
    if q.0 < p.0 {
        std::mem::swap(&mut p, &mut q);
    }
    if q.0 - p.0 < 1e-6 {
        return None;
    }
    // The offset is linear in s; keep the part of the edge where it is within the sliver.
    let slope = (q.1 - p.1) / (q.0 - p.0);
    let (mut lo, mut hi) = (p.0.max(0.0), q.0.min(len));
    if slope.abs() < 1e-6 {
        if p.1.abs() >= SLIVER {
            return None;
        }
    } else {
        let s = |o: f32| p.0 + (o - p.1) / slope;
        let (s0, s1) = (s(-SLIVER), s(SLIVER));
        (lo, hi) = (lo.max(s0.min(s1)), hi.min(s0.max(s1)));
    }
    if hi - lo < 1e-3 {
        return None;
    }
    let point = |s: f32| {
        let t = (s - p.0) / (q.0 - p.0);
        (p.2.0 + (q.2.0 - p.2.0) * t, p.2.1 + (q.2.1 - p.2.1) * t)
    };
    Some(Run {
        lo,
        hi,
        at_lo: point(lo),
        at_hi: point(hi),
        road,
    })
}

/// Where the line through `end` along `dir` meets the line of one of `roads`' segments,
/// nearest `end` and at most `reach` away; None if none does. Lines, not segments: the road
/// replacing an edge may end short of the cross edge, which then meets the road beyond it.
pub(super) fn meet(
    net: &RoadNetwork,
    roads: impl Iterator<Item = RoadId>,
    end: Point,
    dir: Point,
    reach: f32,
) -> Option<Point> {
    let len = dir.0.hypot(dir.1);
    let d = (dir.0 / len, dir.1 / len);
    let mut best: Option<(Point, f32)> = None;
    for road in roads {
        for (_, r0, _, r1) in net.shape(road).segments() {
            let e = (r1.0 - r0.0, r1.1 - r0.1);
            let denom = cross(d, e);
            if denom.abs() < 1e-6 {
                continue;
            }
            let w = (r0.0 - end.0, r0.1 - end.1);
            let t = cross(w, e) / denom;
            if t.abs() <= reach && best.is_none_or(|b| t.abs() < b.1) {
                best = Some(((end.0 + d.0 * t, end.1 + d.1 * t), t.abs()));
            }
        }
    }
    best.map(|b| b.0)
}

/// Whether `road`'s bounding box comes within `SLIVER` of the segment `a`–`b`'s: a cheap
/// test that skips most roads before their segments are measured.
fn within_reach(net: &RoadNetwork, road: RoadId, a: Point, b: Point) -> bool {
    let [x0, y0, x1, y1] = block::shape_rect(net.shape(road));
    x0 - SLIVER <= a.0.max(b.0)
        && a.0.min(b.0) <= x1 + SLIVER
        && y0 - SLIVER <= a.1.max(b.1)
        && a.1.min(b.1) <= y1 + SLIVER
}

fn dot(a: Point, b: Point) -> f32 {
    a.0 * b.0 + a.1 * b.1
}
fn cross(a: Point, b: Point) -> f32 {
    a.0 * b.1 - a.1 * b.0
}
