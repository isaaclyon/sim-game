//! Schools (ADR 0055): placing, seats, enrolment, graduating, teacher jobs and land value.
use super::*;
use crate::citizen::Education;
use crate::world::residents::family::CHILDHOOD_DAYS;
use crate::{Command, Config};

mod lifts;
mod view;

/// Homes, shops and industry along the top row of blocks; the bottom row is empty.
fn town() -> World {
    let mut w = World::new(
        Config {
            ignore_demand: true,
            ..Config::default()
        },
        crate::road::RoadNetwork::generate_lattice(5, 3, 120.0),
        crate::rng::Rng::new(1),
    );
    for (x, zone) in [
        (50.0, Zone::ResidentialLow),
        (170.0, Zone::CommercialLow),
        (290.0, Zone::IndustrialLow),
        (410.0, Zone::ResidentialHigh),
    ] {
        assert!(w.apply(Command::FillBlock { x, y: 50.0, zone }));
    }
    advance(&mut w, 1200);
    w
}

fn advance(w: &mut World, ticks: u32) {
    for _ in 0..ticks {
        w.tick();
    }
}

fn place(w: &mut World, kind: SchoolKind, x: f32, y: f32) -> bool {
    w.apply(Command::PlaceSchool { kind, x, y })
}

fn children(w: &World) -> Vec<u32> {
    (0..w.citizens.active.len() as u32)
        .filter(|&c| w.citizens.active[c as usize] && w.citizens.child[c as usize])
        .collect()
}

/// Every child may walk any distance, so reach never decides a test about something else.
fn tireless(w: &mut World) {
    w.citizens.walk_limit.fill(1e6);
}

fn seated(w: &World, kind: SchoolKind) -> Vec<u32> {
    children(w)
        .into_iter()
        .filter(|&c| {
            w.schools
                .school_of(c)
                .is_some_and(|s| w.schools.kind[s as usize] == kind)
        })
        .collect()
}

#[test]
fn a_school_is_placed_beside_a_road_for_a_price_and_costs_upkeep() {
    let mut w = town();
    let (hash, upkeep, funds) = (w.state_hash(), w.money().upkeep, w.treasury.balance);
    // Far from every road.
    assert!(!place(&mut w, SchoolKind::Primary, -400.0, -400.0));
    assert_eq!(w.state_hash(), hash);

    assert!(place(&mut w, SchoolKind::Primary, 60.0, 135.0));
    let kind = SchoolKind::Primary;
    assert_eq!(w.treasury.balance, funds - kind.cost());
    assert_eq!(
        w.money().upkeep - upkeep,
        i64::from(START_BLOCKS) * kind.block_upkeep()
    );
    let b = w.schools.building[0];
    assert!(w.buildings.active[b as usize]);
    assert_eq!(w.buildings.zone[b as usize], Zone::PrimarySchool);
    assert_eq!(w.buildings.lot[b as usize], crate::building::NO_LOT);
    assert_eq!(
        w.school_stats(kind).seats,
        START_BLOCKS * kind.block_seats()
    );

    // Not on top of another school, and not without the money.
    assert!(!place(&mut w, SchoolKind::High, 62.0, 136.0));
    w.treasury.balance = SchoolKind::High.cost() - 1;
    assert!(!place(&mut w, SchoolKind::High, 300.0, 135.0));
    assert_eq!(w.schools.active_schools().count(), 1);

    let mut preview = Vec::new();
    w.write_school_preview(SchoolKind::High, 300.0, 135.0, &mut preview);
    assert_eq!(preview[..2], [0.0, 0.0], "fits, but unaffordable");
    assert_eq!(preview.len(), 2 + 5 * 2);
    w.treasury.balance = SchoolKind::High.cost();
    w.write_school_preview(SchoolKind::High, 300.0, 135.0, &mut preview);
    assert_eq!(preview[..2], [1.0, 1.0]);
    w.write_school_preview(SchoolKind::High, -400.0, -400.0, &mut preview);
    assert!(preview.is_empty());
}

#[test]
fn bulldozing_a_school_frees_its_pupils_and_undo_puts_it_back() {
    let mut w = town();
    tireless(&mut w);
    let funds = w.treasury.balance;
    assert!(place(&mut w, SchoolKind::Primary, 60.0, 135.0));
    w.tick();
    let pupils = w.schools.pupils[0];
    assert!(pupils > 0, "children enrol at once");
    let at = w.schools.footprint(0).centre;
    let building = w.schools.building[0];

    let mut outline = Vec::new();
    w.write_bulldoze_preview(at.0, at.1, &mut outline);
    assert_eq!(outline.len(), 10, "the school's closed outline");
    assert!(w.apply(Command::DeleteRoad { x: at.0, y: at.1 }));
    assert!(!w.schools.active[0] && !w.buildings.active[building as usize]);
    assert!(seated(&w, SchoolKind::Primary).is_empty());
    assert_eq!(w.school_upkeep(), 0);

    assert!(w.apply(Command::Undo));
    assert!(w.schools.active[1] && w.schools.footprint(1).centre == at);
    w.tick();
    assert_eq!(w.schools.pupils[1], pupils);
    // Undoing the placement itself takes the school away and refunds it.
    assert!(w.apply(Command::Undo));
    assert!(!w.apply(Command::Undo), "nothing is left to undo");
    assert_eq!(w.schools.active_schools().count(), 0);
    assert!(w.treasury.balance >= funds, "refunded in full");
    assert!(w.apply(Command::Redo));
    assert_eq!(w.schools.active_schools().count(), 1);
    assert!(World::load(&w.save().unwrap()).is_ok());
}

#[test]
fn children_enrol_by_stage_at_the_nearest_school_with_a_free_seat() {
    let mut w = town();
    tireless(&mut w);
    assert!(place(&mut w, SchoolKind::Primary, 60.0, 135.0));
    assert!(place(&mut w, SchoolKind::Primary, 420.0, 135.0));
    assert!(place(&mut w, SchoolKind::High, 200.0, 135.0));
    w.tick();
    let kids = children(&w);
    let full = |s: u32| w.schools.pupils[s as usize] == w.schools.seats(s);
    assert!(kids.len() > 4);
    for &c in &kids {
        let Some(s) = w.schools.school_of(c) else {
            // Only a full city of schools leaves a child waiting.
            assert!(
                w.schools
                    .active_schools()
                    .all(|s| { w.schools.kind[s as usize] != w.school_stage(c) || full(s) })
            );
            continue;
        };
        assert_eq!(w.schools.kind[s as usize], w.school_stage(c));
        if w.school_stage(c) == SchoolKind::Primary {
            let node = w.building_node(w.home_building(c)).unwrap();
            let d = |s| w.school_distance(s, node).unwrap();
            let other = 1 - s;
            let nearest = d(s) < d(other) || (d(s) == d(other) && s < other);
            assert!(nearest || full(other), "child {c} at school {s}");
        }
    }
    let taken: u32 = w.schools.pupils.iter().sum();
    let holders = kids.iter().filter(|&&c| w.schools.school_of(c).is_some());
    assert_eq!(taken as usize, holders.count());
    for s in w.schools.active_schools() {
        assert!(w.schools.pupils[s as usize] <= w.schools.seats(s));
    }
    assert!(w.schools.pupils[0] > 0 && w.schools.pupils[1] > 0);

    // A full school turns children away: they take the farther one, or wait.
    let c = seated(&w, SchoolKind::Primary)[0];
    let near = w.schools.school_of(c).unwrap();
    let far = seated(&w, SchoolKind::Primary)
        .into_iter()
        .find(|&o| w.schools.school_of(o) == Some(1 - near))
        .expect("a pupil at the other school");
    w.leave_school(far);
    w.leave_school(c);
    let spare = w.schools.seats(near) - w.schools.pupils[near as usize];
    w.schools.pupils[near as usize] += spare;
    assert!(w.try_enrol(c));
    assert_eq!(w.schools.school_of(c), Some(1 - near));
    w.schools.pupils[near as usize] -= spare;
    // With every seat taken, a child waits.
    let seats = (w.schools.seats(0), w.schools.seats(1));
    (w.schools.pupils[0], w.schools.pupils[1]) = seats;
    assert!(!w.try_enrol(far));
}

#[test]
fn a_child_out_of_walking_reach_or_outside_the_area_waits() {
    let mut w = town();
    assert!(place(&mut w, SchoolKind::Primary, 60.0, 135.0));
    w.citizens.walk_limit.fill(1.0);
    w.citizens.owns_car.fill(false);
    w.tick();
    assert!(seated(&w, SchoolKind::Primary).is_empty());
    // Seats or roads changing makes waiting children look again.
    tireless(&mut w);
    assert!(place(&mut w, SchoolKind::High, 200.0, 135.0));
    w.tick();
    assert!(!seated(&w, SchoolKind::Primary).is_empty());

    // A school whose road is cut off from the homes covers nobody.
    let c = seated(&w, SchoolKind::Primary)[0];
    let node = w.building_node(w.home_building(c)).unwrap();
    assert!(w.school_distance(0, node).unwrap() <= SchoolKind::Primary.cover_m());
    assert_eq!(w.school_distance(0, u32::MAX), None);
}

#[test]
fn a_child_graduates_only_after_half_a_stage_in_a_seat() {
    let mut w = town();
    tireless(&mut w);
    assert!(place(&mut w, SchoolKind::Primary, 60.0, 135.0));
    assert!(place(&mut w, SchoolKind::High, 200.0, 135.0));
    w.tick();
    let stage = w.days_to_ticks(CHILDHOOD_DAYS) / 2;
    let high = seated(&w, SchoolKind::High);
    let primary = seated(&w, SchoolKind::Primary);
    assert!(high.len() >= 2 && !primary.is_empty());
    let now = w.totals.tick;

    // Seated for the whole stage: graduates.
    let (a, b) = (high[0], high[1]);
    w.schools.schooled[a as usize] = stage;
    w.finish_school(a);
    assert_eq!(w.citizens.education[a as usize], Education::HighSchool);
    assert_eq!(w.schools.school_of(a), None);
    // Seated only just before the stage ended: does not.
    w.schools.schooled[b as usize] = 0;
    w.schools.since[b as usize] = now;
    w.finish_school(b);
    assert_eq!(w.citizens.education[b as usize], Education::None);
    // Schooled long enough but with no seat at the end: does not.
    let c = primary[0];
    w.schools.schooled[c as usize] = stage;
    w.leave_school(c);
    w.finish_school(c);
    assert_eq!(w.citizens.education[c as usize], Education::None);
    assert_eq!(
        w.schools.pupils.iter().sum::<u32>() as usize,
        high.len() + primary.len() - 3
    );
    assert!(World::load(&w.save().unwrap()).is_ok());
}

#[test]
fn a_primary_pupil_moves_up_to_high_school_at_the_stage_change() {
    let mut w = town();
    tireless(&mut w);
    assert!(place(&mut w, SchoolKind::Primary, 60.0, 135.0));
    assert!(place(&mut w, SchoolKind::High, 200.0, 135.0));
    w.tick();
    let c = seated(&w, SchoolKind::Primary)[0];
    let stage = w.days_to_ticks(CHILDHOOD_DAYS) / 2;
    // The high school is full: one pupil leaves town to make room.
    w.leave_school(seated(&w, SchoolKind::High)[0]);
    w.schools.pending = false;
    // Make the child one tick short of high school age, schooled all along.
    w.citizens.born[c as usize] = w.totals.tick as i64 - stage as i64 + 1;
    w.schools.since[c as usize] = 0;
    w.schools.schooled[c as usize] = stage;
    w.schools.stages.push(Reverse((w.totals.tick + 1, c)));
    advance(&mut w, 2);
    assert_eq!(w.citizens.education[c as usize], Education::Primary);
    let s = w.schools.school_of(c).expect("a high school seat");
    assert_eq!(w.schools.kind[s as usize], SchoolKind::High);
}

#[test]
fn a_school_offers_teacher_jobs_that_residents_take() {
    let mut w = town();
    assert!(place(&mut w, SchoolKind::High, 200.0, 135.0));
    let b = w.schools.building[0];
    advance(&mut w, 3000);
    let teachers = START_BLOCKS * SchoolKind::High.block_teachers();
    assert_eq!(w.residents.jobs[b as usize], teachers);
    let staff = w.residents.workers[b as usize];
    assert!(staff > 0 && staff <= teachers, "{staff} of {teachers}");
    assert_eq!(
        crate::world::residents::education::job_education(Zone::HighSchool),
        Education::HighSchool
    );
    // Schools need no power or water and never close.
    advance(&mut w, 3000);
    assert!(w.buildings.active[b as usize]);
    assert!(World::load(&w.save().unwrap()).is_ok());
}

#[test]
fn homes_in_a_schools_area_gain_land_value() {
    let value = |w: &World| {
        let mut total = 0.0f32;
        for b in w.buildings.active_buildings() {
            if w.buildings.zone[b as usize] == Zone::ResidentialLow {
                let p = w.building_front(b);
                total += w.pollution_at(p.0, p.1 - 10.0).unwrap().land_value;
            }
        }
        total
    };
    let mut plain = town();
    let mut schooled = town();
    assert!(place(&mut schooled, SchoolKind::Primary, 60.0, 135.0));
    advance(&mut plain, 1200);
    advance(&mut schooled, 1200);
    assert!(value(&schooled) > value(&plain));
}

#[test]
fn schools_survive_a_save_and_a_city_without_one_hashes_as_before() {
    let mut w = town();
    let untouched = w.state_hash();
    tireless(&mut w);
    assert!(place(&mut w, SchoolKind::Primary, 60.0, 135.0));
    assert_ne!(w.state_hash(), untouched);
    advance(&mut w, 50);
    let mut loaded = World::load(&w.save().unwrap()).unwrap();
    assert_eq!(loaded.state_hash(), w.state_hash());
    assert_eq!(loaded.schools.pupils, w.schools.pupils);
    advance(&mut w, 600);
    advance(&mut loaded, 600);
    assert_eq!(loaded.state_hash(), w.state_hash());

    let mut out = Vec::new();
    w.write_schools(&mut out);
    assert_eq!(out.len(), SCHOOL_STRIDE);
    assert_eq!((out[0], out[3]), (0.0, SchoolKind::Primary.half_size()));
}

/// Runs one clock day, counting ticks on which each watched thing was seen.
struct Day {
    at_school: bool,
    child_walkers: bool,
    left_before_seven: bool,
}

fn watch_a_day(w: &mut World) -> Day {
    use crate::citizen::Activity;
    use crate::clock::clock_at;
    let mut day = Day {
        at_school: false,
        child_walkers: false,
        left_before_seven: false,
    };
    let mut agents = Vec::new();
    for tick in 0..w.days_to_ticks(1.0) {
        w.tick();
        let kids = children(w);
        let out = |c: &u32| w.citizens.location[*c as usize] != w.home_building(*c);
        let walking = |c: &u32| w.citizens.destination[*c as usize].is_some();
        day.at_school |= kids.iter().any(|&c| {
            w.citizens.activity[c as usize] == Activity::School
                && Some(w.citizens.location[c as usize]) == w.school_building(c)
        });
        if clock_at(w.totals.tick, w.cfg.dt).second_of_day < 7 * 3600 {
            day.left_before_seven |= kids.iter().any(|c| out(c) || walking(c));
        }
        if tick % 50 == 0 {
            w.write_agent_snapshot(&mut agents);
            day.child_walkers |= agents
                .chunks(crate::world::SNAPSHOT_STRIDE)
                .any(|a| a[3] >= 0.0 && a[4] == 3.0);
        }
    }
    day
}

#[test]
fn enrolled_children_walk_to_school_in_the_morning_and_home_again() {
    let mut w = town();
    tireless(&mut w);
    assert!(place(&mut w, SchoolKind::Primary, 60.0, 135.0));
    assert!(place(&mut w, SchoolKind::High, 180.0, 135.0));
    w.tick();
    assert!(
        !seated(&w, SchoolKind::Primary).is_empty() || !seated(&w, SchoolKind::High).is_empty()
    );
    let walks = w.totals.walks_completed;

    let day = watch_a_day(&mut w);
    assert!(day.at_school, "children spend the day at their school");
    assert!(
        day.child_walkers,
        "children on the way are drawn as child walkers"
    );
    assert!(!day.left_before_seven, "nobody sets out before the morning");
    assert!(w.totals.walks_completed > walks);
    // A second day ends with every child back home or on the way.
    watch_a_day(&mut w);
    let now = crate::clock::clock_at(w.totals.tick, w.cfg.dt).second_of_day;
    for c in children(&w) {
        let i = c as usize;
        let home = w.home_building(c);
        let settled = w.citizens.location[i] == home || w.citizens.destination[i] == Some(home);
        // Mid school day, a seated child is at school instead.
        let in_class = (9 * 3600..14 * 3600).contains(&now) && w.school_building(c).is_some();
        assert!(
            settled || in_class,
            "child {c} is neither home nor in class"
        );
    }
}

#[test]
fn a_child_without_a_seat_stays_home() {
    let mut w = town();
    // Only a high school, and no child old enough for it: nobody holds a seat.
    assert!(place(&mut w, SchoolKind::High, 180.0, 135.0));
    for c in children(&w) {
        w.citizens.born[c as usize] = w.totals.tick as i64;
    }
    let day = watch_a_day(&mut w);
    assert!(seated(&w, SchoolKind::High).is_empty());
    assert!(!day.at_school && !day.child_walkers);
    for c in children(&w) {
        assert_eq!(w.citizens.location[c as usize], w.home_building(c));
    }
}

#[test]
fn bulldozing_a_school_in_session_sends_its_pupils_home() {
    use crate::citizen::Activity;
    let mut w = town();
    tireless(&mut w);
    assert!(place(&mut w, SchoolKind::Primary, 60.0, 135.0));
    assert!(place(&mut w, SchoolKind::High, 180.0, 135.0));
    let in_class = |w: &World| {
        children(w)
            .into_iter()
            .filter(|&c| Some(w.citizens.location[c as usize]) == w.school_building(c))
            .count()
    };
    for _ in 0..w.days_to_ticks(2.0) {
        w.tick();
        if in_class(&w) > 0 {
            break;
        }
    }
    assert!(in_class(&w) > 0);
    for s in 0..2 {
        let at = w.schools.footprint(s).centre;
        assert!(w.apply(Command::DeleteRoad { x: at.0, y: at.1 }));
    }
    for c in children(&w) {
        let i = c as usize;
        assert!(w.buildings.active[w.citizens.location[i] as usize]);
        assert_eq!(w.citizens.activity[i], Activity::Home);
        assert!(w.citizens.destination[i].is_none_or(|b| b == w.home_building(c)));
    }
    watch_a_day(&mut w);
    for c in children(&w) {
        assert_eq!(w.citizens.location[c as usize], w.home_building(c));
    }
}
