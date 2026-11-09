//! Education levels (ADR 0055 slice 1): arrivals bring a mix, and jobs prefer educated
//! workers without ever staying empty for lack of them.
use super::*;
use crate::citizen::Education;

/// Homes beside dense shops, with water for the shops (ADR 0052).
fn shop_town() -> World {
    let mut w = World::new(
        Config {
            ignore_demand: true,
            ..Config::default()
        },
        crate::road::RoadNetwork::generate_lattice(4, 3, 120.0),
        crate::rng::Rng::new(1),
    );
    for (x, zone) in [(50.0, Zone::ResidentialLow), (170.0, Zone::CommercialHigh)] {
        assert!(w.apply(Command::FillBlock { x, y: 50.0, zone }));
    }
    assert!(w.apply(Command::PlaceWater {
        kind: crate::water::WaterKind::Pump,
        x: -20.0,
        y: 60.0,
    }));
    advance(&mut w, 120);
    w
}

fn adults(w: &World) -> Vec<u32> {
    (0..w.citizens.active.len() as u32)
        .filter(|&c| w.citizens.active[c as usize] && !w.citizens.child[c as usize])
        .collect()
}

/// Puts every adult back on the job market with `level`, unable to take a job above it
/// until `wait` sim seconds from now.
fn reset_seekers(w: &mut World, level: Education, wait: f32) {
    let until = w.totals.tick + w.secs_to_ticks(wait);
    for c in adults(w) {
        w.release_job(c);
        w.citizens.education[c as usize] = level;
        w.citizens.any_job_at[c as usize] = until;
        w.schedule_search(c, 1.0);
    }
}

fn dense_workers(w: &World) -> u32 {
    w.buildings
        .active_buildings()
        .filter(|&b| w.buildings.zone[b as usize] == Zone::CommercialHigh)
        .map(|b| w.residents.workers[b as usize])
        .sum()
}

#[test]
fn arriving_adults_bring_a_mix_and_children_start_with_none() {
    let mut w = town();
    // Nobody has grown up in this young town, so every adult arrived as one.
    for c in 0..w.citizens.active.len() {
        if w.citizens.active[c] {
            let level = w.citizens.education[c];
            assert_eq!(level == Education::None, w.citizens.child[c], "citizen {c}");
        }
    }
    let mut counts = [0u32; 4];
    for _ in 0..2_000 {
        counts[w.arriving_education() as usize] += 1;
    }
    let [none, primary, high, university] = counts;
    assert_eq!(none, 0, "arriving adults have at least primary: {counts:?}");
    assert!(
        primary > high && high > university && university > 0,
        "{counts:?}"
    );
}

#[test]
fn dense_shops_hire_high_school_seekers_at_once() {
    let mut w = shop_town();
    reset_seekers(&mut w, Education::HighSchool, 1e6);
    advance(&mut w, 5);
    assert!(dense_workers(&w) > 0);
}

#[test]
fn less_educated_seekers_fill_dense_shops_only_after_waiting() {
    let mut w = shop_town();
    reset_seekers(&mut w, Education::Primary, 60.0);
    advance(&mut w, 30);
    assert_eq!(
        dense_workers(&w),
        0,
        "primary seekers wait before taking shop jobs"
    );
    advance(&mut w, 300);
    assert!(
        dense_workers(&w) > 0,
        "the jobs never stay empty for lack of schools"
    );
}

#[test]
fn a_new_search_draws_a_wait_and_a_hire_clears_it() {
    let mut w = town();
    let c = adults(&w)[0];
    w.release_job(c);
    w.citizens.any_job_at[c as usize] = u64::MAX;
    w.schedule_search(c, 1.0);
    let at = w.citizens.any_job_at[c as usize];
    let now = w.totals.tick;
    assert!(
        at > now && at < now + w.secs_to_ticks(4_800.0),
        "{at} vs {now}"
    );
    advance(&mut w, 30);
    assert!(w.citizens.job[c as usize].is_some());
    assert_eq!(w.citizens.any_job_at[c as usize], u64::MAX);
}
