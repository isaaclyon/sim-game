//! Footprints use lot polygons, never the zone grid. All clipping uses the fixed polygon grid.
use std::collections::BTreeSet;

use i_overlay::mesh::int::outline::offset::IntOutlineOffset;
use i_overlay::mesh::int::style::IntOutlineStyle;

use crate::block::ROAD_HALF_WIDTH;
use crate::geometry::Point;
use crate::lot::{LotId, Lots};
use crate::poly::{self, Area, Ring};
use crate::rng::Rng;
use crate::road::{RoadClass, RoadNetwork};
use crate::zone::Zone;

const MIN_FOOTPRINT: f32 = 25.0;

pub(super) fn make(
    lots: &Lots,
    lot: LotId,
    roads: &RoadNetwork,
    zone: Zone,
    rng: &mut Rng,
) -> Option<Vec<Point>> {
    let l = lot as usize;
    let land = poly::ring(lots.outline(lot).iter().copied());
    let facing = (lots.facing_x[l], lots.facing_y[l]);
    let front = (lots.front_x[l], lots.front_y[l]);
    // Every footprint type must clear frontage and corner roads. Lots themselves use a
    // fixed 4 m kerb, while main roads render 6 m wide on either side.
    let road = lots.road[l];
    let (a, b) = roads.road_ends(road);
    let nearby: BTreeSet<_> = [a, b]
        .into_iter()
        .flat_map(|n| roads.out_lanes(n))
        .map(|&lane| lane / 2)
        .chain([road])
        .collect();
    let mut streets = Vec::new();
    for r in nearby {
        let half_width = if roads.road_class(r) == RoadClass::Main {
            6.0
        } else {
            ROAD_HALF_WIDTH
        };
        for (_, a, _, b) in roads.shape(r).segments() {
            streets.push(street_band(a, b, half_width + 0.8));
        }
    }
    // Party walls stay on lot boundaries; only edges near streets recede.
    let clear = largest(poly::subtract(std::slice::from_ref(&land), &streets))?;
    let ring = match zone {
        Zone::ResidentialLow => road_box(
            &clear,
            front,
            facing,
            ((lots.frontage[l] - 4.0).clamp(5.0, 14.0), 14.0),
            8.0,
        ),
        Zone::IndustrialLow | Zone::IndustrialHigh => {
            // Integer offsets are deterministic, including at concave corners.
            let gap = 2.0 + rng.below(3) as f32;
            let style = IntOutlineStyle::new(-(gap * 256.0) as i32);
            let inset = clear
                .validate_outline(&style)
                .ok()
                .and_then(|()| clear.outline(&style).ok())
                .and_then(largest)?;
            let frontage = lots.frontage[l];
            let depth = if frontage < 24.0 {
                rng.range(14.0, 24.0)
            } else if frontage < 48.0 {
                rng.range(24.0, 38.0)
            } else {
                rng.range(32.0, 48.0)
            };
            let width = (frontage * rng.range(0.65, 0.85)).max(5.0);
            let setback = ROAD_HALF_WIDTH + rng.range(4.0, 9.0);
            road_box(&inset, front, facing, (width, depth), setback).or(Some(inset))
        }
        Zone::ResidentialHigh | Zone::CommercialLow | Zone::CommercialHigh => Some(clear),
        Zone::None => None,
    }?;
    Some(ring.into_iter().map(poly::to_point).collect())
}

fn largest(areas: Vec<Area>) -> Option<Ring> {
    areas
        .into_iter()
        .filter(|a| a.len() == 1)
        .map(|mut a| a.remove(0))
        .filter(|r| poly::area(r) >= MIN_FOOTPRINT)
        .max_by(|a, b| poly::area(a).total_cmp(&poly::area(b)))
}

fn street_band(a: Point, b: Point, width: f32) -> Ring {
    let len = (b.0 - a.0).hypot(b.1 - a.1);
    let (dx, dy) = ((b.0 - a.0) / len, (b.1 - a.1) / len);
    // Square caps cover the joins between curve samples.
    let a = (a.0 - dx * width, a.1 - dy * width);
    let b = (b.0 + dx * width, b.1 + dy * width);
    poly::ring([
        (a.0 + dy * width, a.1 - dx * width),
        (b.0 + dy * width, b.1 - dx * width),
        (b.0 - dy * width, b.1 + dx * width),
        (a.0 - dy * width, a.1 + dx * width),
    ])
}

/// Fit a road-aligned box, shrinking it on irregular lots. Exact polygon subtraction
/// (not just corner containment) prevents boxes crossing concave lot edges.
fn road_box(land: &Ring, front: Point, facing: Point, size: Point, setback: f32) -> Option<Ring> {
    let right = (-facing.1, facing.0);
    let (width, depth) = size;
    let mut best: Option<Ring> = None;
    let mut best_area = MIN_FOOTPRINT;
    for scale in [1.0, 0.85, 0.7, 0.55, 0.4] {
        let (w, d) = (width * scale, depth * scale);
        for back in [setback, setback + 6.0, setback + 14.0, setback + 22.0] {
            for side in [0.0, -width / 4.0, width / 4.0] {
                let p = (
                    front.0 - facing.0 * back + right.0 * side,
                    front.1 - facing.1 * back + right.1 * side,
                );
                let point = |x: f32, y: f32| {
                    (
                        p.0 + right.0 * x - facing.0 * y,
                        p.1 + right.1 * x - facing.1 * y,
                    )
                };
                let ring = poly::ring([
                    point(-w / 2.0, 0.0),
                    point(w / 2.0, 0.0),
                    point(w / 2.0, d),
                    point(-w / 2.0, d),
                ]);
                let area = poly::area(&ring);
                if area > best_area
                    && poly::subtract(std::slice::from_ref(&ring), std::slice::from_ref(land))
                        .is_empty()
                {
                    best_area = area;
                    best = Some(ring);
                }
            }
        }
        if best.is_some() {
            break;
        }
    }
    best
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::block::LotPlan;

    fn lot(outline: Vec<Point>) -> (Lots, RoadNetwork) {
        let mut roads = RoadNetwork::default();
        let a = roads.add_node(0.0, 0.0);
        let b = roads.add_node(100.0, 0.0);
        roads.add_road(a, b, RoadClass::Street);
        roads.rebuild_adjacency();
        let mut lots = Lots::default();
        lots.push(
            LotPlan {
                road: 0,
                frontage: 16.0,
                front: (18.0, 0.0),
                facing: (0.0, -1.0),
                outline,
                cells: vec![0],
            },
            [0.0, 0.0, 100.0, 100.0],
            1,
        );
        (lots, roads)
    }

    #[test]
    fn houses_have_yards_industry_has_gaps_and_dense_buildings_keep_party_walls() {
        let (lots, roads) = lot(vec![(10.0, 4.0), (26.0, 4.0), (26.0, 60.0), (10.0, 60.0)]);
        for zone in [
            Zone::ResidentialLow,
            Zone::IndustrialLow,
            Zone::ResidentialHigh,
            Zone::CommercialLow,
        ] {
            let outline = make(&lots, 0, &roads, zone, &mut Rng::new(1)).unwrap();
            let r = poly::ring(outline);
            let bounds = poly::bounds(&r);
            assert!(
                poly::subtract(&[r], &[poly::ring(lots.outline(0).iter().copied())]).is_empty()
            );
            match zone {
                Zone::ResidentialLow => {
                    assert!(bounds[0] > 10.0 && bounds[2] < 26.0);
                    assert!(bounds[1] >= 8.0 && bounds[3] < 60.0);
                }
                Zone::IndustrialLow => {
                    assert!(bounds[0] >= 12.0 && bounds[2] <= 24.0);
                    assert!(bounds[1] >= 6.79 && bounds[3] <= 58.0);
                }
                _ => {
                    assert_eq!((bounds[0], bounds[2]), (10.0, 26.0));
                    assert!((bounds[1] - 4.8).abs() < 0.01);
                }
            }
        }
    }

    #[test]
    fn irregular_and_concave_lots_never_produce_outside_footprints() {
        for outline in [
            vec![(10.0, 4.0), (26.0, 4.0), (40.0, 50.0), (20.0, 60.0)],
            vec![
                (10.0, 4.0),
                (26.0, 4.0),
                (26.0, 30.0),
                (19.0, 30.0),
                (19.0, 18.0),
                (10.0, 18.0),
            ],
        ] {
            let (lots, roads) = lot(outline);
            for zone in [
                Zone::ResidentialLow,
                Zone::IndustrialHigh,
                Zone::CommercialHigh,
            ] {
                let ring = make(&lots, 0, &roads, zone, &mut Rng::new(1)).unwrap();
                let inside = poly::ring(ring);
                let outside = poly::subtract(
                    std::slice::from_ref(&inside),
                    &[poly::ring(lots.outline(0).iter().copied())],
                );
                assert!(outside.is_empty());
                assert!(poly::area(&inside) >= MIN_FOOTPRINT);
            }
        }
    }

    #[test]
    fn industrial_yards_vary_without_changing_a_given_seeds_footprint() {
        let (mut lots, roads) = lot(vec![(0.0, 4.0), (64.0, 4.0), (64.0, 60.0), (0.0, 60.0)]);
        lots.frontage[0] = 64.0;
        lots.front_x[0] = 32.0;
        let make_seed =
            |seed| make(&lots, 0, &roads, Zone::IndustrialHigh, &mut Rng::new(seed)).unwrap();
        let first = make_seed(1);
        assert_eq!(first, make_seed(1));
        for seed in 2..10 {
            let ring = make_seed(seed);
            assert_ne!(ring, first);
            let bounds = poly::bounds(&poly::ring(ring.clone()));
            assert!(bounds[0] >= 2.0 && bounds[2] <= 62.0);
            assert!(bounds[1] >= 6.79 && bounds[3] <= 58.0);
            assert!(
                poly::subtract(
                    &[poly::ring(ring)],
                    &[poly::ring(lots.outline(0).iter().copied())]
                )
                .is_empty()
            );
        }
    }

    #[test]
    fn dense_buildings_clear_the_wider_main_road_surface() {
        let (lots, _) = lot(vec![(10.0, 4.0), (26.0, 4.0), (26.0, 60.0), (10.0, 60.0)]);
        let mut roads = RoadNetwork::default();
        let a = roads.add_node(0.0, 0.0);
        let b = roads.add_node(100.0, 0.0);
        roads.add_road(a, b, RoadClass::Main);
        roads.rebuild_adjacency();
        let ring = make(&lots, 0, &roads, Zone::CommercialHigh, &mut Rng::new(1)).unwrap();
        assert!(
            ring.iter().all(|p| p.1 >= 6.79),
            "main-road asphalt is 12 m wide"
        );
    }

    #[test]
    fn corner_houses_clear_a_main_road_beside_the_lot() {
        let (lots, _) = lot(vec![(10.0, 4.0), (26.0, 4.0), (26.0, 60.0), (10.0, 60.0)]);
        let mut roads = RoadNetwork::default();
        let a = roads.add_node(6.0, 0.0);
        let b = roads.add_node(100.0, 0.0);
        let c = roads.add_node(6.0, 100.0);
        roads.add_road(a, b, RoadClass::Street);
        roads.add_road(a, c, RoadClass::Main);
        roads.rebuild_adjacency();
        let ring = make(&lots, 0, &roads, Zone::ResidentialLow, &mut Rng::new(1)).unwrap();
        assert!(
            ring.iter().all(|p| p.0 >= 12.79),
            "the house must not extend into side-road asphalt"
        );
        assert_eq!(ring.len(), 4, "shrinking a house must keep it a box");
    }

    #[test]
    fn too_small_lots_stay_open() {
        let (lots, roads) = lot(vec![(16.0, 4.0), (18.0, 4.0), (18.0, 8.0), (16.0, 8.0)]);
        for zone in [
            Zone::ResidentialLow,
            Zone::IndustrialLow,
            Zone::ResidentialHigh,
        ] {
            assert!(make(&lots, 0, &roads, zone, &mut Rng::new(1)).is_none());
        }
    }
}
