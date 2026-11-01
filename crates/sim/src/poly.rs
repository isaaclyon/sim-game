//! Polygon operations for lots (ADR 0017).
//!
//! Points are snapped to a fixed grid of 1/256 m in whole numbers before any operation, so
//! results are exact, shared edges match bit for bit and every machine gets the same answer.

use i_overlay::core::fill_rule::FillRule;
use i_overlay::core::overlay_rule::OverlayRule;
use i_overlay::core::single::SingleIntOverlay;
use i_overlay::i_float::int::point::IntPoint;

use crate::geometry::Point;

/// Grid steps per metre.
const SCALE: f64 = 256.0;

/// A closed ring of grid points, without repeating the first.
pub(crate) type Ring = Vec<IntPoint>;
/// Rings forming one area: the first is the outside (counter-clockwise), the rest are holes.
pub(crate) type Area = Vec<Ring>;

pub(crate) fn to_grid(p: Point) -> IntPoint {
    IntPoint::new(
        (p.0 as f64 * SCALE).round() as i32,
        (p.1 as f64 * SCALE).round() as i32,
    )
}

pub(crate) fn to_point(p: IntPoint) -> Point {
    ((p.x as f64 / SCALE) as f32, (p.y as f64 / SCALE) as f32)
}

pub(crate) fn ring(points: impl IntoIterator<Item = Point>) -> Ring {
    let mut r: Ring = points.into_iter().map(to_grid).collect();
    r.dedup();
    if r.len() > 1 && r[0] == r[r.len() - 1] {
        r.pop();
    }
    r
}

fn op(subject: &[Ring], clip: &[Ring], rule: OverlayRule) -> Vec<Area> {
    subject.overlay(clip, rule, FillRule::NonZero)
}

/// Where `a` and `b` overlap. Rings within each side are combined by winding: holes must
/// wind the opposite way to outsides, as `Area`s do.
pub(crate) fn intersect(a: &[Ring], b: &[Ring]) -> Vec<Area> {
    op(a, b, OverlayRule::Intersect)
}

/// `a` without `b`.
pub(crate) fn subtract(a: &[Ring], b: &[Ring]) -> Vec<Area> {
    op(a, b, OverlayRule::Difference)
}

/// `a` and `b` together.
pub(crate) fn union(a: &[Ring], b: &[Ring]) -> Vec<Area> {
    op(a, b, OverlayRule::Union)
}

/// Signed area in square metres, positive for counter-clockwise rings.
pub(crate) fn area(r: &[IntPoint]) -> f32 {
    let twice: i64 = (0..r.len())
        .map(|i| {
            let (a, b) = (r[i], r[(i + 1) % r.len()]);
            a.x as i64 * b.y as i64 - b.x as i64 * a.y as i64
        })
        .sum();
    (twice as f64 / 2.0 / (SCALE * SCALE)) as f32
}

/// Length of the ring's edges, metres.
pub(crate) fn perimeter(r: &[IntPoint]) -> f32 {
    let sum: f64 = (0..r.len())
        .map(|i| {
            let (a, b) = (r[i], r[(i + 1) % r.len()]);
            ((b.x - a.x) as f64).hypot((b.y - a.y) as f64)
        })
        .sum();
    (sum / SCALE) as f32
}

/// `[min_x, min_y, max_x, max_y]` of the ring, metres.
pub(crate) fn bounds(r: &[IntPoint]) -> [f32; 4] {
    let (mut lo, mut hi) = (
        IntPoint::new(i32::MAX, i32::MAX),
        IntPoint::new(i32::MIN, i32::MIN),
    );
    for p in r {
        lo = IntPoint::new(lo.x.min(p.x), lo.y.min(p.y));
        hi = IntPoint::new(hi.x.max(p.x), hi.y.max(p.y));
    }
    let (a, b) = (to_point(lo), to_point(hi));
    [a.0, a.1, b.0, b.1]
}

/// Whether `p` is inside ring `r` (even-odd; points on an edge may go either way).
pub(crate) fn contains(r: &[Point], p: Point) -> bool {
    let mut inside = false;
    for j in 0..r.len() {
        let (a, b) = (r[j], r[(j + 1) % r.len()]);
        if (a.1 > p.1) != (b.1 > p.1) && p.0 < a.0 + (p.1 - a.1) * (b.0 - a.0) / (b.1 - a.1) {
            inside = !inside;
        }
    }
    inside
}

#[cfg(test)]
mod tests {
    use super::*;

    fn square(x: f32, y: f32, s: f32) -> Ring {
        ring([(x, y), (x + s, y), (x + s, y + s), (x, y + s)])
    }

    #[test]
    fn operations_are_exact_on_the_grid() {
        let a = square(0.0, 0.0, 10.0);
        let b = square(5.0, 5.0, 10.0);
        let both = intersect(std::slice::from_ref(&a), std::slice::from_ref(&b));
        assert_eq!(both.len(), 1);
        assert_eq!(area(&both[0][0]), 25.0);
        let rest = subtract(std::slice::from_ref(&a), std::slice::from_ref(&b));
        assert_eq!(area(&rest[0][0]), 75.0);
        let all = union(&[a], &[b]);
        assert_eq!((all.len(), area(&all[0][0])), (1, 175.0));
        assert_eq!(perimeter(&all[0][0]), 60.0);
    }

    #[test]
    fn holes_wind_against_their_outside() {
        let outer = square(0.0, 0.0, 30.0);
        let hole = square(10.0, 10.0, 10.0);
        let land = subtract(&[outer], &[hole]);
        assert_eq!(land[0].len(), 2, "outside and hole");
        assert!(area(&land[0][0]) > 0.0 && area(&land[0][1]) < 0.0);
        // A strip across it keeps the hole out.
        let cut = intersect(&land[0], &[square(0.0, 12.0, 30.0)]);
        let total: f32 = cut.iter().flatten().map(|r| area(r)).sum();
        assert_eq!(total, 30.0 * 18.0 - 10.0 * 8.0);
    }
}
