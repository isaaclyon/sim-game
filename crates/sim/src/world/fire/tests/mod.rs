//! Fire tests (ADR 0053). Fixtures grow a small powered, watered town on a lattice.

mod burning;
mod stations;
mod trucks;

use super::super::World;
use crate::building::BuildingId;
use crate::power::PlantKind;
use crate::water::WaterKind;
use crate::{Command, Config, Zone};

fn advance(w: &mut World, seconds: u32) {
    for _ in 0..seconds * 10 {
        w.tick();
    }
}

/// Roads every 120 m, `cols` × `rows` junctions from the origin.
fn lattice(cols: u32, rows: u32) -> World {
    let mut w = World::new(
        Config::default(),
        crate::road::RoadNetwork::generate_lattice(cols, rows, 120.0),
        crate::rng::Rng::new(1),
    );
    w.treasury.balance = 10_000_000;
    w
}

/// A 4 × 3 lattice (x = 0..360, y = 0..240) with every block zoned, a coal plant on the east
/// road and a pump on the north road for the dense homes (ADR 0052), grown for `seconds`.
fn town(seconds: u32) -> World {
    let mut w = lattice(4, 3);
    for (x, zone) in [
        (60.0, Zone::ResidentialHigh),
        (180.0, Zone::CommercialLow),
        (300.0, Zone::IndustrialLow),
    ] {
        for y in [60.0, 180.0] {
            assert!(w.apply(Command::FillBlock { x, y, zone }));
        }
    }
    assert!(w.apply(Command::PlacePlant {
        kind: PlantKind::Coal,
        x: 391.0,
        y: 60.0,
    }));
    assert!(w.apply(Command::PlaceWater {
        kind: WaterKind::Pump,
        x: 180.0,
        y: 260.0,
    }));
    advance(&mut w, seconds);
    w
}

fn station(w: &mut World, x: f32, y: f32) -> bool {
    w.apply(Command::PlaceStation { x, y })
}

fn standing(w: &World) -> Vec<BuildingId> {
    w.buildings.active_buildings().collect()
}

/// The standing building with the most households, so destroying it matters.
fn busiest_home(w: &World) -> BuildingId {
    standing(w)
        .into_iter()
        .filter(|&b| w.buildings.zone[b as usize] == Zone::ResidentialHigh)
        .max_by_key(|&b| households(w, b))
        .expect("the town has homes")
}

fn households(w: &World, b: BuildingId) -> usize {
    (0..w.homes.building.len())
        .filter(|&h| w.homes.building[h] == b && w.homes.household[h].is_some())
        .count()
}
