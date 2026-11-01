//! Footprints use lot polygons, never the zone grid. All clipping uses the fixed polygon grid.
use std::collections::BTreeSet;

use i_overlay::mesh::int::outline::offset::IntOutlineOffset;
use i_overlay::mesh::int::style::IntOutlineStyle;

use crate::block::ROAD_HALF_WIDTH;
use crate::geometry::Point;
use crate::lot::{LotId, Lots};
use crate::poly::{self, Area, Ring};
use crate::road::{RoadClass, RoadNetwork};
use crate::zone::Zone;

const MIN_FOOTPRINT: f32 = 25.0;

pub(super) fn make(lots: &Lots, lot: LotId, roads: &RoadNetwork, zone: Zone) -> Option<Vec<Point>> {
    let l = lot as usize;
    let land = poly::ring(lots.outline(lot).iter().copied());
    let facing = (lots.facing_x[l], lots.facing_y[l]);
    let front = (lots.front_x[l], lots.front_y[l]);
    let ring = match zone {
        Zone::ResidentialLow => house(&land, front, facing, lots.frontage[l]),
        Zone::IndustrialLow | Zone::IndustrialHigh => {
            // Integer offsets are deterministic, including at concave corners.
            let style = IntOutlineStyle::new(-768); // 3 m on poly's 1/256 m grid.
            let inset = land
                .validate_outline(&style)
                .ok()
                .and_then(|()| land.outline(&style).ok());
            inset
                .and_then(largest)
                .or_else(|| house(&land, front, facing, lots.frontage[l]))
        }
        Zone::ResidentialHigh | Zone::CommercialLow | Zone::CommercialHigh => {
            // Leave party walls on lot boundaries intact. Only street-facing edges recede.
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
            largest(poly::subtract(std::slice::from_ref(&land), &streets))
                .or_else(|| house(&land, front, facing, lots.frontage[l]))
        }
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

/// A road-aligned house, normally 4 m behind the kerb with side and back yards.
/// On irregular lots, try successively smaller boxes at nearby sites. Exact polygon
/// subtraction (not just corner containment) prevents boxes crossing concave lot edges.
fn house(land: &Ring, front: Point, facing: Point, frontage: f32) -> Option<Ring> {
    let right = (-facing.1, facing.0);
    let width = (frontage - 4.0).clamp(5.0, 14.0);
    let depth = 14.0;
    let mut best: Option<Ring> = None;
    let mut best_area = MIN_FOOTPRINT;
    for scale in [1.0, 0.85, 0.7, 0.55, 0.4] {
        let (w, d) = (width * scale, depth * scale);
        for back in [ROAD_HALF_WIDTH + 4.0, 14.0, 22.0, 30.0] {
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
            let outline = make(&lots, 0, &roads, zone).unwrap();
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
                Zone::IndustrialLow => assert_eq!(bounds, [13.0, 7.0, 23.0, 57.0]),
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
                let ring = make(&lots, 0, &roads, zone).unwrap();
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
    fn dense_buildings_clear_the_wider_main_road_surface() {
        let (lots, _) = lot(vec![(10.0, 4.0), (26.0, 4.0), (26.0, 60.0), (10.0, 60.0)]);
        let mut roads = RoadNetwork::default();
        let a = roads.add_node(0.0, 0.0);
        let b = roads.add_node(100.0, 0.0);
        roads.add_road(a, b, RoadClass::Main);
        roads.rebuild_adjacency();
        let ring = make(&lots, 0, &roads, Zone::CommercialHigh).unwrap();
        assert!(
            ring.iter().all(|p| p.1 >= 6.79),
            "main-road asphalt is 12 m wide"
        );
    }

    #[test]
    fn too_small_lots_stay_open() {
        let (lots, roads) = lot(vec![(16.0, 4.0), (18.0, 4.0), (18.0, 8.0), (16.0, 8.0)]);
        for zone in [
            Zone::ResidentialLow,
            Zone::IndustrialLow,
            Zone::ResidentialHigh,
        ] {
            assert!(make(&lots, 0, &roads, zone).is_none());
        }
    }
}
