//! Industrial lot sizes, seeding and recuts (ADR 0019).

use sim::road::RoadClass;
use sim::{Command, Zone};

use super::{check_lots, empty, fill, rect, road, unlotted};

#[test]
fn industrial_blocks_mix_small_medium_and_factory_sized_lots() {
    let mut w = empty();
    rect(&mut w, 0.0, 0.0, 240.0, 120.0);
    fill(&mut w, 40.0, 40.0, Zone::IndustrialHigh);
    check_lots(&w);
    let widths: Vec<_> = w
        .lots
        .active_lots()
        .map(|l| w.lots.frontage[l as usize])
        .collect();
    assert!(widths.iter().any(|&v| v < 24.0), "keep small workshops");
    assert!(
        widths.iter().any(|&v| (24.0..48.0).contains(&v)),
        "add warehouses"
    );
    assert!(widths.iter().any(|&v| v >= 48.0), "add factory-sized lots");
    assert_eq!(unlotted(&w), 0);
}

#[test]
fn industrial_curves_keep_connected_lots_and_stable_ids_after_repainting() {
    let mut w = empty();
    road(&mut w, 0.0, 0.0, 240.0, 0.0);
    road(&mut w, 240.0, 0.0, 240.0, 100.0);
    road(&mut w, 0.0, 100.0, 0.0, 0.0);
    w.apply(Command::BuildCurve {
        ax: 0.0,
        ay: 100.0,
        cx: 120.0,
        cy: 140.0,
        bx: 240.0,
        by: 100.0,
        class: RoadClass::Street,
    });
    fill(&mut w, 50.0, 50.0, Zone::IndustrialLow);
    let ids = check_lots(&w);
    assert!(ids.iter().any(|&l| w.lots.frontage[l as usize] > 24.0));
    fill(&mut w, 50.0, 50.0, Zone::ResidentialHigh);
    road(&mut w, 0.0, 0.0, -80.0, 0.0);
    assert_eq!(
        check_lots(&w),
        ids,
        "an unchanged curved factory site keeps its identity"
    );
}

#[test]
fn road_edits_give_replacement_lots_the_current_zones_sizes() {
    for zone in [Zone::IndustrialHigh, Zone::ResidentialLow] {
        let mut w = empty();
        rect(&mut w, 0.0, 0.0, 240.0, 120.0);
        fill(&mut w, 40.0, 40.0, Zone::IndustrialHigh);
        fill(&mut w, 40.0, 40.0, zone);
        let old_slots = w.lots.len();
        // This joins and splits a frontage road from outside the block.
        road(&mut w, 120.0, 0.0, 120.0, -80.0);
        check_lots(&w);
        let replacements: Vec<_> = w
            .lots
            .active_lots()
            .filter(|&l| l as usize >= old_slots)
            .map(|l| w.lots.frontage[l as usize])
            .collect();
        assert!(!replacements.is_empty());
        if zone == Zone::ResidentialLow {
            assert!(
                replacements.iter().all(|&v| v < 24.0),
                "retired factory plots must not become oversized house plots: {replacements:?}"
            );
        } else {
            assert!(
                replacements.iter().filter(|&&v| v >= 24.0).count() >= 2,
                "replacement industry still needs a mix of site sizes: {replacements:?}"
            );
        }
    }
}

#[test]
fn industrial_lot_layout_is_seeded_but_residential_layout_is_unchanged() {
    let layout = |seed, zone| {
        let mut w = empty();
        w.cfg.seed = seed;
        rect(&mut w, 0.0, 0.0, 240.0, 120.0);
        fill(&mut w, 40.0, 40.0, zone);
        let mut out = Vec::new();
        w.lots.write_outlines(&mut out);
        out
    };
    assert_eq!(
        layout(1, Zone::IndustrialHigh),
        layout(1, Zone::IndustrialHigh)
    );
    assert_ne!(
        layout(1, Zone::IndustrialHigh),
        layout(2, Zone::IndustrialHigh)
    );
    assert_eq!(
        layout(1, Zone::ResidentialLow),
        layout(2, Zone::ResidentialLow)
    );
}

#[test]
fn industrial_lot_sizes_do_not_swallow_other_zones_or_change_on_repaint() {
    let mut w = empty();
    rect(&mut w, 0.0, 0.0, 240.0, 120.0);
    // Paint both uses before cutting to exercise a mixed block.
    w.zones.paint(0.0, 0.0, 120.0, 120.0, Zone::ResidentialLow);
    w.apply(Command::PaintZone {
        ax: 120.0,
        ay: 0.0,
        bx: 240.0,
        by: 120.0,
        zone: Zone::IndustrialHigh,
    });
    let ids = check_lots(&w);
    for &l in &ids {
        if w.lots.frontage[l as usize] > 24.0 {
            assert!(
                w.lots
                    .cells(l)
                    .iter()
                    .all(|&c| w.zones.zone(c) == Zone::IndustrialHigh)
            );
        }
    }
    let mut before = Vec::new();
    w.lots.write_outlines(&mut before);
    fill(&mut w, 40.0, 40.0, Zone::CommercialHigh);
    let mut after = Vec::new();
    w.lots.write_outlines(&mut after);
    assert_eq!(check_lots(&w), ids);
    assert_eq!(
        after, before,
        "repainting must not repartition existing lots"
    );
}
