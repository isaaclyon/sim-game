//! Residential frontage variety without repartitioning existing neighbourhoods.
use sim::Zone;

use super::{check_lots, empty, fill, rect, road, unlotted};

#[test]
fn residential_blocks_mix_standard_and_wide_lots() {
    for zone in [Zone::ResidentialLow, Zone::ResidentialHigh] {
        let mut w = empty();
        rect(&mut w, 0.0, 0.0, 240.0, 120.0);
        fill(&mut w, 40.0, 40.0, zone);
        let ids = check_lots(&w);
        let widths: Vec<_> = ids.iter().map(|&l| w.lots.frontage[l as usize]).collect();
        assert!(widths.iter().any(|&v| v < 24.0), "keep standard lots");
        assert!(widths.iter().any(|&v| v >= 24.0), "include wider lots");
        assert!(widths.iter().all(|&v| v < 40.0), "no factory-sized homes");
        assert_eq!(unlotted(&w), 0);
        let mut before = Vec::new();
        w.lots.write_outlines(&mut before);
        fill(&mut w, 40.0, 40.0, Zone::IndustrialHigh);
        road(&mut w, 0.0, 0.0, -80.0, 0.0);
        let mut after = Vec::new();
        w.lots.write_outlines(&mut after);
        assert_eq!(check_lots(&w), ids);
        assert_eq!(before, after, "repaint and unchanged recuts preserve lots");
    }
}

#[test]
fn residential_widths_are_repeatable_and_seeded() {
    let layout = |seed| {
        let mut w = empty();
        w.cfg.seed = seed;
        rect(&mut w, 0.0, 0.0, 240.0, 120.0);
        fill(&mut w, 40.0, 40.0, Zone::ResidentialLow);
        let mut out = Vec::new();
        w.lots.write_outlines(&mut out);
        out
    };
    assert_eq!(layout(1), layout(1));
    assert_ne!(layout(1), layout(2));
}
