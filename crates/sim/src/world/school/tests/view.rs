//! The schools view, school inspection and education figures (ADR 0055, slice 4).
use super::*;
use crate::world::{AREA_HIGH, AREA_PRIMARY, SCHOOL_ROAD_STRIDE, SCHOOL_VIEW_STRIDE};

#[test]
fn the_schools_view_lists_schools_covered_roads_and_homes() {
    let mut w = town();
    let (mut schools, mut roads, mut homes) = (Vec::new(), Vec::new(), Vec::new());
    w.write_school_view(&mut schools, &mut roads, &mut homes);
    assert!(schools.is_empty() && roads.is_empty() && homes.is_empty());

    tireless(&mut w);
    assert!(place(&mut w, SchoolKind::Primary, 60.0, 135.0));
    w.tick();
    w.write_school_view(&mut schools, &mut roads, &mut homes);
    assert_eq!(schools.len(), SCHOOL_VIEW_STRIDE);
    let pupils = seated(&w, SchoolKind::Primary).len();
    assert!(pupils > 0);
    assert_eq!(
        schools[5..],
        [
            0.0,
            w.schools.seats(0) as f32,
            pupils as f32,
            w.schools.building[0] as f32
        ]
    );
    assert!(!roads.is_empty() && roads.len() % SCHOOL_ROAD_STRIDE == 0);
    assert!(
        roads
            .chunks(SCHOOL_ROAD_STRIDE)
            .all(|r| r[4] == f32::from(AREA_PRIMARY))
    );
    // Every home is listed with its lot; those inside the area say so.
    let mut covered = 0;
    let mut at = 0;
    while at < homes.len() {
        covered += usize::from(homes[at] == f32::from(AREA_PRIMARY));
        at += 2 + 2 * homes[at + 1] as usize;
    }
    assert_eq!(at, homes.len());
    assert!(covered > 0);

    assert!(place(&mut w, SchoolKind::High, 180.0, 135.0));
    w.tick();
    w.write_school_view(&mut schools, &mut roads, &mut homes);
    assert_eq!(schools.len(), 2 * SCHOOL_VIEW_STRIDE);
    let both = f32::from(AREA_PRIMARY | AREA_HIGH);
    assert!(roads.chunks(SCHOOL_ROAD_STRIDE).any(|r| r[4] == both));
}

#[test]
fn a_school_and_its_pupils_can_be_inspected() {
    let mut w = town();
    tireless(&mut w);
    assert!(place(&mut w, SchoolKind::Primary, 60.0, 135.0));
    w.tick();
    let b = w.schools.building[0];
    let info = w.school_info(b).unwrap();
    assert_eq!(info.kind, SchoolKind::Primary);
    assert_eq!(info.seats, w.schools.seats(0));
    assert_eq!(info.pupils as usize, seated(&w, SchoolKind::Primary).len());
    assert_eq!(w.school_info(0).filter(|_| b != 0), None);

    let c = seated(&w, SchoolKind::Primary)[0];
    let pupil = w.citizen_info(c).unwrap();
    assert_eq!(pupil.school.map(|p| p.building), Some(b));
    assert_eq!(pupil.education, Education::None);
    assert!(!pupil.riding);
}

#[test]
fn education_figures_count_adults_by_level_and_children_without_a_seat() {
    let mut w = town();
    let stats = w.stats();
    let kids = children(&w).len() as u32;
    assert_eq!(stats.education.unschooled, kids);
    assert_eq!(
        stats.education.adults.iter().sum::<u32>(),
        stats.population - kids
    );
    let adult = (0..w.citizens.active.len())
        .find(|&c| w.citizens.active[c] && !w.citizens.child[c])
        .unwrap();
    w.citizens.education[adult] = Education::None;
    let before = w.stats().education;
    w.citizens.education[adult] = Education::HighSchool;
    tireless(&mut w);
    assert!(place(&mut w, SchoolKind::Primary, 60.0, 135.0));
    assert!(place(&mut w, SchoolKind::High, 180.0, 135.0));
    w.tick();
    let after = w.stats().education;
    assert_eq!(after.adults[0], before.adults[0] - 1);
    assert_eq!(after.adults[2], before.adults[2] + 1);
    assert_eq!(after.unschooled, kids - seats_all(&w));
}

fn seats_all(w: &World) -> u32 {
    (seated(w, SchoolKind::Primary).len() + seated(w, SchoolKind::High).len()) as u32
}
