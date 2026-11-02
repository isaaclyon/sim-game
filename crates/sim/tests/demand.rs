//! R/C/I demand decides what grows next (ADR 0025).
use sim::{Command, Config, World, Zone};

fn advance(w: &mut World, seconds: u32) {
    for _ in 0..seconds * 10 {
        w.tick();
    }
}

fn fill(w: &mut World, blocks: &[(f32, f32)], zone: Zone) {
    for &(x, y) in blocks {
        assert!(w.apply(Command::FillBlock { x, y, zone }));
    }
}

fn built(w: &World, zones: [Zone; 2]) -> usize {
    w.buildings
        .active_buildings()
        .filter(|&b| zones.contains(&w.buildings.zone[b as usize]))
        .count()
}

const HOMES: [Zone; 2] = [Zone::ResidentialLow, Zone::ResidentialHigh];
const ROW: [(f32, f32); 5] = [
    (60.0, 60.0),
    (180.0, 60.0),
    (300.0, 60.0),
    (420.0, 60.0),
    (540.0, 60.0),
];

#[test]
fn an_empty_town_wants_a_little_of_everything() {
    let w = World::generate(Config::default(), 6, 6);
    let d = w.stats().demand;
    assert!(
        d.residential > 0 && d.commercial > 0 && d.industrial > 0,
        "{d:?}"
    );
}

#[test]
fn housing_without_jobs_stops_growing_until_jobs_arrive() {
    let mut w = World::generate(Config::default(), 6, 6);
    fill(&mut w, &ROW, Zone::ResidentialHigh);
    advance(&mut w, 300);
    let homes = built(&w, HOMES);
    assert!(homes > 0, "the base demand starts a village");
    assert!(
        homes < w.lots.active_lots().count(),
        "zoned lots wait for demand"
    );
    assert!(w.stats().demand.residential <= 0);
    assert!(w.stats().demand.industrial > 0, "residents want jobs");

    let before = w.stats().demand.residential;
    fill(
        &mut w,
        &[(60.0, 300.0), (180.0, 300.0)],
        Zone::IndustrialLow,
    );
    fill(
        &mut w,
        &[(300.0, 300.0), (420.0, 300.0)],
        Zone::CommercialLow,
    );
    advance(&mut w, 60);
    assert!(built(&w, [Zone::IndustrialLow; 2]) > 0);
    assert!(built(&w, [Zone::CommercialLow; 2]) > 0);
    assert!(
        w.stats().demand.residential > before,
        "new jobs raise housing demand"
    );
    advance(&mut w, 300);
    assert!(built(&w, HOMES) > homes, "new jobs pull in residents");
}

#[test]
fn a_single_use_is_capped_by_its_own_demand() {
    let mut w = World::generate(Config::default(), 6, 6);
    fill(&mut w, &ROW, Zone::CommercialLow);
    advance(&mut w, 300);
    let shops = built(&w, [Zone::CommercialLow; 2]);
    assert!(shops > 0 && shops < w.lots.active_lots().count());
    assert!(w.stats().demand.commercial <= 0);
}

#[test]
fn demand_driven_growth_is_deterministic() {
    let run = || {
        let mut w = World::generate(Config::default(), 6, 6);
        fill(&mut w, &ROW, Zone::ResidentialLow);
        fill(&mut w, &[(60.0, 300.0)], Zone::IndustrialLow);
        fill(&mut w, &[(180.0, 300.0)], Zone::CommercialLow);
        advance(&mut w, 400);
        (w.state_hash(), w.stats().demand)
    };
    assert_eq!(run(), run());
}
