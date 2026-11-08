use super::super::World;
use crate::building::BuildingId;
use crate::power::PlantKind;
use crate::water::{PUMP_OUTPUT, STEP_S, TOWER_CAPACITY, WaterKind};
use crate::world::water_price;
use crate::{Command, Config, Zone};

fn advance(w: &mut World, seconds: u32) {
    for _ in 0..seconds * 10 {
        w.tick();
    }
}

/// Sim seconds per water update.
const STEP_SIM_S: u32 = (STEP_S / crate::clock::CLOCK_SECONDS_PER_SIM_SECOND) as u32 + 1;
/// Sim seconds per clock day.
const DAY_SIM_S: u32 = (86_400 / crate::clock::CLOCK_SECONDS_PER_SIM_SECOND) as u32;

/// Roads at x = 0..360 and y = 0..240, every 120 m.
fn lattice() -> World {
    World::new(
        Config::default(),
        crate::road::RoadNetwork::generate_lattice(4, 3, 120.0),
        crate::rng::Rng::new(1),
    )
}

/// Pump beside the west road, tower beside it further south, coal beside the east road.
const PUMP: (f32, f32) = (-20.0, 60.0);
const TOWER: (f32, f32) = (-20.0, 180.0);
const COAL: (f32, f32) = (391.0, 60.0);

fn place(w: &mut World, kind: WaterKind, (x, y): (f32, f32)) -> bool {
    w.apply(Command::PlaceWater { kind, x, y })
}

/// Bulldozes the piece with ID `p` at its centre.
fn bulldoze(w: &mut World, p: u32) -> bool {
    let (x, y) = w.water.footprint(p).centre;
    w.apply(Command::DeleteRoad { x, y })
}

/// Powered, with dense homes and shops and light industry zoned.
fn zoned(dense: Zone, light: Zone) -> World {
    let mut w = lattice();
    w.treasury.balance = 1_000_000;
    assert!(w.apply(Command::PlacePlant {
        kind: PlantKind::Coal,
        x: COAL.0,
        y: COAL.1,
    }));
    for (x, zone) in [(60.0, dense), (180.0, light), (300.0, Zone::IndustrialLow)] {
        for y in [60.0, 180.0] {
            assert!(w.apply(Command::FillBlock { x, y, zone }));
        }
    }
    w
}

fn dense_buildings(w: &World) -> Vec<BuildingId> {
    w.buildings
        .active_buildings()
        .filter(|&b| w.buildings.zone[b as usize].is_dense())
        .collect()
}

#[test]
fn pump_output_falls_with_ground_pollution_under_it() {
    let mut clean = lattice();
    assert!(place(&mut clean, WaterKind::Pump, PUMP));
    assert_eq!(clean.water_stats().supply, PUMP_OUTPUT);

    let mut dirty = lattice();
    let ground = &mut dirty.pollution.ground;
    ground.iter_mut().for_each(|g| *g = u16::MAX);
    assert!(place(&mut dirty, WaterKind::Pump, PUMP));
    assert_eq!(dirty.water_stats().supply, PUMP_OUTPUT / 4);
}

#[test]
fn a_tower_fills_from_spare_output_and_drains_into_shortfalls() {
    let mut w = zoned(Zone::ResidentialHigh, Zone::CommercialHigh);
    assert!(place(&mut w, WaterKind::Pump, PUMP));
    assert!(place(&mut w, WaterKind::Tower, TOWER));
    advance(&mut w, STEP_SIM_S * 3);
    let s = w.water_stats();
    assert_eq!(s.capacity, TOWER_CAPACITY);
    assert!(s.stored > 0, "spare output fills the tower: {s:?}");
    assert!(
        s.stored <= 3 * (s.supply - s.used.min(s.supply)) + PUMP_OUTPUT,
        "{s:?}"
    );
    advance(&mut w, 1500);
    let s = w.water_stats();
    assert!(s.used > 0 && s.dry == 0, "{s:?}");
    // A town at its pump's limit leaves the tower little spare to fill from; top it up so
    // what follows checks the drain.
    w.water.level[1] = s.demand * 2;
    let full = s.demand * 2;

    assert!(bulldoze(&mut w, 0));
    let s = w.water_stats();
    assert_eq!(
        (s.supply, s.dry),
        (0, 0),
        "the tower covers everyone: {s:?}"
    );
    advance(&mut w, STEP_SIM_S);
    let s = w.water_stats();
    assert!(s.stored < full, "the tower drains: {s:?}");
    for _ in 0..200 {
        if w.water_stats().stored == 0 {
            break;
        }
        advance(&mut w, STEP_SIM_S);
    }
    let s = w.water_stats();
    assert!(s.dry > 0, "an empty tower leaves buildings dry: {s:?}");
}

#[test]
fn water_goes_to_the_oldest_dense_buildings_first() {
    let mut w = zoned(Zone::ResidentialHigh, Zone::CommercialHigh);
    // A fire would take buildings for its own reasons (ADR 0053).
    assert!(w.apply(Command::SetFires { on: false }));
    assert!(place(&mut w, WaterKind::Pump, PUMP));
    advance(&mut w, 1500);
    let dense = dense_buildings(&w);
    assert!(dense.len() >= 3, "{dense:?}");
    assert!(dense.iter().all(|&b| w.building_watered(b)));

    // A weaker pump on polluted ground replaces the clean one.
    assert!(bulldoze(&mut w, 0));
    w.pollution.ground.iter_mut().for_each(|g| *g = u16::MAX);
    assert!(place(&mut w, WaterKind::Pump, PUMP));
    let watered: Vec<bool> = dense.iter().map(|&b| w.building_watered(b)).collect();
    let first_dry = watered.iter().position(|&x| !x).expect("someone goes dry");
    assert!(first_dry > 0, "the oldest still has water: {watered:?}");
    assert!(watered[first_dry..].iter().all(|&x| !x), "{watered:?}");
}

#[test]
fn low_density_ignores_water() {
    let mut w = zoned(Zone::ResidentialLow, Zone::CommercialLow);
    advance(&mut w, 1500);
    assert!(w.residents.population > 0);
    assert!(
        w.buildings
            .active_buildings()
            .all(|b| w.building_watered(b))
    );
    let s = w.water_stats();
    assert_eq!((s.demand, s.dry, s.waiting), (0, 0, 0), "{s:?}");
}

#[test]
fn dense_lots_wait_for_water_and_remember_why() {
    let mut w = zoned(Zone::ResidentialHigh, Zone::CommercialHigh);
    advance(&mut w, 900);
    assert!(dense_buildings(&w).is_empty(), "no starter water");
    let s = w.water_stats();
    assert!(s.waiting > 0 && s.dry == 0, "{s:?}");
    assert_eq!(w.power_stats().waiting, 0, "power is not the reason");

    let mut loaded = World::load(&w.save().unwrap()).unwrap();
    assert_eq!(loaded.water_stats(), s);
    assert_eq!(loaded.state_hash(), w.state_hash());
    for world in [&mut w, &mut loaded] {
        assert!(place(world, WaterKind::Pump, PUMP));
        advance(world, 900);
    }
    assert_eq!(loaded.state_hash(), w.state_hash());
    assert!(!dense_buildings(&w).is_empty());
    assert!(w.water_stats().waiting < s.waiting);
}

#[test]
fn dry_buildings_keep_people_for_a_day_then_slowly_empty() {
    let mut w = zoned(Zone::ResidentialHigh, Zone::CommercialHigh);
    // A fire would take buildings for its own reasons (ADR 0053).
    assert!(w.apply(Command::SetFires { on: false }));
    assert!(place(&mut w, WaterKind::Pump, PUMP));
    advance(&mut w, 1500);
    let dense = dense_buildings(&w);
    let homes: Vec<BuildingId> = dense
        .iter()
        .copied()
        .filter(|&b| w.buildings.zone[b as usize] == Zone::ResidentialHigh)
        .collect();
    let shops: Vec<BuildingId> = dense
        .iter()
        .copied()
        .filter(|&b| w.buildings.zone[b as usize] == Zone::CommercialHigh)
        .collect();
    let people = |w: &World, b: BuildingId| {
        let h = &w.households;
        let living = (0..h.active.len())
            .filter(|&i| h.active[i] && w.homes.building[h.home[i] as usize] == b)
            .count() as u32;
        (living, w.residents.workers[b as usize])
    };
    let home = *homes.iter().max_by_key(|&&b| people(&w, b).0).unwrap();
    let shop = *shops.iter().max_by_key(|&&b| people(&w, b).1).unwrap();
    let (residents, _) = people(&w, home);
    let (_, workers) = people(&w, shop);
    assert!(residents >= 2 && workers >= 2, "{residents} {workers}");

    assert!(bulldoze(&mut w, 0));
    assert!(!w.building_watered(home) && !w.building_watered(shop));
    let households = w.households.active.iter().filter(|&&a| a).count();
    advance(&mut w, DAY_SIM_S - 2 * STEP_SIM_S);
    assert_eq!(
        people(&w, home).0,
        residents,
        "nobody leaves in the grace day"
    );
    assert_eq!(people(&w, shop).1, workers);

    advance(&mut w, 3 * STEP_SIM_S);
    let (r, _) = people(&w, home);
    let (_, j) = people(&w, shop);
    // At least one household per building per interval; workers also go with households
    // elsewhere. Dry homes are unhappy too, so more may leave (ADR 0058).
    assert!(
        r < residents && j < workers,
        "{r}/{residents} {j}/{workers}"
    );
    let left = w.households.active.iter().filter(|&&a| a).count();
    assert!(left < households, "households move out of the city");
    let unemployed = w.residents.population - w.residents.employed;
    assert!(unemployed > 0, "the worker stays, unemployed");
    let mut loaded = World::load(&w.save().unwrap()).unwrap();

    // Water returns: nobody else leaves and the building fills again.
    for world in [&mut w, &mut loaded] {
        assert!(place(world, WaterKind::Pump, PUMP));
        advance(world, 1200);
    }
    assert_eq!(w.state_hash(), loaded.state_hash());
    assert!(w.building_watered(home) && w.building_watered(shop));
    assert!(
        w.buildings.active[home as usize],
        "dry buildings are never removed"
    );
    assert!(
        people(&w, home).0 > r || people(&w, shop).1 > j,
        "it fills again"
    );
}

#[test]
fn pumps_and_towers_cost_money_and_upkeep_and_need_a_road() {
    for kind in [WaterKind::Pump, WaterKind::Tower] {
        let (cost, upkeep) = water_price(kind);
        let mut w = lattice();
        let start = w.money();
        assert!(!place(&mut w, kind, (2000.0, 2000.0)), "no road in reach");
        w.treasury.balance = cost - 1;
        let hash = w.state_hash();
        let mut preview = Vec::new();
        w.write_water_preview(kind, PUMP.0, PUMP.1, &mut preview);
        assert_eq!(
            preview[..2],
            [0.0, 0.0],
            "{kind:?} fits but is unaffordable"
        );
        assert!(
            !place(&mut w, kind, PUMP),
            "{kind:?} refused when unaffordable"
        );
        assert_eq!(w.state_hash(), hash);
        w.treasury.balance = cost;
        assert!(place(&mut w, kind, PUMP));
        assert_eq!(w.money().balance, 0);
        assert_eq!(w.money().utilities, start.utilities + upkeep);
        assert!(!place(&mut w, kind, PUMP), "pieces do not overlap");
        assert!(bulldoze(&mut w, 0));
        assert_eq!(w.water.active_pieces().count(), 0);
        assert_eq!(w.money().utilities, start.utilities);
        assert_eq!(w.roads.road_count(), lattice().roads.road_count());
    }
}

#[test]
fn water_state_is_saved_and_hashed() {
    let mut w = zoned(Zone::ResidentialHigh, Zone::CommercialHigh);
    let mut twin = zoned(Zone::ResidentialHigh, Zone::CommercialHigh);
    for world in [&mut w, &mut twin] {
        assert!(place(world, WaterKind::Pump, PUMP));
        assert!(place(world, WaterKind::Tower, TOWER));
        advance(world, 600);
    }
    assert_eq!(w.state_hash(), twin.state_hash(), "deterministic");
    assert!(w.water_stats().stored > 0);
    let loaded = World::load(&w.save().unwrap()).unwrap();
    assert_eq!(loaded.state_hash(), w.state_hash());
    assert_eq!(loaded.water_stats(), w.water_stats());
    let (mut a, mut b) = (Vec::new(), Vec::new());
    w.write_water_pieces(&mut a);
    loaded.write_water_pieces(&mut b);
    assert_eq!(a, b);
    w.write_water_buildings(&mut a);
    loaded.write_water_buildings(&mut b);
    assert_eq!(a, b);

    twin.water.level[1] -= 1;
    assert_ne!(twin.state_hash(), w.state_hash(), "tower levels are hashed");
}
