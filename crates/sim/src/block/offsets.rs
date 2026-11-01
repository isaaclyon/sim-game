//! How far back from each road side its lots reach, and where they divide along it.

use super::{Blocks, LOT_REACH, LOT_WIDTH, ROAD_HALF_WIDTH, cross, left_of, sub, unit};
use crate::geometry::{Point, dist, lerp, project};
use crate::poly::{self, Ring};
use crate::road::{LaneId, RoadId};

/// Spacing, metres, of the points along a road side that trace the back of its lots.
const BACK_STEP: f32 = 2.0;
/// Corners turning back sharper than this (`1 + cos` of the turn) end lots square instead of
/// along the corner's bisector: dead ends and hairpins.
const MIN_MITRE: f32 = 0.05;

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

impl Blocks {
    /// Offsets back from `lane`'s side of its road: `(arc, point, unit direction, depth)` at
    /// the road's ends, its samples, its stretch ends and every `BACK_STEP` between, in order
    /// along the road. Each goes back as far as its land is nearer this road than any other
    /// (so lots from facing roads meet on one line), at most `LOT_REACH` from its road, and
    /// stops where it first crosses another offset of the same side, so lots on curves come
    /// to a point instead of folding over each other.
    pub(super) fn offsets(
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
    pub(super) fn ribbon(
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
    pub(super) fn edges(&self, lane: LaneId, flanks: &[LaneId]) -> Vec<f32> {
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
}
