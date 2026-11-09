//! Happiness tests (ADR 0058). The town powers and waters everything, so only jobs, commutes,
//! fire cover and pollution move moods unless a test changes them.

use super::{BASE_MOOD, Complaint, LEAVE_BELOW, NEUTRAL_MOOD};
use crate::World;
use crate::{Command, Config, Zone};

fn advance(w: &mut World, seconds: u32) {
    for _ in 0..seconds * 10 {
        w.tick();
    }
}

/// A 4 × 3 lattice with homes, shops and factories, grown for `seconds`.
fn town(seconds: u32) -> World {
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
    advance(&mut w, seconds);
    w
}

fn residents(w: &World) -> impl Iterator<Item = u32> + '_ {
    (0..w.citizens.active.len() as u32).filter(|&c| w.citizens.active[c as usize])
}

#[test]
fn a_trip_to_work_records_its_clock_minutes() {
    let mut w = town(120);
    let c = residents(&w)
        .find(|&c| !w.citizens.child[c as usize])
        .unwrap();
    // 100 ticks of 0.1 s are 10 sim seconds, three clock minutes.
    w.citizens.trip_start[c as usize] = w.totals.tick - 100;
    w.note_commute(c);
    assert_eq!(w.citizens.commute[c as usize], 3);
    w.release_job(c);
    assert_eq!(w.citizens.commute[c as usize], 0, "no job, no commute");
}

#[test]
fn workers_remember_their_morning_commute() {
    // From 6am to past 10am: everyone with a job has gone to work once.
    let w = town(900);
    let commuted = residents(&w)
        .filter(|&c| w.citizens.job[c as usize].is_some() && w.citizens.commute[c as usize] > 0)
        .count();
    assert!(commuted > 0, "some workers took more than a minute");
}

#[test]
fn jobs_and_commutes_matter_to_adults_and_never_to_children() {
    let mut w = town(120);
    let adult = residents(&w)
        .find(|&c| !w.citizens.child[c as usize])
        .unwrap();
    let (before, _) = w.citizen_mood(adult);
    w.release_job(adult);
    w.citizens.commute[adult as usize] = 200;
    let (after, why) = w.citizen_mood(adult);
    assert!(after < before, "{after} < {before}");
    assert!(why.list().contains(&Complaint::NoJob));
    assert!(why.list().contains(&Complaint::LongCommute));
    for c in residents(&w).filter(|&c| w.citizens.child[c as usize]) {
        let why = w.citizen_mood(c).1.list();
        assert!(!why.contains(&Complaint::NoJob) && !why.contains(&Complaint::LongCommute));
    }
}

#[test]
fn a_fire_station_nearby_cheers_people_up() {
    let mut w = town(120);
    w.treasury.balance = 10_000_000;
    let c = residents(&w).next().unwrap();
    let (before, _) = w.citizen_mood(c);
    assert!(w.apply(Command::PlaceStation { x: 60.0, y: 170.0 }));
    w.tick();
    let (after, _) = w.citizen_mood(c);
    assert_eq!(i32::from(after), (i32::from(before) + 10).min(100));
}

#[test]
fn the_city_mood_moves_housing_demand() {
    let mut w = town(120);
    w.happiness.average = NEUTRAL_MOOD;
    let neutral = w.demand().residential;
    w.happiness.average = 90;
    let happy = w.demand().residential;
    w.happiness.average = 10;
    let unhappy = w.demand().residential;
    assert!(
        happy > neutral && neutral > unhappy,
        "{happy} {neutral} {unhappy}"
    );
}

#[test]
fn the_update_averages_moods_and_paints_cells_where_people_live() {
    let mut w = town(120);
    assert!(
        w.happiness.version > 0,
        "cells were painted as the town grew"
    );
    w.update_happiness();
    let moods: Vec<u32> = residents(&w)
        .map(|c| u32::from(w.citizen_mood(c).0))
        .collect();
    let mean = moods.iter().sum::<u32>() / moods.len() as u32;
    assert_eq!(u32::from(w.happiness.average), mean);
    let unemployed = residents(&w)
        .filter(|&c| !w.citizens.child[c as usize] && w.citizens.job[c as usize].is_none())
        .count() as u32;
    assert_eq!(
        w.happiness.complaints[Complaint::NoJob as usize],
        unemployed
    );
    let cells = w.happiness.cells();
    assert_eq!(cells.len(), w.pollution.len());
    let home = w.home_building(residents(&w).next().unwrap());
    let at = w.buildings.footprint(home)[0];
    let cell = w.pollution.cell_at(at.0, at.1).unwrap();
    assert!(cells[cell] > 0, "people live here");
    let far = w.pollution.cell_at(330.0, 230.0).unwrap();
    assert_eq!(cells[far], 0, "nobody lives by the far factories");
}

#[test]
fn miserable_households_leave_after_a_day_unless_they_cheer_up() {
    let mut w = town(120);
    // A household of adults only, so making each of them miserable sets its mean.
    let hh = (0..w.households.active.len())
        .find(|&h| {
            w.households.active[h]
                && w.households.members[h]
                    .iter()
                    .all(|&c| !w.citizens.child[c as usize])
        })
        .unwrap();
    let members = w.households.members[hh].clone();
    let miserable = |w: &mut World| {
        for &c in &members {
            w.release_job(c);
            w.citizens.commute[c as usize] = 200;
        }
    };
    miserable(&mut w);
    assert!(u32::from(w.citizen_mood(members[0]).0) < LEAVE_BELOW);
    let grace = w.days_to_ticks(1.0);
    w.update_happiness();
    let since = w.happiness.low_since[hh];
    assert_eq!(since, w.totals.tick);
    // Within the day nobody leaves.
    for _ in 0..1000 {
        w.update_happiness();
    }
    assert!(w.households.active[hh]);
    // Cheering up resets the wait.
    for &c in &members {
        w.citizens.commute[c as usize] = 0;
        let job = (0..w.buildings.active.len() as u32)
            .find(|&b| w.buildings.zone[b as usize] == Zone::IndustrialLow)
            .unwrap();
        w.citizens.job[c as usize] = Some(job);
        w.residents.workers[job as usize] += 1;
        w.residents.employed += 1;
    }
    assert!(i32::from(w.citizen_mood(members[0]).0) >= BASE_MOOD - 20);
    w.update_happiness();
    assert_eq!(w.happiness.low_since[hh], u64::MAX);
    // Miserable again for over a day: they leave, and their home is vacant.
    miserable(&mut w);
    w.update_happiness();
    w.totals.tick += grace;
    let population = w.residents.population;
    for _ in 0..5000 {
        if !w.households.active[hh] {
            break;
        }
        w.update_happiness();
    }
    assert!(!w.households.active[hh], "the household left the city");
    assert_eq!(w.residents.population, population - members.len() as u32);
    assert!(members.iter().all(|&c| !w.citizens.active[c as usize]));
}

#[test]
fn inspection_reports_mood_complaints_and_commute() {
    let mut w = town(120);
    let c = residents(&w)
        .find(|&c| !w.citizens.child[c as usize])
        .unwrap();
    w.release_job(c);
    let info = w.citizen_info(c).unwrap();
    let (mood, why) = w.citizen_mood(c);
    assert_eq!(info.mood, mood);
    assert_eq!(info.complaints, why.list());
    assert!(info.complaints.contains(&Complaint::NoJob));
    assert_eq!(info.commute, 0);
}
