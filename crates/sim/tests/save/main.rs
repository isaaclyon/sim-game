//! Full-state saves (ADR 0033): a loaded world matches the saved one and evolves identically.
mod rejects;

use sim::road::RoadClass;
use sim::{Command, Config, World, Zone};

fn advance(w: &mut World, seconds: u32) {
    for _ in 0..seconds * 10 {
        w.tick();
    }
}

/// A town with traffic, walkers, a closing block and lots still queued to grow.
fn busy_town() -> World {
    let mut w = World::new(
        Config {
            seed: 7,
            ignore_demand: true,
            ..Config::default()
        },
        sim::road::RoadNetwork::generate_lattice(5, 3, 120.0),
        sim::rng::Rng::new(7),
    );
    for (x, zone) in [
        (50.0, Zone::ResidentialLow),
        (170.0, Zone::CommercialLow),
        (290.0, Zone::IndustrialLow),
        (410.0, Zone::ResidentialHigh),
    ] {
        assert!(w.apply(Command::FillBlock { x, y: 50.0, zone }));
    }
    assert!(w.apply(Command::SpawnAgents { count: 150 }));
    advance(&mut w, 200);
    // Repaint shops to housing: those buildings start closing.
    assert!(w.apply(Command::FillBlock {
        x: 170.0,
        y: 50.0,
        zone: Zone::ResidentialLow,
    }));
    // A fresh block whose lots are still waiting to grow.
    assert!(w.apply(Command::FillBlock {
        x: 50.0,
        y: 170.0,
        zone: Zone::CommercialHigh,
    }));
    for _ in 0..20 {
        w.tick();
    }
    w
}

/// Everything a player or the frontend can observe.
#[derive(Debug, PartialEq)]
struct View {
    hash: u64,
    tick: u64,
    stats: String,
    agents: Vec<u32>,
    roads: Vec<u32>,
    lots: Vec<u32>,
    signals: Vec<u32>,
    buildings: Vec<u32>,
    zones: Vec<u8>,
    citizens: Vec<Option<sim::world::CitizenInfo>>,
    places: Vec<Option<sim::world::BuildingInfo>>,
}

fn bits(v: &[f32]) -> Vec<u32> {
    v.iter().map(|f| f.to_bits()).collect()
}

fn view(w: &World) -> View {
    let mut buf = Vec::new();
    let mut take = |f: &dyn Fn(&mut Vec<f32>)| {
        f(&mut buf);
        bits(&buf)
    };
    View {
        hash: w.state_hash(),
        tick: w.tick_count(),
        stats: format!("{:?}", w.stats()),
        agents: take(&|o| w.write_agent_snapshot(o)),
        roads: take(&|o| w.write_road_segments(o)),
        lots: take(&|o| w.write_lot_outlines(o)),
        signals: take(&|o| w.write_signal_snapshot(o)),
        buildings: take(&|o| w.buildings.write_snapshot(&w.lots, o)),
        zones: w.zones.cells.clone(),
        citizens: (0..w.citizens.active.len() as u32)
            .map(|c| w.citizen_info(c))
            .collect(),
        places: (0..w.buildings.active.len() as u32)
            .map(|b| w.building_info(b))
            .collect(),
    }
}

fn edits() -> [Command; 3] {
    [
        Command::BuildRoad {
            ax: 0.0,
            ay: 240.0,
            bx: 480.0,
            by: 360.0,
            class: RoadClass::Main,
        },
        Command::DeleteRoad { x: 360.0, y: 60.0 },
        Command::FillBlock {
            x: 290.0,
            y: 50.0,
            zone: Zone::None,
        },
    ]
}

#[test]
fn saved_town_has_traffic_walkers_closures_and_queued_growth() {
    let w = busy_town();
    let s = w.stats();
    assert!(s.driving > 0, "{s:?}");
    assert!(s.population > 0);
    assert!(
        (0..w.buildings.active.len() as u32).any(|b| w.building_closing(b)),
        "a block is closing"
    );
    // Walks start over the next minutes; make sure the scenario exercises them.
    let mut later = World::load(&w.save().unwrap()).unwrap();
    let mut walked = false;
    let built = later.buildings.active_buildings().count();
    for _ in 0..1200 {
        later.tick();
        walked |= later.stats().walking > 0;
    }
    assert!(walked, "someone walks");
    assert!(
        later.buildings.active_buildings().count() > built,
        "queued lots grow"
    );
}

#[test]
fn load_restores_every_observable_and_the_same_bytes() {
    let w = busy_town();
    let before = view(&w);
    let bytes = w.save().unwrap();
    assert_eq!(view(&w), before, "saving changes nothing");
    assert_eq!(w.save().unwrap(), bytes, "saving is repeatable");
    let loaded = World::load(&bytes).unwrap();
    assert_eq!(view(&loaded), before);
    assert_eq!(loaded.save().unwrap(), bytes);
}

#[test]
fn loaded_town_evolves_identically_under_ticks_and_edits() {
    let mut a = busy_town();
    let mut b = World::load(&a.save().unwrap()).unwrap();
    let slots = a.buildings.active.len();
    let (mut walked, mut closed) = (false, false);
    let mut both_tick = |a: &mut World, b: &mut World, ticks: u32| {
        for _ in 0..ticks {
            a.tick();
            b.tick();
            walked |= a.stats().walking > 0;
            closed |= (0..a.buildings.active.len())
                .any(|i| !a.buildings.active[i] && a.buildings.zone[i] == Zone::CommercialLow);
        }
    };
    for (step, cmd) in edits().into_iter().enumerate() {
        both_tick(&mut a, &mut b, 300);
        assert_eq!(view(&a), view(&b), "before edit {step}");
        assert!(a.apply(cmd.clone()), "edit {step} changes the town");
        assert!(b.apply(cmd));
        assert_eq!(view(&a), view(&b), "after edit {step}");
    }
    both_tick(&mut a, &mut b, 3000);
    assert!(walked && closed, "walks and closures ran");
    assert!(a.buildings.active.len() > slots, "queued lots grew");
    assert_eq!(view(&a), view(&b));
    assert_eq!(a.save().unwrap(), b.save().unwrap());
}

#[test]
fn a_reloaded_save_of_a_loaded_world_still_matches() {
    let mut a = busy_town();
    let mut b = World::load(&a.save().unwrap()).unwrap();
    for _ in 0..500 {
        a.tick();
        b.tick();
    }
    let mut c = World::load(&b.save().unwrap()).unwrap();
    for _ in 0..500 {
        a.tick();
        c.tick();
    }
    assert_eq!(view(&a), view(&c));
}

#[test]
fn fresh_and_generated_worlds_round_trip() {
    for w in [
        World::generate(Config::default(), 0, 0),
        World::generate(Config::default(), 6, 6),
    ] {
        let mut loaded = World::load(&w.save().unwrap()).unwrap();
        assert_eq!(view(&loaded), view(&w));
        let mut w = w;
        for _ in 0..50 {
            w.tick();
            loaded.tick();
        }
        assert_eq!(view(&loaded), view(&w));
    }
}
