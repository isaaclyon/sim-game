use super::tests::{advance, town};
use crate::{Command, Zone};

#[test]
fn building_counts_are_residency_and_employment_not_attendance() {
    let mut w = town();
    let mut twin = town();
    for b in w.buildings.active_buildings() {
        let info = w.building_info(b).unwrap();
        assert_eq!(info.building, b);
        assert_eq!(info.zone, w.buildings.zone[b as usize]);
        if matches!(info.zone, Zone::ResidentialLow | Zone::ResidentialHigh) {
            let capacity: u32 = w
                .homes
                .building
                .iter()
                .enumerate()
                .filter(|&(_, &building)| building == b)
                .map(|(h, _)| w.homes.capacity[h] as u32)
                .sum();
            let residents = (0..w.citizens.active.len())
                .filter(|&c| w.citizens.active[c] && w.home_building(c as u32) == b)
                .count() as u32;
            assert!(capacity > 0);
            assert_eq!(info.capacity, capacity);
            assert_eq!(info.occupied, residents);
        } else {
            assert_eq!(info.capacity, w.residents.jobs[b as usize]);
            assert_eq!(
                info.occupied,
                w.citizens.job.iter().filter(|&&job| job == Some(b)).count() as u32
            );
        }
        assert!(!info.closing);
    }
    assert_eq!(w.state_hash(), twin.state_hash(), "query is read only");
    advance(&mut w, 60);
    advance(&mut twin, 60);
    assert_eq!(
        w.state_hash(),
        twin.state_hash(),
        "queries do not affect later ticks"
    );
}

#[test]
fn apartments_sum_all_homes_and_closure_keeps_original_use() {
    let mut w = town();
    w.apply(Command::FillBlock {
        x: 50.0,
        y: 50.0,
        zone: Zone::ResidentialHigh,
    });
    advance(&mut w, 600);
    let b = w
        .buildings
        .active_buildings()
        .find(|&b| w.buildings.zone[b as usize] == Zone::ResidentialHigh)
        .unwrap();
    let before = w.building_info(b).unwrap();
    assert!(
        before.capacity > 7,
        "whole apartment building, not a single home"
    );
    assert!(before.occupied > 0);
    w.apply(Command::FillBlock {
        x: 50.0,
        y: 50.0,
        zone: Zone::IndustrialHigh,
    });
    let closing = w.building_info(b).unwrap();
    assert!(closing.closing);
    assert_eq!(
        (closing.zone, closing.capacity, closing.occupied),
        (before.zone, before.capacity, before.occupied)
    );
    // One household leaves per closure step; a larger occupied apartment building
    // can take longer than a fixed ten minutes to empty.
    let homes = w.homes.building.iter().filter(|&&home| home == b).count();
    for _ in 0..=homes {
        if !w.buildings.active[b as usize] {
            break;
        }
        advance(&mut w, crate::world::closure::STEP_MAX_S as u32);
    }
    assert!(w.building_info(b).is_none());
    assert!(w.building_info(u32::MAX).is_none());
}
