//! Cutting a block into lots along its roads (ADR 0018).

use std::collections::BTreeSet;

use super::{
    Blocks, LOT_REACH, LotPlan, MAX_GAP_AREA, MIN_LOT_AREA, ROAD_HALF_WIDTH, Rect, contains_rect,
    cross, intersects, left_of, sub, unit,
};
use crate::geometry::Point;
use crate::poly::{self, Ring};
use crate::road::{LaneId, RoadId};
use crate::zone::ZoneGrid;

/// Outline corners sharper than this cosine (about 10°) cutting off less than `SPIKE_AREA`
/// square metres are spikes: hair-thin strips where two lots' back lines nearly agree.
const SPIKE_COS: f32 = 0.985;
const SPIKE_AREA: f32 = 2.0;
/// Cosine of the largest turn, 30°, between pieces of one straight block side.
const STRAIGHT_COS: f32 = 0.866;
/// Cosine of the largest turn away from 90°, 30°, at a corner of a rectangle-like block.
const RIGHT_COS: f32 = 0.5;

/// A lot being cut, with its outline still on the polygon grid.
struct Piece {
    road: RoadId,
    left: bool,
    /// The stretch of road the lot fronts, metres from the road's start.
    span: (f32, f32),
    ring: Ring,
    rect: Rect,
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

impl Blocks {
    /// Cuts block `i` into lots (ADR 0018): each road side around it, in stretches of about
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
}
