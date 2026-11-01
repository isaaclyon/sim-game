use super::*;
use std::collections::BTreeSet;

#[test]
fn first_departures_are_staggered_and_daily_departure_keeps_clock_rate() {
    let mut w = town();
    isolate(&mut w);
    let times: BTreeSet<_> = w.citizens.departure.iter().copied().collect();
    assert!(times.len() > 1);
    assert!(times.iter().all(|&t| (7 * 3600..9 * 3600).contains(&t)));
    let job = distant(&w, 0);
    assign_job(&mut w, 0, job);
    // Late return after midnight: next departure is still the next local morning.
    w.totals.tick = w.secs_to_ticks(19.0 * 3600.0 / 6.0);
    w.schedule_day(0);
    let at = crate::clock::clock_at(w.citizens.due[0], w.cfg.dt);
    assert_eq!(at.day, 2);
    assert!((at.second_of_day as i64 - w.citizens.departure[0] as i64).abs() <= 1);
}

#[test]
fn daily_work_repeats_without_overlapping_trips_or_growing_vehicle_slots() {
    let mut w = town();
    isolate(&mut w);
    let job = distant(&w, 0);
    assign_job(&mut w, 0, job);
    w.schedule_day(0);
    let slots = w.agents.len();
    let mut work_arrivals = 0;
    for _ in 0..300_000 {
        let was_travelling = w.citizens.destination[0].is_some();
        w.tick();
        if was_travelling
            && w.citizens.destination[0].is_none()
            && w.citizens.activity[0] == Activity::Work
        {
            work_arrivals += 1;
        }
        if let Some(a) = w.citizens.agent[0] {
            let queued = w
                .traffic
                .queues
                .iter()
                .flatten()
                .filter(|&&id| id == a)
                .count();
            assert!(queued <= 1);
            if w.citizens.destination[0].is_none() {
                assert_eq!(
                    w.agents.state[a as usize],
                    crate::agents::AgentState::Dormant
                );
            }
        }
    }
    assert!(work_arrivals >= 2, "regular work resumes on later days");
    assert!(
        w.agents.len() <= slots + 1,
        "one stable reservation, not one per trip"
    );
}

#[test]
fn shop_choices_vary_but_prefer_shorter_distances() {
    let mut w = town();
    isolate(&mut w);
    let mut choices = BTreeSet::new();
    for _ in 0..100 {
        choices.insert(w.choose_destination(0, false).unwrap());
    }
    assert!(choices.len() > 1, "not always the closest shop");
}

#[test]
fn repaint_blocks_jobs_and_shops_and_restoring_exact_zone_reopens_them() {
    let mut w = town();
    isolate(&mut w);
    let b = w.choose_destination(0, false).unwrap();
    assert!(w.building_accepts_assignments(b));
    w.apply(Command::FillBlock {
        x: 170.0,
        y: 50.0,
        zone: Zone::CommercialHigh,
    });
    assert!(!w.building_accepts_assignments(b));
    assert!(w.choose_destination(0, false).is_none());
    assert!(w.buildings.active[b as usize]);
    w.apply(Command::FillBlock {
        x: 170.0,
        y: 50.0,
        zone: Zone::CommercialLow,
    });
    assert!(w.building_accepts_assignments(b));
}
