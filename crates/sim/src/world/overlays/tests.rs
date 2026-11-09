use super::{GAP_FIRE, GAP_POWER, GAP_WATER, TRAFFIC_STRIDE};
use crate::power::PlantKind;
use crate::rng::Rng;
use crate::road::{RoadClass, RoadNetwork};
use crate::water::WaterKind;
use crate::world::World;
use crate::world::fire::FIRE_THRESHOLD;
use crate::{Command, Config, Zone};

/// One straight 100 m street from (0, 0) to (100, 0): lanes 0 and 1.
fn street() -> World {
    let mut net = RoadNetwork::default();
    let (a, b) = (net.add_node(0.0, 0.0), net.add_node(100.0, 0.0));
    net.add_road(a, b, RoadClass::Street);
    let mut w = World::new(Config::default(), net, Rng::new(1));
    w.traffic.ensure_lanes(w.roads.lane_count());
    w
}

/// Puts a car on lane 0 at `pos` metres, driving at `speed` m/s.
fn car(w: &mut World, pos: f32, speed: f32) {
    let agent = w.agents.push(w.roads.lane_from[0]);
    w.traffic.enter(0, agent, &mut w.agents);
    w.agents.pos[agent as usize] = pos;
    w.agents.speed[agent as usize] = speed;
}

fn traffic(w: &World) -> Vec<f32> {
    let mut out = Vec::new();
    w.write_road_traffic(&mut out);
    out
}

#[test]
fn empty_roads_send_nothing() {
    assert!(traffic(&street()).is_empty());
}

#[test]
fn a_car_at_the_limit_flows_and_a_full_stopped_lane_is_jammed() {
    let mut w = street();
    let limit = w.roads.lane_speed[0];
    car(&mut w, 50.0, limit);
    let out = traffic(&w);
    assert_eq!(out.len(), TRAFFIC_STRIDE);
    assert_eq!(&out[..6], &[0.0, 0.0, 100.0, 0.0, 0.0, 0.0]);
    assert_eq!(out[6], 0.0, "one car at the limit is no jam");

    let mut w = street();
    for k in 0..14 {
        car(&mut w, 95.0 - 7.0 * k as f32, 0.0);
    }
    let jam = traffic(&w)[6];
    assert!(jam > 0.95 && jam <= 1.0, "{jam}");

    let mut w = street();
    car(&mut w, 95.0, 0.0);
    let jam = traffic(&w)[6];
    assert!(
        jam > 0.0 && jam < 0.1,
        "one car at a light is a short queue: {jam}"
    );
}

/// A 4 × 3 lattice with dense homes, shops and industry, a coal plant and a pump, grown for
/// ten minutes; also where the pump and the plant stand.
fn town() -> (World, (f32, f32), (f32, f32)) {
    let mut w = World::new(
        Config::default(),
        RoadNetwork::generate_lattice(4, 3, 120.0),
        Rng::new(1),
    );
    w.treasury.balance = 10_000_000;
    for (x, zone) in [
        (60.0, Zone::ResidentialHigh),
        (180.0, Zone::CommercialLow),
        (300.0, Zone::IndustrialLow),
    ] {
        for y in [60.0, 180.0] {
            assert!(w.apply(Command::FillBlock { x, y, zone }));
        }
    }
    let (x, y) = (391.0, 60.0);
    let plant = w.plant_spot(PlantKind::Coal, x, y).unwrap().centre;
    assert!(w.apply(Command::PlacePlant {
        kind: PlantKind::Coal,
        x,
        y
    }));
    let pump = w.water_spot(WaterKind::Pump, 180.0, 260.0).unwrap().centre;
    assert!(w.apply(Command::PlaceWater {
        kind: WaterKind::Pump,
        x: 180.0,
        y: 260.0
    }));
    for _ in 0..6_000 {
        w.tick();
    }
    (w, pump, plant)
}

/// Each building's gap bits and lot outline point count.
fn gaps(w: &World) -> Vec<(u8, usize)> {
    let mut out = Vec::new();
    w.write_service_gaps(&mut out);
    let mut i = 0;
    let mut found = Vec::new();
    while i < out.len() {
        let n = out[i + 1] as usize;
        found.push((out[i] as u8, n));
        i += 2 + 2 * n;
    }
    found
}

#[test]
fn service_gaps_follow_power_water_and_fire_cover() {
    let (mut w, pump, plant) = town();
    let served = gaps(&w);
    assert_eq!(served.len(), w.buildings.active_buildings().count());
    assert!(served.len() > 6, "the town grew");
    assert!(served.iter().all(|&(g, n)| g == 0 && n >= 3), "{served:?}");

    // Fire cover only counts once fires can start.
    w.residents.population = FIRE_THRESHOLD;
    assert!(w.fire_gaps_shown());
    assert!(gaps(&w).iter().all(|&(g, _)| g == GAP_FIRE));
    assert!(w.apply(Command::SetFires { on: false }));
    assert!(gaps(&w).iter().all(|&(g, _)| g == 0));
    assert!(w.apply(Command::SetFires { on: true }));
    assert!(w.apply(Command::PlaceStation { x: -20.0, y: 60.0 }));
    w.tick();
    w.residents.population = FIRE_THRESHOLD;
    assert!(gaps(&w).iter().all(|&(g, _)| g & GAP_FIRE == 0));

    // Without the pump, dense homes are dry; low-density buildings use wells.
    assert!(w.apply(Command::DeleteRoad {
        x: pump.0,
        y: pump.1
    }));
    w.tick();
    let dense = w
        .buildings
        .active_buildings()
        .map(|b| w.buildings.zone[b as usize].is_dense())
        .collect::<Vec<_>>();
    let dry = gaps(&w);
    assert!(dense.iter().any(|&d| d));
    for (&(g, _), d) in dry.iter().zip(dense) {
        assert_eq!(g & GAP_WATER != 0, d, "{dry:?}");
        assert_eq!(g & GAP_POWER, 0);
    }

    // Without the plant, only the starter power is left, for the oldest buildings.
    assert!(w.apply(Command::DeleteRoad {
        x: plant.0,
        y: plant.1
    }));
    w.tick();
    let after = gaps(&w);
    let dark = after.iter().filter(|&&(g, _)| g & GAP_POWER != 0).count();
    assert!(dark > 0 && dark < after.len(), "{dark} of {}", after.len());
}
