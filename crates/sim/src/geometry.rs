//! Quadratic Bézier centreline geometry shared by construction, traffic and snapshots.
//! Subdivision bounds the deviation from a rendered chord to 2 cm. The same chords build
//! the distance lookup table; simulation queries evaluate the curve, not the chords.

pub(crate) type Point = (f32, f32);

#[derive(Clone, Copy, Debug)]
pub(crate) struct Shape {
    pub a: Point,
    pub control: Option<Point>,
    pub b: Point,
}

pub(crate) fn lerp(a: Point, b: Point, t: f32) -> Point {
    (a.0 + (b.0 - a.0) * t, a.1 + (b.1 - a.1) * t)
}
pub(crate) fn dist(a: Point, b: Point) -> f32 {
    (b.0 - a.0).hypot(b.1 - a.1)
}
pub(crate) fn project(a: Point, b: Point, p: Point) -> f32 {
    let d = (b.0 - a.0, b.1 - a.1);
    (((p.0 - a.0) * d.0 + (p.1 - a.1) * d.1) / (d.0 * d.0 + d.1 * d.1).max(1e-12)).clamp(0.0, 1.0)
}

impl Shape {
    pub fn point(self, t: f32) -> Point {
        // Shared node coordinates must match exactly for the renderer's junction pads.
        // Interpolation can otherwise round an endpoint away from its stored position.
        if t == 0.0 {
            return self.a;
        }
        if t == 1.0 {
            return self.b;
        }
        match self.control {
            Some(c) => lerp(lerp(self.a, c, t), lerp(c, self.b, t), t),
            None => lerp(self.a, self.b, t),
        }
    }

    pub fn tangent(self, t: f32) -> Point {
        let c = self.control.unwrap_or(lerp(self.a, self.b, 0.5));
        let p = lerp(
            (c.0 - self.a.0, c.1 - self.a.1),
            (self.b.0 - c.0, self.b.1 - c.1),
            t,
        );
        (2.0 * p.0, 2.0 * p.1)
    }

    pub fn valid(self) -> bool {
        let finite =
            |p: Point| p.0.is_finite() && p.1.is_finite() && p.0.abs() <= 1e6 && p.1.abs() <= 1e6;
        if !finite(self.a)
            || !finite(self.b)
            || !self.control.is_none_or(finite)
            || dist(self.a, self.b) < 1e-3
        {
            return false;
        }
        // Reject cusps and zero-length endpoint tangents: a road must have a direction.
        let (u, v) = (self.tangent(0.0), self.tangent(1.0));
        dist(lerp(u, v, project(u, v, (0.0, 0.0))), (0.0, 0.0)) > 1e-3
    }

    pub fn steps(self) -> usize {
        match self.control {
            None => 1,
            Some(c) => {
                let bend = (self.a.0 - 2.0 * c.0 + self.b.0).hypot(self.a.1 - 2.0 * c.1 + self.b.1);
                ((bend / 0.08).sqrt().ceil() as usize).clamp(1, 8192)
            }
        }
    }

    pub fn segments(self) -> impl Iterator<Item = (f32, Point, f32, Point)> {
        let n = self.steps();
        (0..n).map(move |i| {
            let t = i as f32 / n as f32;
            let u = (i + 1) as f32 / n as f32;
            (t, self.point(t), u, self.point(u))
        })
    }

    pub fn lengths(self) -> Vec<f32> {
        let mut out = vec![0.0];
        for (_, p, _, q) in self.segments() {
            out.push(out.last().copied().unwrap() + dist(p, q));
        }
        out
    }

    /// Closest parameter, starting from the nearest sampled chord then refining on the curve.
    pub fn nearest(self, p: Point) -> f32 {
        if self.control.is_none() {
            return project(self.a, self.b, p);
        }
        let mut best = (f32::MAX, 0.0);
        for (t, a, u, b) in self.segments() {
            let f = project(a, b, p);
            let d = dist(lerp(a, b, f), p);
            if d < best.0 {
                best = (d, t + (u - t) * f);
            }
        }
        let mut t = best.1;
        let c = self.control.unwrap();
        let second = (
            2.0 * (self.a.0 - 2.0 * c.0 + self.b.0),
            2.0 * (self.a.1 - 2.0 * c.1 + self.b.1),
        );
        for _ in 0..6 {
            let q = self.point(t);
            let d = self.tangent(t);
            let denom = d.0 * d.0 + d.1 * d.1 + (q.0 - p.0) * second.0 + (q.1 - p.1) * second.1;
            if denom.abs() < 1e-6 {
                break;
            }
            t = (t - ((q.0 - p.0) * d.0 + (q.1 - p.1) * d.1) / denom).clamp(0.0, 1.0);
        }
        t
    }

    /// Exact Bézier subcurve, using the derivative at its first endpoint.
    pub fn section(self, t: f32, u: f32) -> Self {
        let a = self.point(t);
        let d = self.tangent(t);
        Self {
            a,
            b: self.point(u),
            control: self
                .control
                .map(|_| (a.0 + d.0 * (u - t) / 2.0, a.1 + d.1 * (u - t) / 2.0)),
        }
    }
}
