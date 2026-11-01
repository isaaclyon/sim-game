use super::super::World;
use super::Place;
use crate::agents::AgentState;
use crate::citizen::Activity;
use crate::{Command, Config, Zone};

fn advance(w: &mut World, seconds: u32) {
    for _ in 0..seconds * 10 {
        w.tick();
    }
}

/// Homes, shops and industry on one lattice, with residents settled in.
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

fn place(w: &World, b: u32) -> Place {
    Place {
        building: b,
        zone: w.buildings.zone[b as usize],
    }
}

/// Runs until some citizen's car is on the road.
fn driving_citizen(w: &mut World) -> u32 {
    for _ in 0..20_000 {
        w.tick();
        if let Some(a) = (0..w.agents.len()).find(|&a| w.agents.state[a] == AgentState::Driving) {
            return w.agents.citizen[a].expect("normal play has citizen cars only");
        }
    }
    panic!("nobody drove");
}

#[test]
fn reports_resident_facts_as_the_sim_holds_them() {
    let mut w = town();
    let c = driving_citizen(&mut w);
    let i = c as usize;
    let info = w.citizen_info(c).unwrap();
    assert!(info.resident);
    assert!(info.driving);
    assert_eq!(info.agent, w.citizens.agent[i]);
    let hh = w.citizens.household[i] as usize;
    let home = w.homes.building[w.households.home[hh] as usize];
    assert_eq!(info.home, Some(place(&w, home)));
    assert_eq!(info.job, w.citizens.job[i].map(|b| place(&w, b)));
    assert_eq!(info.activity, w.citizens.activity[i]);
    assert_eq!(
        info.destination,
        Some(place(&w, w.citizens.destination[i].unwrap()))
    );
    assert_eq!(info.location, Some(place(&w, w.citizens.location[i])));
}

#[test]
fn a_selected_car_maps_to_its_citizen_and_synthetic_cars_to_nobody() {
    let mut w = town();
    let c = driving_citizen(&mut w);
    let a = w.citizens.agent[c as usize].unwrap();
    assert_eq!(w.citizen_of_agent(a), Some(c));
    let before = w.agents.len() as u32;
    assert!(w.apply(Command::SpawnAgents { count: 1 }));
    assert_eq!(
        w.citizen_of_agent(before),
        None,
        "benchmark traffic has no resident"
    );
    assert_eq!(w.citizen_of_agent(before + 1), None, "unknown slot");
}

#[test]
fn inspecting_changes_nothing() {
    let mut w = town();
    let c = driving_citizen(&mut w);
    let mut twin = town();
    driving_citizen(&mut twin);
    let hash = w.state_hash();
    for id in 0..w.citizens.active.len() as u32 + 3 {
        let _ = w.citizen_info(id);
        let _ = w.citizen_of_agent(id);
    }
    assert_eq!(w.state_hash(), hash);
    // An inspected world keeps running exactly like one nobody looked at.
    let _ = w.citizen_info(c);
    advance(&mut w, 60);
    advance(&mut twin, 60);
    assert_eq!(w.state_hash(), twin.state_hash());
}

#[test]
fn names_are_stable_per_seed_and_shared_surnames_follow_households() {
    let w = town();
    let again = town();
    let n = w.citizens.active.len() as u32;
    assert!(n > 1);
    let names: Vec<_> = (0..n).map(|c| w.citizen_info(c).unwrap().name).collect();
    let repeat: Vec<_> = (0..n)
        .map(|c| again.citizen_info(c).unwrap().name)
        .collect();
    assert_eq!(names, repeat);
    let surname = |s: &str| s.rsplit(' ').next().unwrap().to_owned();
    for c in 0..n as usize {
        assert!(
            names[c].contains(' '),
            "given name and surname: {}",
            names[c]
        );
        let hh = w.citizens.household[c];
        for &m in &w.households.members[hh as usize] {
            assert_eq!(surname(&names[c]), surname(&names[m as usize]));
        }
    }
    let distinct: std::collections::BTreeSet<_> = names.iter().collect();
    assert!(
        distinct.len() * 2 > names.len(),
        "names vary across residents"
    );
}

#[test]
fn unknown_ids_have_no_details() {
    let w = town();
    assert!(w.citizen_info(w.citizens.active.len() as u32).is_none());
    assert!(w.citizen_info(u32::MAX).is_none());
}

#[test]
fn a_resident_who_left_town_shows_no_stale_places() {
    let mut w = town();
    let c = driving_citizen(&mut w);
    let name = w.citizen_info(c).unwrap().name;
    // Remove every building: households with nowhere to go leave town.
    for b in 0..w.buildings.active.len() {
        w.buildings.active[b] = false;
    }
    w.reconcile_residents();
    assert!(!w.citizens.active[c as usize]);
    let info = w
        .citizen_info(c)
        .expect("departed residents stay inspectable");
    assert!(!info.resident);
    assert_eq!(info.name, name, "the name survives leaving town");
    assert!(!info.driving);
    assert_eq!(
        (
            info.home,
            info.job,
            info.location,
            info.destination,
            info.agent
        ),
        (None, None, None, None, None)
    );
}

#[test]
fn places_in_removed_buildings_are_never_reported() {
    let mut w = town();
    let c = driving_citizen(&mut w);
    let i = c as usize;
    // Between a removal and the cleanup that follows it, the raw refs are stale.
    let dest = w.citizens.destination[i].unwrap();
    let from = w.citizens.location[i];
    w.buildings.active[dest as usize] = false;
    w.buildings.active[from as usize] = false;
    let info = w.citizen_info(c).unwrap();
    assert_eq!(info.destination, None);
    assert_eq!(info.location, None);
    if w.citizens.job[i] == Some(dest) || w.citizens.job[i] == Some(from) {
        assert_eq!(info.job, None);
    }
}

#[test]
fn a_parked_resident_reports_where_they_are_and_no_trip() {
    let w = town();
    let c = (0..w.citizens.active.len() as u32)
        .find(|&c| w.citizens.destination[c as usize].is_none())
        .unwrap();
    let info = w.citizen_info(c).unwrap();
    assert!(!info.driving);
    assert_eq!(info.destination, None);
    assert_eq!(
        info.location,
        Some(place(&w, w.citizens.location[c as usize]))
    );
    assert!(matches!(
        info.activity,
        Activity::Home | Activity::Work | Activity::Shop
    ));
}
