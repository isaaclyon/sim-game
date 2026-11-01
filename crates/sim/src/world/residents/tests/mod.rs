use super::super::super::World;
use crate::citizen::Activity;
use crate::{Command, Config, Zone};

mod allocation;
mod occupancy;
mod schedules;
mod trips;

fn advance(w: &mut World, seconds: u32) {
    for _ in 0..seconds * 10 {
        w.tick();
    }
}
fn town() -> World {
    let mut w = World::new(
        Config::default(),
        crate::road::RoadNetwork::generate_lattice(4, 3, 120.0),
        crate::rng::Rng::new(1),
    );
    for (x, zone) in [
        (50.0, Zone::ResidentialLow),
        (170.0, Zone::CommercialLow),
        (290.0, Zone::IndustrialLow),
    ] {
        assert!(w.apply(Command::FillBlock { x, y: 50.0, zone }));
    }
    advance(&mut w, 120);
    w
}
fn isolate(w: &mut World) {
    for c in 0..w.citizens.active.len() as u32 {
        w.cancel_citizen_trip(c);
        let i = c as usize;
        w.citizens.location[i] = w.home_building(c);
        w.citizens.activity[i] = Activity::Home;
    }
    w.residents.activities.clear();
}
fn journey(w: &mut World, c: u32, b: u32, activity: Activity) {
    w.citizens.destination[c as usize] = Some(b);
    w.citizens.activity[c as usize] = activity;
    w.start_citizen_trip(c);
}
fn distant(w: &World, c: u32) -> u32 {
    let from = w.building_node(w.citizens.location[c as usize]);
    w.buildings
        .active_buildings()
        .find(|&b| {
            w.building_node(b) != from
                && matches!(
                    w.buildings.zone[b as usize],
                    Zone::CommercialLow | Zone::IndustrialLow
                )
        })
        .unwrap()
}
