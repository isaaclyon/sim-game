//! Planning alleys and connector streets through blocks too deep to reach from their edges.

use super::{Blocks, Cell, LOT_REACH, cross, intersects, sub};
use crate::geometry::{Point, Shape};
use crate::road::RoadId;
use crate::zone::{Zone, ZoneGrid};

impl Blocks {
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
