//! Placing things beside a road, facing it: the shared snap for anything the player sets down
//! along a street (plants now; service buildings and other roadside pieces later; ADR 0042).
//! Planning only; nothing here mutates the network.

use crate::geometry::{Point, dist, lerp, project};
use crate::road::{RoadClass, RoadId, RoadNetwork};

/// Distance from a road's centreline to the outer edge of its sidewalk, metres. Matches the
/// frontend's road and sidewalk widths (`road-geometry.ts`, `renderer.ts`).
pub fn kerb(class: RoadClass) -> f32 {
    match class {
        RoadClass::Main => 8.0,
        RoadClass::Street | RoadClass::Alley => 6.0,
    }
}

/// Where a point meets the road nearest to it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Frontage {
    pub road: RoadId,
    /// The nearest point on the road's centreline.
    pub on: Point,
    /// Unit direction from the point's side of the road toward the road: the way a thing
    /// placed here faces.
    pub front: Point,
    /// The road's kerb (`kerb`) on that side.
    pub kerb: f32,
}

impl Frontage {
    /// The same place, facing from `from`'s side of the road instead. A point on the
    /// centreline keeps the side it has.
    pub fn facing(self, from: Point) -> Frontage {
        let away = (from.0 - self.on.0) * self.front.0 + (from.1 - self.on.1) * self.front.1;
        if away > 0.0 {
            Frontage {
                front: (-self.front.0, -self.front.1),
                ..self
            }
        } else {
            self
        }
    }

    /// The point `offset` metres out from the centreline, on the placed side.
    pub fn out(self, offset: f32) -> Point {
        (
            self.on.0 - self.front.0 * offset,
            self.on.1 - self.front.1 * offset,
        )
    }
}

/// The active road whose centreline passes nearest `p`, within `reach` metres. Ties keep the
/// lower road ID and earlier chord, so every target picks the same road.
pub fn frontage(net: &RoadNetwork, p: Point, reach: f32) -> Option<Frontage> {
    if !(p.0.is_finite() && p.1.is_finite()) {
        return None;
    }
    let mut best: Option<(f32, Frontage)> = None;
    for road in net.active_roads() {
        for (_, a, _, b) in net.shape(road).segments() {
            let on = lerp(a, b, project(a, b, p));
            let d = dist(on, p);
            let (tx, ty) = (b.0 - a.0, b.1 - a.1);
            let len = (tx * tx + ty * ty).sqrt();
            if d > reach || len == 0.0 || best.is_some_and(|(old, _)| d >= old) {
                continue;
            }
            // The normal on the point's side; a point on the centreline takes the left.
            let mut n = (-ty / len, tx / len);
            if (p.0 - on.0) * n.0 + (p.1 - on.1) * n.1 < 0.0 {
                n = (-n.0, -n.1);
            }
            let front = (-n.0, -n.1);
            let kerb = kerb(net.road_class(road));
            best = Some((
                d,
                Frontage {
                    road,
                    on,
                    front,
                    kerb,
                },
            ));
        }
    }
    best.map(|(_, f)| f)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn street() -> RoadNetwork {
        let mut net = RoadNetwork::default();
        let a = net.add_node(0.0, 0.0);
        let b = net.add_node(100.0, 0.0);
        net.add_road(a, b, RoadClass::Street);
        net
    }

    #[test]
    fn faces_the_road_from_either_side() {
        let net = street();
        let above = frontage(&net, (40.0, 30.0), 50.0).unwrap();
        assert_eq!(above.on, (40.0, 0.0));
        assert_eq!(above.front, (0.0, -1.0));
        assert_eq!(above.out(10.0), (40.0, 10.0));
        let below = frontage(&net, (60.0, -5.0), 50.0).unwrap();
        assert_eq!(below.front, (0.0, 1.0));
        let out = below.out(6.0);
        assert!((out.0 - 60.0).abs() < 1e-4 && out.1 == -6.0);
        assert_eq!(below.kerb, 6.0);
        assert_eq!(below.facing((0.0, 50.0)).front, (0.0, -1.0));
        assert_eq!(below.facing((0.0, -50.0)), below);
    }

    #[test]
    fn refuses_points_out_of_reach() {
        let net = street();
        assert!(frontage(&net, (40.0, 30.0), 20.0).is_none());
        assert!(frontage(&net, (f32::NAN, 0.0), 20.0).is_none());
        assert!(frontage(&RoadNetwork::default(), (0.0, 0.0), 20.0).is_none());
    }

    #[test]
    fn picks_the_nearest_road() {
        let mut net = street();
        let c = net.add_node(0.0, 50.0);
        let d = net.add_node(100.0, 50.0);
        net.add_road(c, d, RoadClass::Main);
        let f = frontage(&net, (10.0, 40.0), 50.0).unwrap();
        assert_eq!((f.on, f.front, f.kerb), ((10.0, 50.0), (0.0, 1.0), 8.0));
    }
}
