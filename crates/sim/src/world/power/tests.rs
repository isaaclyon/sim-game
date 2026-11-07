use super::super::World;
use crate::power::{PlantKind, STARTER_POWER};
use crate::{Command, Config, Zone};

fn advance(w: &mut World, seconds: u32) {
    for _ in 0..seconds * 10 {
        w.tick();
    }
}

/// Roads at x = 0..360 and y = 0..240, every 120 m.
fn lattice() -> World {
    World::new(
        Config::default(),
        crate::road::RoadNetwork::generate_lattice(4, 3, 120.0),
        crate::rng::Rng::new(1),
    )
}

/// Every block zoned and a pump, so growth is limited by demand and power only.
fn zoned() -> World {
    let mut w = lattice();
    // Apartments need water (ADR 0052).
    assert!(w.apply(Command::PlaceWater {
        kind: crate::water::WaterKind::Pump,
        x: -20.0,
        y: 60.0,
    }));

    for (x, zone) in [
        (60.0, Zone::ResidentialHigh),
        (180.0, Zone::CommercialLow),
        (300.0, Zone::IndustrialLow),
    ] {
        for y in [60.0, 180.0] {
            assert!(w.apply(Command::FillBlock { x, y, zone }));
        }
    }
    w
}

fn plant(w: &mut World, kind: PlantKind, x: f32, y: f32) -> bool {
    w.apply(Command::PlacePlant { kind, x, y })
}

fn line(w: &mut World, a: (f32, f32), b: (f32, f32)) -> bool {
    w.apply(Command::BuildLine {
        ax: a.0,
        ay: a.1,
        bx: b.0,
        by: b.1,
    })
}

/// Coal beside the east road, clear of it.
const COAL: (f32, f32) = (391.0, 60.0);

#[test]
fn starter_power_caps_growth_until_a_plant_arrives() {
    let mut w = zoned();
    advance(&mut w, 900);
    let s = w.power_stats();
    assert!(s.waiting > 0, "lots wait for power: {s:?}");
    assert!(s.used <= STARTER_POWER && s.supply == 0, "{s:?}");
    assert_eq!(s.unpowered, 0, "nothing is built that would be dark");
    let before = w.buildings.active_buildings().count();

    assert!(plant(&mut w, PlantKind::Coal, COAL.0, COAL.1));
    advance(&mut w, 900);
    let s = w.power_stats();
    assert!(s.supply > 0 && s.used > STARTER_POWER, "{s:?}");
    assert!(w.buildings.active_buildings().count() > before);
}

#[test]
fn unzoning_clears_the_power_waiting_warning_immediately() {
    let mut w = zoned();
    advance(&mut w, 900);
    assert!(w.power_stats().waiting > 0);
    assert!(w.apply(Command::PaintZone {
        ax: -1000.0,
        ay: -1000.0,
        bx: 1000.0,
        by: 1000.0,
        zone: Zone::None,
    }));
    assert!(
        w.lots.active_lots().count() > 0,
        "lots remain after unzoning"
    );
    assert_eq!(w.power_stats().waiting, 0);
    advance(&mut w, 60);
    assert_eq!(w.power_stats().waiting, 0);
}

#[test]
fn loading_preserves_power_waiting_and_subsequent_growth_checks() {
    let mut w = zoned();
    advance(&mut w, 900);
    assert!(w.power_stats().waiting > 0);
    let mut loaded = World::load(&w.save().unwrap()).unwrap();
    assert_eq!(w.power_stats(), loaded.power_stats());
    assert_eq!(w.state_hash(), loaded.state_hash());
    for world in [&mut w, &mut loaded] {
        assert!(plant(world, PlantKind::Coal, COAL.0, COAL.1));
    }
    for _ in 0..600 {
        w.tick();
        loaded.tick();
        assert_eq!(w.power_stats(), loaded.power_stats());
    }
    assert_eq!(w.state_hash(), loaded.state_hash());
    assert_eq!(w.power_stats().waiting, 0);
}

#[test]
fn supply_counts_only_grids_that_serve_a_building() {
    let mut w = zoned();
    assert!(plant(&mut w, PlantKind::Coal, COAL.0, COAL.1));
    advance(&mut w, 900);
    assert!(w.buildings.active_buildings().count() > 0);
    let coal = w.power_stats().supply;
    assert_eq!(coal, w.power_cache.output[0]);

    assert!(plant(&mut w, PlantKind::Wind, 500.0, 60.0));
    assert!(w.power_cache.output[1] > 0, "the turbine is turning");
    assert_eq!(w.power_stats().supply, coal, "a lone turbine powers no one");
    assert!(line(&mut w, (500.0, 60.0), (362.0, 60.0)));
    assert_eq!(w.power_stats().supply, coal + w.power_cache.output[1]);
}

#[test]
fn plants_keep_clear_of_roads_and_each_other() {
    let mut w = lattice();
    assert!(!plant(&mut w, PlantKind::Solar, 120.0, 60.0), "over a road");
    assert!(
        !plant(&mut w, PlantKind::Coal, 500.0, 60.0),
        "coal needs a road"
    );
    assert!(
        plant(&mut w, PlantKind::Solar, 500.0, 60.0),
        "solar does not"
    );
    assert!(
        !plant(&mut w, PlantKind::Wind, 510.0, 60.0),
        "overlaps the farm"
    );
    assert!(!plant(&mut w, PlantKind::Wind, f32::NAN, 60.0));
    assert!(plant(&mut w, PlantKind::Coal, COAL.0, COAL.1));
}

#[test]
fn an_off_road_plant_powers_the_town_only_through_a_line() {
    let mut w = lattice();
    assert!(plant(&mut w, PlantKind::Wind, 500.0, 60.0));
    let mut grids = Vec::new();
    w.write_plant_snapshot(&mut grids);
    let alone = grids[super::PLANT_STRIDE - 1];
    assert!(line(&mut w, (500.0, 60.0), (362.0, 60.0)), "plant to road");
    w.write_plant_snapshot(&mut grids);
    assert_ne!(
        grids[super::PLANT_STRIDE - 1],
        alone,
        "the plant joined the road grid"
    );
    assert!(!line(&mut w, (500.0, 60.0), (501.0, 60.0)), "too short");
}

#[test]
fn a_plant_unzones_and_clears_its_land() {
    let mut w = lattice();
    w.cfg.ignore_demand = true;
    assert!(w.apply(Command::FillBlock {
        x: 60.0,
        y: 60.0,
        zone: Zone::ResidentialLow,
    }));
    advance(&mut w, 300);
    let built = w.buildings.active_buildings().count();
    assert!(built > 0);
    // Lots line the north and south roads; this covers some on the south.
    assert!(plant(&mut w, PlantKind::Solar, 60.0, 30.0));
    assert!(w.buildings.active_buildings().count() < built);
    assert_eq!(w.zones.zone_at(60.0, 30.0), Some(Zone::None));
    let at = w.power.footprint(0);
    assert!(
        w.buildings
            .active_buildings()
            .all(|b| !at.hits_ring(w.buildings.footprint(b)))
    );
}

#[test]
fn a_line_clears_its_corridor_and_holds_lots_until_bulldozed() {
    let mut w = lattice();
    w.cfg.ignore_demand = true;
    assert!(w.apply(Command::FillBlock {
        x: 60.0,
        y: 60.0,
        zone: Zone::ResidentialLow,
    }));
    advance(&mut w, 300);
    let full = w.buildings.active_buildings().count();
    assert!(line(&mut w, (60.0, 0.0), (60.0, 120.0)), "road to road");
    let cut = w.buildings.active_buildings().count();
    assert!(cut < full, "the corridor is cleared");
    advance(&mut w, 300);
    assert_eq!(
        w.buildings.active_buildings().count(),
        cut,
        "nothing rebuilds"
    );
    assert_eq!(w.zones.zone_at(60.0, 60.0), Some(Zone::ResidentialLow));

    let mut target = Vec::new();
    w.write_bulldoze_preview(61.0, 5.0, &mut target);
    // Each end snaps to the sidewalk facing the other: 1 m inside a street's 6 m kerb.
    assert_eq!(target, [60.0, 5.0, 60.0, 115.0], "the line, not a road");
    assert!(w.apply(Command::DeleteRoad { x: 61.0, y: 5.0 }));
    assert_eq!(w.power.active_lines().count(), 0);
    advance(&mut w, 300);
    assert!(
        w.buildings.active_buildings().count() > cut,
        "lots build again"
    );
}

#[test]
fn losing_power_keeps_residents_and_reloads_identically() {
    let mut w = zoned();
    assert!(plant(&mut w, PlantKind::Coal, COAL.0, COAL.1));
    advance(&mut w, 1200);
    let population = w.residents.population;
    assert!(w.power_stats().used > STARTER_POWER);
    assert!(w.apply(Command::DeleteRoad {
        x: COAL.0,
        y: COAL.1
    }));
    let s = w.power_stats();
    assert!(s.unpowered > 0 && s.supply == 0, "{s:?}");
    assert_eq!(w.residents.population, population, "nobody leaves");
    let unpowered = (0..w.buildings.active.len() as u32)
        .find(|&b| w.buildings.active[b as usize] && !w.building_powered(b))
        .unwrap();
    assert!(!w.building_fills_vacancies(unpowered));

    advance(&mut w, 120);
    let mut loaded = World::load(&w.save().unwrap()).unwrap();
    let (mut a, mut b) = (Vec::new(), Vec::new());
    w.write_power_buildings(&mut a);
    loaded.write_power_buildings(&mut b);
    assert_eq!(a, b, "the incremental allocation equals a fresh one");
    assert_eq!(w.power_stats(), loaded.power_stats());
    advance(&mut w, 120);
    advance(&mut loaded, 120);
    assert_eq!(w.state_hash(), loaded.state_hash());
}

#[test]
fn plants_and_lines_are_hashed() {
    let mut a = lattice();
    let mut b = lattice();
    assert_eq!(a.state_hash(), b.state_hash());
    assert!(plant(&mut a, PlantKind::Wind, 500.0, 60.0));
    assert_ne!(a.state_hash(), b.state_hash());
    assert!(plant(&mut b, PlantKind::Wind, 500.0, 60.0));
    assert!(line(&mut a, (500.0, 60.0), (362.0, 60.0)));
    assert_ne!(a.state_hash(), b.state_hash());
}

/// One street from (0, 0) to (300, 300), at 45°.
fn diagonal() -> World {
    let mut roads = crate::road::RoadNetwork::default();
    let a = roads.add_node(0.0, 0.0);
    let b = roads.add_node(300.0, 300.0);
    roads.add_road(a, b, crate::road::RoadClass::Street);
    World::new(Config::default(), roads, crate::rng::Rng::new(1))
}

#[test]
fn coal_snaps_beside_the_nearest_road_and_faces_it() {
    let mut w = lattice();
    // Anywhere east of the x = 360 street within reach lands at the same distance from it.
    for x in [372.0, 391.0, 430.0] {
        let at = w.plant_spot(PlantKind::Coal, x, 60.0).unwrap();
        assert_eq!(at.centre, (360.0 + 6.0 + 1.0 + 22.0, 60.0), "from x = {x}");
        assert_eq!(at.front, (-1.0, 0.0), "faces the street");
    }
    let mut preview = Vec::new();
    w.write_plant_preview(PlantKind::Coal, 430.0, 60.0, &mut preview);
    assert_eq!(preview[..2], [1.0, 1.0], "fits and affordable");
    assert!(plant(&mut w, PlantKind::Coal, 430.0, 60.0));
    let mut ring = Vec::new();
    w.write_bulldoze_preview(389.0, 60.0, &mut ring);
    assert_eq!(ring, preview[2..], "it stands where the preview showed");
    assert!(
        w.plant_spot(PlantKind::Solar, 430.0, 60.0).unwrap().centre == (430.0, 60.0),
        "solar and wind stand where asked"
    );
}

#[test]
fn coal_turns_to_face_a_diagonal_road() {
    let mut w = diagonal();
    let at = w.plant_spot(PlantKind::Coal, 100.0, 160.0).unwrap();
    let s = std::f32::consts::FRAC_1_SQRT_2;
    assert!(
        (at.front.0 - s).abs() < 1e-5 && (at.front.1 + s).abs() < 1e-5,
        "{at:?}"
    );
    let back = 6.0 + 1.0 + 22.0;
    let off = (at.centre.1 - at.centre.0) * s;
    assert!(
        (off - back).abs() < 1e-3,
        "the front is just outside the kerb: {off}"
    );
    assert!(plant(&mut w, PlantKind::Coal, 100.0, 160.0));
    let mut plants = Vec::new();
    w.write_plant_snapshot(&mut plants);
    assert!((plants[4] - s).abs() < 1e-5 && (plants[5] + s).abs() < 1e-5);
    let terminal = w.power.terminal(0);
    assert_eq!(&plants[6..8], [terminal.0, terminal.1]);
    let mut loaded = World::load(&w.save().unwrap()).unwrap();
    assert_eq!(loaded.power.footprint(0), w.power.footprint(0));
    assert_eq!(loaded.state_hash(), w.state_hash());
    loaded.power.plant_fx[0] = 2.0;
    assert!(
        World::load(&loaded.save().unwrap()).is_err(),
        "fronts must be unit"
    );
}

#[test]
fn coal_with_no_road_in_reach_is_refused() {
    let w = lattice();
    let mut preview = Vec::new();
    w.write_plant_preview(PlantKind::Coal, 500.0, 60.0, &mut preview);
    assert_eq!(preview[..2], [0.0, 1.0], "affordable but no road");
    assert_eq!(&preview[2..4], [478.0, 38.0], "shown unsnapped where asked");
}

#[test]
fn lines_snap_to_plant_terminals_and_the_sidewalk_facing_them() {
    let mut w = lattice();
    assert!(plant(&mut w, PlantKind::Wind, 500.0, 60.0));
    let terminal = w.power.terminal(0);
    assert_eq!(terminal, (500.0, 60.0 - 5.0 - 2.0), "behind the back edge");
    let mut ends = Vec::new();
    // Anywhere on or near the turbine joins its terminal; near the street joins its sidewalk
    // on the side the line comes from.
    w.write_line_preview(503.0, 62.0, 358.0, 60.0, &mut ends);
    assert_eq!(ends, [terminal.0, terminal.1, 1.0, 365.0, 60.0, 1.0]);
    w.write_line_preview(450.0, 60.0, 440.0, 60.0, &mut ends);
    assert_eq!(
        ends,
        [450.0, 60.0, 0.0, 440.0, 60.0, 0.0],
        "free ends stay put"
    );
    assert!(line(&mut w, (503.0, 62.0), (358.0, 60.0)));
    let mut plants = Vec::new();
    w.write_plant_snapshot(&mut plants);
    let grid = plants[super::PLANT_STRIDE - 1];
    assert_eq!(
        w.power_cache.grids.node[0] as f32, grid,
        "the road grid reaches the turbine"
    );
}

#[test]
fn powered_homes_still_wait_for_the_land_value_threshold() {
    let mut w = lattice();
    assert!(plant(&mut w, PlantKind::Coal, COAL.0, COAL.1));
    assert!(w.apply(Command::FillBlock {
        x: 60.0,
        y: 60.0,
        zone: Zone::ResidentialLow,
    }));
    // A former factory site with lingering ground pollution but ample power.
    w.pollution.ground.fill(u16::MAX);
    w.pollution.derive(&w.pollution.sources());
    let lot = w.lots.active_lots().next().unwrap();
    assert!(w.land_threshold(lot, Zone::ResidentialLow) > w.demand().residential);
    assert!(w.power_cache.supply.iter().sum::<u32>() > STARTER_POWER);
    advance(&mut w, 300);
    assert_eq!(w.buildings.active_buildings().count(), 0);
    assert_eq!(
        w.power_stats().waiting,
        0,
        "land, not power, holds these lots"
    );

    w.pollution = crate::pollution::Pollution::covering(&w.zones);
    advance(&mut w, 300);
    assert!(w.buildings.active_buildings().count() > 0);
    assert!(
        w.buildings
            .active_buildings()
            .all(|b| w.building_powered(b))
    );
}

#[test]
fn plants_cost_money_and_upkeep_but_lines_and_bulldozing_are_free() {
    use crate::world::plant_price;
    let mut w = lattice();
    let start = w.money();
    let mut balance = start.balance;
    let mut utilities = 0;
    for (kind, x, y) in [
        (PlantKind::Coal, COAL.0, COAL.1),
        (PlantKind::Wind, 500.0, 60.0),
        (PlantKind::Solar, 500.0, 180.0),
    ] {
        assert!(plant(&mut w, kind, x, y));
        let (cost, upkeep) = plant_price(kind);
        balance -= cost;
        utilities += upkeep;
        assert_eq!(w.money().balance, balance, "{kind:?} costs {cost}");
        assert_eq!(w.money().utilities, utilities);
    }
    assert_eq!(w.money().upkeep, start.upkeep, "road upkeep is separate");
    assert!(line(&mut w, (500.0, 60.0), (362.0, 60.0)));
    assert_eq!(w.money().balance, balance, "lines are free");
    for (x, y) in [(450.0, 60.0), (500.0, 60.0), (500.0, 180.0), COAL] {
        assert!(w.apply(Command::DeleteRoad { x, y }));
        assert_eq!(w.money().balance, balance, "power demolition is free");
    }
    assert_eq!(w.money().utilities, 0, "upkeep ends with the plant");
    assert_eq!(w.power.active_plants().count(), 0);
}

#[test]
fn plants_are_refused_when_the_city_cannot_pay() {
    use crate::world::plant_price;
    for kind in [PlantKind::Coal, PlantKind::Solar, PlantKind::Wind] {
        let mut w = lattice();
        let (x, y) = if kind == PlantKind::Coal {
            COAL
        } else {
            (500.0, 60.0)
        };
        w.treasury.balance = plant_price(kind).0 - 1;
        let hash = w.state_hash();
        let mut preview = Vec::new();
        w.write_plant_preview(kind, x, y, &mut preview);
        assert_eq!(
            preview[..2],
            [0.0, 0.0],
            "{kind:?} fits but is unaffordable"
        );
        assert!(!plant(&mut w, kind, x, y), "{kind:?}");
        assert_eq!(w.state_hash(), hash, "a refused plant changes nothing");
        w.treasury.balance = plant_price(kind).0;
        assert!(plant(&mut w, kind, x, y), "{kind:?}");
        assert_eq!(w.money().balance, 0);
    }
}

#[test]
fn an_empty_map_has_starter_power_before_the_first_tick() {
    let w = World::empty(Config::default(), 20, 20);
    assert!(w.power_source(STARTER_POWER, u32::MAX).is_some());
    assert!(w.power_source(STARTER_POWER + 1, u32::MAX).is_none());
    let loaded = World::load(&w.save().unwrap()).unwrap();
    assert!(loaded.power_source(STARTER_POWER, u32::MAX).is_some());
    assert_eq!(w.power_stats(), loaded.power_stats());
}

#[test]
fn coal_plants_pollute_and_solar_and_wind_do_not() {
    let mut clean = lattice();
    assert!(plant(&mut clean, PlantKind::Wind, 500.0, 60.0));
    assert!(plant(&mut clean, PlantKind::Solar, 500.0, 180.0));
    let mut before = Vec::new();
    clean.pollution.write_snapshot(&mut before);
    advance(&mut clean, 120);
    let mut after = Vec::new();
    clean.pollution.write_snapshot(&mut after);
    assert_eq!(before, after, "solar and wind emit nothing");

    let mut w = lattice();
    assert!(plant(&mut w, PlantKind::Coal, COAL.0, COAL.1));
    let at = w.power.footprint(0).centre;
    let fresh = w.pollution_at(at.0, at.1).unwrap();
    assert_eq!(fresh.land_value, 1.0);
    advance(&mut w, 600);
    let s = w.pollution_at(at.0, at.1).unwrap();
    assert!(s.air > 0.5 && s.noise > 0.0 && s.ground > 0.0, "{s:?}");
    assert!(s.land_value < 0.5, "{s:?}");

    // Bulldozing the plant stops air and noise at once; ground pollution lingers.
    assert!(w.apply(Command::DeleteRoad { x: at.0, y: at.1 }));
    advance(&mut w, 20);
    let gone = w.pollution_at(at.0, at.1).unwrap();
    assert_eq!((gone.air, gone.noise), (0.0, 0.0), "{gone:?}");
    assert!(gone.ground > 0.0, "{gone:?}");
}
