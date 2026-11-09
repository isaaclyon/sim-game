//! Placing stations, what they cost, and the area they cover.

use super::*;
use crate::world::fire::{COVER_M, STATION_COST, STATION_JOBS, STATION_UPKEEP, TRUCKS_PER_STATION};

/// West of the road at x = 0, facing it.
const WEST: (f32, f32) = (-20.0, 60.0);

#[test]
fn a_station_costs_money_upkeep_and_brings_jobs_and_trucks() {
    let mut w = lattice(4, 3);
    let (balance, upkeep) = (w.treasury.balance, w.treasury.upkeep);
    assert!(station(&mut w, WEST.0, WEST.1));
    assert_eq!(w.treasury.balance, balance - STATION_COST);
    assert_eq!(w.treasury.upkeep, upkeep + STATION_UPKEEP);
    assert_eq!(w.fire_jobs(), STATION_JOBS);
    let s = w.fire_stats();
    assert_eq!(
        (s.stations, s.trucks, s.free_trucks),
        (1, TRUCKS_PER_STATION, TRUCKS_PER_STATION)
    );
    // It faces the road and stands clear of it.
    let at = w.fire.footprint(0);
    assert!(at.centre.0 < 0.0 && at.front.0 > 0.9, "{at:?}");

    // Bulldozing it ends upkeep and jobs and retires its trucks.
    assert!(w.apply(Command::DeleteRoad {
        x: at.centre.0,
        y: at.centre.1
    }));
    assert_eq!(w.treasury.upkeep, upkeep);
    assert_eq!(w.fire_jobs(), 0);
    assert_eq!(w.fire_stats().trucks, 0);
    assert_eq!(
        w.roads.active_roads().count(),
        17,
        "the road beside it stays"
    );
}

#[test]
fn stations_are_refused_off_road_over_pieces_and_without_money() {
    let mut w = lattice(4, 3);
    assert!(!station(&mut w, -500.0, 60.0), "no road in reach");
    assert!(station(&mut w, WEST.0, WEST.1));
    assert!(!station(&mut w, WEST.0, WEST.1 + 5.0), "overlaps the first");
    assert!(!w.apply(Command::PlacePlant {
        kind: PlantKind::Coal,
        x: WEST.0,
        y: WEST.1
    }));
    let mut poor = lattice(4, 3);
    poor.treasury.balance = STATION_COST - 1;
    assert!(!station(&mut poor, WEST.0, WEST.1), "cannot pay");
    let mut preview = Vec::new();
    poor.write_station_preview(WEST.0, WEST.1, &mut preview);
    assert_eq!(preview[..2], [0.0, 0.0], "fits but cannot pay");
    assert!(
        preview.len() > 2,
        "the footprint still shows where it would stand"
    );
    assert_eq!(poor.treasury.balance, STATION_COST - 1);
}

#[test]
fn a_station_clears_the_buildings_and_zoning_under_it() {
    let mut w = town(600);
    let inside = |w: &World| {
        standing(w)
            .into_iter()
            .filter(|&b| {
                let at = w.station_spot(60.0, 220.0).unwrap();
                at.hits_ring(w.buildings.footprint(b))
            })
            .count()
    };
    assert!(
        inside(&w) > 0,
        "the fixture has buildings where the station goes"
    );
    assert!(station(&mut w, 60.0, 220.0));
    assert_eq!(inside(&w), 0);
    advance(&mut w, 300);
    assert_eq!(inside(&w), 0, "nothing regrows under it");
}

#[test]
fn coverage_follows_road_distance_not_straight_lines() {
    // A long street from the station, and a separate street just across from it that no road
    // joins: close as the crow flies, unreachable by truck.
    let mut net = crate::road::RoadNetwork::generate_lattice(16, 1, 120.0);
    let (a, b) = (net.add_node(0.0, 60.0), net.add_node(120.0, 60.0));
    assert!(net.add_road(a, b, crate::road::RoadClass::Street));
    let island = (net.lane_count() / 2 - 1) as u32;
    let mut w = World::new(Config::default(), net, crate::rng::Rng::new(1));
    w.treasury.balance = 1_000_000;
    assert!(station(&mut w, 0.0, -20.0));
    let mut covered: Vec<u32> = w
        .roads
        .active_roads()
        .filter(|&r| w.road_covered(r))
        .collect();
    covered.sort_unstable();
    // Road r joins x = 120 r and 120 (r + 1); one end within 1,200 m covers it.
    let reach = (COVER_M / 120.0) as u32;
    assert_eq!(covered, (0..reach + 1).collect::<Vec<_>>());
    assert!(!w.road_covered(island));
}

#[test]
fn covered_buildings_follow_stations_and_gain_land_value() {
    let mut w = town(600);
    let homes: Vec<_> = standing(&w)
        .into_iter()
        .filter(|&b| w.buildings.zone[b as usize] == Zone::ResidentialHigh)
        .collect();
    assert!(homes.iter().all(|&b| !w.building_covered(b)));
    let centre = |w: &World, b: BuildingId| {
        let ring = w.buildings.footprint(b);
        let n = ring.len() as f32;
        ring.iter()
            .fold((0.0, 0.0), |(x, y), p| (x + p.0 / n, y + p.1 / n))
    };
    let land = |w: &World| -> f32 {
        homes
            .iter()
            .map(|&b| {
                let (x, y) = centre(w, b);
                w.pollution_at(x, y).unwrap().land_value
            })
            .sum()
    };
    let before = land(&w);
    assert!(station(&mut w, WEST.0, WEST.1));
    w.tick();
    let homes: Vec<_> = homes
        .into_iter()
        .filter(|&b| w.buildings.active[b as usize])
        .collect();
    assert!(homes.iter().all(|&b| w.building_covered(b)));
    assert_eq!(w.fire_stats().covered as usize, standing(&w).len());
    advance(&mut w, 120);
    let after: f32 = homes
        .iter()
        .map(|&b| {
            let (x, y) = centre(&w, b);
            w.pollution_at(x, y).unwrap().land_value
        })
        .sum();
    assert!(
        after > before + 0.05 * homes.len() as f32,
        "{before} -> {after}"
    );

    let at = w.fire.footprint(0).centre;
    assert!(w.apply(Command::DeleteRoad { x: at.0, y: at.1 }));
    assert_eq!(w.fire_stats().covered, 0);
}
