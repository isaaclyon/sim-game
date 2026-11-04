//! Hit tests for plant footprints and line corridors against roads, lots and buildings.

use crate::geometry::Point;

/// A plant's square footprint: its centre, the unit direction its front faces, and half its
/// side. Tests run in the footprint's own frame, where it is the axis-aligned square
/// `[-half, half]²` with the front toward +v, so rotation needs no trigonometry.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Footprint {
    pub centre: Point,
    pub front: Point,
    pub half: f32,
}

impl Footprint {
    /// `p` in the footprint's frame: across (right of the front), then along the front.
    fn local(self, p: Point) -> Point {
        let (dx, dy) = (p.0 - self.centre.0, p.1 - self.centre.1);
        let (fx, fy) = self.front;
        (dx * fy - dy * fx, dx * fx + dy * fy)
    }

    fn world(self, (u, v): Point) -> Point {
        let (fx, fy) = self.front;
        (
            self.centre.0 + u * fy + v * fx,
            self.centre.1 - u * fx + v * fy,
        )
    }

    fn square(self) -> [f32; 4] {
        [-self.half, -self.half, self.half, self.half]
    }

    /// Corners counter-clockwise, starting back-left.
    pub fn ring(self) -> [Point; 4] {
        let h = self.half;
        [(-h, -h), (h, -h), (h, h), (-h, h)].map(|c| self.world(c))
    }

    /// `[min_x, min_y, max_x, max_y]` around the corners.
    pub fn bounds(self) -> [f32; 4] {
        self.ring().iter().fold(
            [f32::MAX, f32::MAX, f32::MIN, f32::MIN],
            |[a, b, c, d], p| [a.min(p.0), b.min(p.1), c.max(p.0), d.max(p.1)],
        )
    }

    /// The point `offset` metres behind the back edge's middle.
    pub fn behind(self, offset: f32) -> Point {
        self.world((0.0, -self.half - offset))
    }

    pub fn contains(self, p: Point, margin: f32) -> bool {
        let (u, v) = self.local(p);
        let r = self.half + margin;
        u.abs() <= r && v.abs() <= r
    }

    /// Whether segment `a`–`b` meets the footprint grown by `margin`.
    pub fn hits_segment(self, a: Point, b: Point, margin: f32) -> bool {
        segment_hits_rect(self.local(a), self.local(b), self.square(), margin)
    }

    /// Whether the closed ring overlaps the footprint.
    pub fn hits_ring(self, ring: &[Point]) -> bool {
        let local: Vec<Point> = ring.iter().map(|&p| self.local(p)).collect();
        ring_hits_rect(&local, self.square())
    }

    /// Whether two footprints overlap by more than touching.
    pub fn overlaps(self, other: Footprint) -> bool {
        let shrink = |f: Footprint| Footprint {
            half: f.half - 0.01,
            ..f
        };
        shrink(self).hits_ring(&shrink(other).ring())
    }
}

/// Whether segment `a`–`b` meets the rectangle `[min_x, min_y, max_x, max_y]` grown by `margin`
/// (Liang–Barsky clipping).
pub(crate) fn segment_hits_rect(a: Point, b: Point, rect: [f32; 4], margin: f32) -> bool {
    let (lo, hi) = (
        (rect[0] - margin, rect[1] - margin),
        (rect[2] + margin, rect[3] + margin),
    );
    let d = (b.0 - a.0, b.1 - a.1);
    let (mut t0, mut t1) = (0.0f32, 1.0f32);
    for (p, q) in [
        (-d.0, a.0 - lo.0),
        (d.0, hi.0 - a.0),
        (-d.1, a.1 - lo.1),
        (d.1, hi.1 - a.1),
    ] {
        if p == 0.0 {
            if q < 0.0 {
                return false;
            }
        } else {
            let r = q / p;
            if p < 0.0 {
                t0 = t0.max(r);
            } else {
                t1 = t1.min(r);
            }
            if t0 > t1 {
                return false;
            }
        }
    }
    true
}

fn cross(o: Point, a: Point, b: Point) -> f32 {
    (a.0 - o.0) * (b.1 - o.1) - (a.1 - o.1) * (b.0 - o.0)
}

fn segments_cross(a: Point, b: Point, c: Point, d: Point) -> bool {
    let (d1, d2) = (cross(c, d, a), cross(c, d, b));
    let (d3, d4) = (cross(a, b, c), cross(a, b, d));
    (d1 > 0.0) != (d2 > 0.0) && (d3 > 0.0) != (d4 > 0.0)
}

/// Whether segment `a`–`b` crosses into the closed ring.
pub(crate) fn segment_hits_ring(a: Point, b: Point, ring: &[Point]) -> bool {
    crate::poly::contains(ring, a)
        || crate::poly::contains(ring, b)
        || (0..ring.len()).any(|i| segments_cross(a, b, ring[i], ring[(i + 1) % ring.len()]))
}

/// Whether the closed ring overlaps the rectangle.
pub(crate) fn ring_hits_rect(ring: &[Point], rect: [f32; 4]) -> bool {
    let centre = ((rect[0] + rect[2]) / 2.0, (rect[1] + rect[3]) / 2.0);
    crate::poly::contains(ring, centre)
        || (0..ring.len())
            .any(|i| segment_hits_rect(ring[i], ring[(i + 1) % ring.len()], rect, 0.0))
}

#[cfg(test)]
mod tests {
    use super::*;

    const SQUARE: [Point; 4] = [(0.0, 0.0), (10.0, 0.0), (10.0, 10.0), (0.0, 10.0)];

    #[test]
    fn segments_against_rectangles() {
        let r = [0.0, 0.0, 10.0, 10.0];
        assert!(segment_hits_rect((-5.0, 5.0), (15.0, 5.0), r, 0.0));
        assert!(segment_hits_rect((2.0, 2.0), (3.0, 3.0), r, 0.0));
        assert!(!segment_hits_rect((-5.0, 12.0), (15.0, 12.0), r, 0.0));
        assert!(segment_hits_rect((-5.0, 12.0), (15.0, 12.0), r, 3.0));
        assert!(!segment_hits_rect((12.0, -5.0), (12.0, 15.0), r, 1.0));
    }

    #[test]
    fn segments_and_rects_against_rings() {
        assert!(segment_hits_ring((-5.0, 5.0), (15.0, 5.0), &SQUARE));
        assert!(segment_hits_ring((5.0, 5.0), (6.0, 6.0), &SQUARE));
        assert!(!segment_hits_ring((-5.0, 15.0), (15.0, 15.0), &SQUARE));
        assert!(ring_hits_rect(&SQUARE, [8.0, 8.0, 20.0, 20.0]));
        assert!(ring_hits_rect(&SQUARE, [2.0, 2.0, 3.0, 3.0]));
        assert!(ring_hits_rect(&SQUARE, [-5.0, -5.0, 20.0, 20.0]));
        assert!(!ring_hits_rect(&SQUARE, [11.0, 11.0, 20.0, 20.0]));
    }

    #[test]
    fn turned_footprints() {
        let s = std::f32::consts::FRAC_1_SQRT_2;
        let f = Footprint {
            centre: (10.0, 10.0),
            front: (s, s),
            half: 5.0,
        };
        let ring = f.ring();
        let corner = |p: Point| (p.0 - 10.0).hypot(p.1 - 10.0);
        assert!(
            ring.iter()
                .all(|&p| (corner(p) - 5.0 * 2f32.sqrt()).abs() < 1e-3)
        );
        let twice_area: f32 = (0..4)
            .map(|i| cross((0.0, 0.0), ring[i], ring[(i + 1) % 4]))
            .sum();
        assert!(twice_area > 0.0, "counter-clockwise");
        assert!(
            f.contains((10.0, 16.5), 0.0),
            "inside the turned square, outside its axis box"
        );
        assert!(
            !f.contains((15.5, 15.5), 0.0),
            "inside the axis box, past the front"
        );
        let back = f.behind(2.0);
        assert!((corner(back) - 7.0).abs() < 1e-4 && back.0 < 10.0 && back.1 < 10.0);
        assert!(f.hits_segment((0.0, 20.0), (20.0, 0.0), 0.0));
        // The front edge lies on x + y = 20 + 5√2 ≈ 27.07.
        assert!(!f.hits_segment((12.0, 15.5), (15.5, 12.0), 0.0));
        assert!(f.hits_segment((12.0, 15.5), (15.5, 12.0), 1.0));
        let side = Footprint {
            centre: (10.0 + 10.0 * s, 10.0 - 10.0 * s),
            ..f
        };
        assert!(
            !f.overlaps(side),
            "squares that share an edge do not overlap"
        );
        assert!(f.overlaps(Footprint {
            centre: (12.0, 9.0),
            ..side
        }));
        let b = f.bounds();
        assert!((b[0] - (10.0 - 5.0 * 2f32.sqrt())).abs() < 1e-3);
    }
}
