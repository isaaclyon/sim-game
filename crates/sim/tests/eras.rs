//! Eras (ADR 0059): a new city is a village that refuses main roads and dense zones; its
//! buildings keep the era they were built in.

use sim::road::RoadClass;
use sim::{Command, Config, Era, World, Zone};

fn village() -> World {
    World::empty(Config::default(), 20, 20)
}

fn road(ax: f32, ay: f32, bx: f32, by: f32, class: RoadClass) -> Command {
    Command::BuildRoad {
        ax,
        ay,
        bx,
        by,
        class,
    }
}

/// A block of streets 240 × 120 m with houses zoned inside.
fn hamlet(w: &mut World) {
    for (ax, ay, bx, by) in [
        (0.0, 0.0, 240.0, 0.0),
        (0.0, 120.0, 240.0, 120.0),
        (0.0, 0.0, 0.0, 120.0),
        (240.0, 0.0, 240.0, 120.0),
    ] {
        assert!(w.apply(road(ax, ay, bx, by, RoadClass::Street)));
    }
    assert!(w.apply(Command::FillBlock {
        x: 120.0,
        y: 60.0,
        zone: Zone::ResidentialLow,
    }));
}

fn grow(w: &mut World, ticks: u32) {
    for _ in 0..ticks {
        w.tick();
    }
}

#[test]
fn a_new_city_is_a_village_and_a_generated_grid_a_town() {
    assert_eq!(village().era(), Era::Village);
    assert_eq!(village().stats().era, Era::Village);
    assert_eq!(World::generate(Config::default(), 4, 4).era(), Era::Town);
}

#[test]
fn a_village_refuses_main_roads_and_dense_zones_and_changes_nothing() {
    let mut w = village();
    hamlet(&mut w);
    let hash = w.state_hash();
    let refused = [
        road(0.0, 300.0, 240.0, 300.0, RoadClass::Main),
        Command::BuildCurve {
            ax: 0.0,
            ay: 300.0,
            cx: 120.0,
            cy: 400.0,
            bx: 240.0,
            by: 300.0,
            class: RoadClass::Main,
            chain: false,
        },
        Command::BuildGrid {
            ox: 0.0,
            oy: 300.0,
            ux: 1.0,
            uy: 0.0,
            cell: 80.0,
            cols: 2,
            rows: 2,
            class: RoadClass::Main,
        },
        Command::SetRoadClass {
            x: 120.0,
            y: 0.0,
            class: RoadClass::Main,
        },
        Command::FillBlock {
            x: 120.0,
            y: 60.0,
            zone: Zone::ResidentialHigh,
        },
        Command::PaintZone {
            ax: 10.0,
            ay: 10.0,
            bx: 60.0,
            by: 60.0,
            zone: Zone::CommercialHigh,
        },
        Command::PaintZone {
            ax: 10.0,
            ay: 10.0,
            bx: 60.0,
            by: 60.0,
            zone: Zone::IndustrialHigh,
        },
    ];
    for cmd in refused {
        assert!(!w.apply(cmd.clone()), "{cmd:?}");
        assert_eq!(w.state_hash(), hash, "{cmd:?} changed the city");
    }
    // Previews plan nothing a build would refuse.
    let ends = [0.0, 300.0, 0.0, 240.0, 300.0, 0.0];
    assert_eq!(w.draft_cost(&ends, RoadClass::Main), None);
    assert!(w.draft_cost(&ends, RoadClass::Street).is_some());
    assert_eq!(w.class_cost(120.0, 0.0, RoadClass::Main), None);
    let edge = [0.0, 300.0, 0.0, 80.0, 300.0, 0.0];
    assert!(w.grid_command(&edge, None, 80.0, RoadClass::Main).is_none());
    assert!(
        w.grid_command(&edge, None, 80.0, RoadClass::Street)
            .is_some()
    );

    // The preview messages the browser draws: no stops, no grid, no road to change.
    let mut street = Vec::new();
    let mut main = Vec::new();
    w.write_road_draft(&ends, true, RoadClass::Street, &mut street);
    w.write_road_draft(&ends, true, RoadClass::Main, &mut main);
    assert!(main.len() < street.len(), "a locked class plans no stops");
    let mut grid = Vec::new();
    w.write_grid_draft(&edge, None, 80.0, RoadClass::Main, &mut grid);
    assert_eq!((grid[5], grid[6]), (0.0, 0.0), "no blocks");
    assert!(grid[7].is_nan(), "no cost");
    let mut class = vec![1.0];
    w.write_class_preview(120.0, 0.0, RoadClass::Main, &mut class);
    assert!(class.is_empty());

    // Streets, low-density zones and unzoning stay open.
    assert!(w.apply(road(0.0, 300.0, 240.0, 300.0, RoadClass::Street)));
    for zone in [Zone::CommercialLow, Zone::IndustrialLow, Zone::None] {
        assert!(w.apply(Command::PaintZone {
            ax: 10.0,
            ay: 10.0,
            bx: 60.0,
            by: 60.0,
            zone,
        }));
    }
}

#[test]
fn buildings_keep_the_era_they_were_built_in() {
    let mut w = village();
    hamlet(&mut w);
    grow(&mut w, 2_000);
    let built: Vec<_> = w.buildings.active_buildings().collect();
    assert!(!built.is_empty(), "the hamlet grew houses");
    assert!(
        built
            .iter()
            .all(|&b| w.buildings.era[b as usize] == Era::Village)
    );
    let mut data = Vec::new();
    w.buildings.write_snapshot(&w.lots, &mut data);
    assert_eq!(
        data[4],
        Era::Village as u8 as f32,
        "the record carries the era"
    );

    let mut town = World::generate(
        Config {
            ignore_demand: true,
            ..Config::default()
        },
        4,
        4,
    );
    assert!(town.apply(Command::FillBlock {
        x: 60.0,
        y: 60.0,
        zone: Zone::ResidentialLow,
    }));
    grow(&mut town, 700);
    assert!(town.buildings.active_buildings().count() > 0);
    assert!(
        town.buildings
            .active_buildings()
            .all(|b| town.buildings.era[b as usize] == Era::Town)
    );
}

#[test]
fn the_era_and_building_eras_survive_a_save() {
    let mut w = village();
    hamlet(&mut w);
    grow(&mut w, 2_000);
    let loaded = World::load(&w.save().unwrap()).unwrap();
    assert_eq!(loaded.era(), Era::Village);
    assert_eq!(loaded.buildings.era, w.buildings.era);
    assert_eq!(loaded.state_hash(), w.state_hash());

    let town = World::empty(Config::default(), 20, 20).with_era(Era::Town);
    let loaded = World::load(&town.save().unwrap()).unwrap();
    assert_eq!(loaded.era(), Era::Town);
    assert_ne!(loaded.state_hash(), village().state_hash());
}

#[test]
fn village_streets_cost_half_a_towns() {
    let (village, town) = (village(), village().with_era(Era::Town));
    let price = |w: &World| {
        w.road_cost(0.0, 0.0, 200.0, 0.0, RoadClass::Street)
            .unwrap()
    };
    assert_eq!(price(&town), 2_000);
    assert_eq!(price(&village), 1_000);

    let mut w = village;
    let before = w.money().balance;
    assert!(w.apply(road(0.0, 0.0, 200.0, 0.0, RoadClass::Street)));
    assert_eq!(
        before - w.money().balance,
        1_000,
        "the village price is what it pays"
    );
}
