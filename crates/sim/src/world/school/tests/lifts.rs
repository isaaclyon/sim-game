//! Lifts (ADR 0055, slice 5): adults with a car drive children whose school is too far to walk.
use super::*;
use crate::citizen::Activity;

/// No child can walk to any school; adults own a car or not.
fn far(w: &mut World, cars: bool) {
    for c in 0..w.citizens.active.len() {
        if w.citizens.child[c] {
            w.citizens.walk_limit[c] = 1.0;
        } else {
            w.citizens.owns_car[c] = cars;
        }
    }
}

fn schools(w: &mut World) {
    assert!(place(w, SchoolKind::Primary, 60.0, 135.0));
    assert!(place(w, SchoolKind::High, 180.0, 135.0));
    w.tick();
}

fn seats_taken(w: &World) -> usize {
    seated(w, SchoolKind::Primary).len() + seated(w, SchoolKind::High).len()
}

#[derive(Default)]
struct Seen {
    to_school: bool,
    in_class: bool,
    home_again: bool,
    runs: bool,
    child_trips: bool,
}

fn watch(w: &mut World, days: f32) -> Seen {
    let mut seen = Seen::default();
    for _ in 0..w.days_to_ticks(days) {
        w.tick();
        for c in children(w) {
            let i = c as usize;
            let riding = w.riding_with(c).is_some();
            let activity = w.citizens.activity[i];
            seen.to_school |= riding && activity == Activity::School;
            seen.home_again |= riding && activity == Activity::Home;
            seen.in_class |= !riding && Some(w.citizens.location[i]) == w.school_building(c);
            seen.child_trips |= w.citizens.destination[i].is_some();
            if let Some(driver) = w.riding_with(c) {
                let d = driver as usize;
                assert_eq!(w.citizens.household[d], w.citizens.household[i]);
                assert!(!w.citizens.child[d] && w.citizens.owns_car[d]);
            }
        }
        seen.runs |= (0..w.citizens.active.len()).any(|a| {
            w.citizens.activity[a] == Activity::SchoolRun
                && w.citizens.destination[a].is_some()
                && w.citizens.car_out[a]
        });
    }
    seen
}

#[test]
fn a_school_past_walking_reach_needs_a_car_at_home() {
    let mut w = town();
    far(&mut w, false);
    schools(&mut w);
    assert_eq!(seats_taken(&w), 0);
    // With a car in the household the same school is open.
    far(&mut w, true);
    w.schools.pending = true;
    w.tick();
    assert!(seats_taken(&w) > 0);
}

#[test]
fn adults_drive_children_to_school_and_bring_them_home() {
    let mut w = town();
    far(&mut w, true);
    schools(&mut w);
    assert!(seats_taken(&w) > 0);
    let seen = watch(&mut w, 2.0);
    assert!(seen.runs, "an adult drives a school run by car");
    assert!(seen.to_school, "children ride to school");
    assert!(seen.in_class, "and spend the day there");
    assert!(seen.home_again, "and ride home");
    // Late on, nobody is left in a car or at the school gate.
    let now = crate::clock::clock_at(w.totals.tick, w.cfg.dt).second_of_day;
    let school_hours = (7 * 3600..20 * 3600).contains(&now);
    for c in children(&w) {
        let i = c as usize;
        let home = w.home_building(c);
        let settled = w.citizens.location[i] == home || w.citizens.destination[i] == Some(home);
        assert!(settled || school_hours, "child {c} is still out at {now}");
    }
}

#[test]
fn a_child_who_can_walk_is_not_driven() {
    let mut w = town();
    tireless(&mut w);
    w.citizens.owns_car.fill(true);
    schools(&mut w);
    let seen = watch(&mut w, 1.0);
    assert!(seen.in_class && seen.child_trips);
    assert!(!seen.to_school && !seen.home_again && !seen.runs);
}

#[test]
fn a_lift_survives_a_save_and_a_move_ends_it() {
    let mut w = town();
    far(&mut w, true);
    schools(&mut w);
    let riders = |w: &World| -> Vec<u32> {
        children(w)
            .into_iter()
            .filter(|&c| w.riding_with(c).is_some())
            .collect()
    };
    for _ in 0..w.days_to_ticks(2.0) {
        w.tick();
        if !riders(&w).is_empty() {
            break;
        }
    }
    let c = riders(&w)[0];
    let mut loaded = World::load(&w.save().unwrap()).unwrap();
    assert_eq!(loaded.state_hash(), w.state_hash());
    advance(&mut w, 600);
    advance(&mut loaded, 600);
    assert_eq!(loaded.state_hash(), w.state_hash());

    // The driver's trip ending early puts the child home.
    let mut w = loaded;
    for _ in 0..w.days_to_ticks(2.0) {
        w.tick();
        if !riders(&w).is_empty() {
            break;
        }
    }
    let c2 = *riders(&w).first().unwrap_or(&c);
    let driver = w.riding_with(c2).unwrap();
    w.cancel_citizen_trip(driver);
    assert_eq!(w.riding_with(c2), None);
    assert_eq!(w.citizens.location[c2 as usize], w.home_building(c2));
    assert_eq!(w.citizens.activity[c2 as usize], Activity::Home);
}
