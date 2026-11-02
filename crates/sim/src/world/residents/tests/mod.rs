use super::super::super::World;
use crate::citizen::Activity;
use crate::{Command, Config, Zone};

mod allocation;
mod closure;
mod employment;
mod occupancy;
mod schedules;
mod trips;
mod walking;

fn advance(w: &mut World, seconds: u32) {
    for _ in 0..seconds * 10 {
        w.tick();
    }
}
fn town() -> World {
    let mut w = World::new(
        Config {
            ignore_demand: true,
            ..Config::default()
        },
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
/// Replace a citizen's job the way assignment does, keeping every counter consistent.
fn assign_job(w: &mut World, c: u32, b: u32) {
    w.release_job(c);
    w.citizens.job[c as usize] = Some(b);
    w.residents.workers[b as usize] += 1;
    w.residents.employed += 1;
    let assigned = w.citizens.job.iter().filter(|j| j.is_some()).count() as u32;
    assert_eq!(w.residents.employed, assigned);
}
/// Makes citizen `c` an owner who drives every trip that starts from home.
fn driver(w: &mut World, c: u32) {
    w.citizens.owns_car[c as usize] = true;
    w.citizens.walk_limit[c as usize] = 0.0;
}
/// Makes citizen `c` walk every trip, with no car and no practical limit.
fn walker(w: &mut World, c: u32) {
    w.citizens.owns_car[c as usize] = false;
    w.citizens.walk_limit[c as usize] = 1e6;
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
