//! Tracing the bounded faces of the road graph.

use std::cmp::Ordering;

use super::{Blocks, Rect, cross, left_of, sub, union, unit};
use crate::geometry::{Point, dist, lerp};
use crate::road::{LaneId, RoadId, RoadNetwork};

/// Faces with less area, square metres, are slivers or tree-shaped pieces with no inside.
const MIN_BLOCK_AREA: f32 = 1.0;

fn rect_of(points: &[Point]) -> Rect {
    points
        .iter()
        .fold([f32::MAX, f32::MAX, f32::MIN, f32::MIN], |r, p| {
            union(r, [p.0, p.1, p.0, p.1])
        })
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
}
