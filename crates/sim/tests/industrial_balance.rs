//! Compare equally sized, fully developed blocks rather than building counts.
use sim::{Command, Config, World, Zone};

fn block(seed: u64, zone: Zone) -> (u32, i32) {
    let mut w = World::new(
        Config {
            seed,
            ignore_demand: true,
            ..Config::default()
        },
        sim::road::RoadNetwork::generate_lattice(2, 2, 120.0),
        sim::rng::Rng::new(seed),
    );
    assert!(w.apply(Command::FillBlock {
        x: 60.0,
        y: 60.0,
        zone
    }));
    for _ in 0..1200 {
        w.tick();
    }
    let stats = w.stats();
    // With no residents, demand is the starting allowance minus open jobs.
    let jobs = if matches!(zone, Zone::CommercialLow | Zone::CommercialHigh) {
        8 - stats.demand.commercial
    } else {
        15 - stats.demand.industrial
    };
    (stats.population, jobs)
}

#[test]
fn low_commerce_still_supports_multiple_blocks_of_houses() {
    let (mut residents, mut jobs) = (0, 0);
    for seed in 1..=8 {
        residents += block(seed, Zone::ResidentialLow).0;
        jobs += block(seed, Zone::CommercialLow).1;
    }
    let blocks_supported = jobs as f64 / (residents as f64 * 0.4);
    eprintln!(
        "8 blocks: house residents={residents}, low commercial jobs={jobs}; house blocks supported per shop block={blocks_supported:.2}"
    );
    assert!(
        (5.0..=10.0).contains(&blocks_supported),
        "shop staffing should neither dominate house land use nor make commerce negligible"
    );
}

#[test]
fn industrial_land_supports_houses_and_keeps_up_with_apartments() {
    let (mut residents, mut apartments, mut low_jobs, mut high_jobs) = (0, 0, 0, 0);
    for seed in 1..=8 {
        residents += block(seed, Zone::ResidentialLow).0 as i32;
        apartments += block(seed, Zone::ResidentialHigh).0 as i32;
        low_jobs += block(seed, Zone::IndustrialLow).1;
        high_jobs += block(seed, Zone::IndustrialHigh).1;
    }
    // Industry supplies 60% of jobs; commerce supplies the other 40%.
    let industrial_need = residents as f64 * 0.6;
    let low_blocks = low_jobs as f64 / industrial_need;
    let high_blocks = high_jobs as f64 / industrial_need;
    eprintln!(
        "8 blocks: house residents={residents}, apartment residents={apartments}, low jobs={low_jobs}, high jobs={high_jobs}; residential blocks supported per industrial block: low={low_blocks:.2}, high={high_blocks:.2}"
    );
    assert!(
        low_blocks >= 5.0,
        "low industry should support at least five house blocks"
    );
    assert!(
        high_blocks >= 15.0,
        "high industry should support at least fifteen house blocks"
    );
    let industrial_blocks_per_apartment_block = apartments as f64 * 0.6 / high_jobs as f64;
    assert!(
        industrial_blocks_per_apartment_block <= 3.0,
        "dense housing should not require sprawling industry: {industrial_blocks_per_apartment_block:.2} industrial blocks per apartment block"
    );
    assert!(
        high_jobs >= low_jobs * 18 / 10,
        "high density must offer substantially more jobs on the same land"
    );
}
