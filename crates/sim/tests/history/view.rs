//! What the undo and redo buttons show and preview; read-only.
use super::*;
use sim::power::PlantKind;
use sim::world::StepKind;

/// `[n, x0, y0, …]` records, as lists of points.
fn shapes(flat: &[f32]) -> Vec<Vec<(f32, f32)>> {
    let mut out = Vec::new();
    let mut i = 0;
    while i < flat.len() {
        let n = flat[i] as usize;
        out.push(
            flat[i + 1..i + 1 + 2 * n]
                .chunks(2)
                .map(|p| (p[0], p[1]))
                .collect(),
        );
        i += 1 + 2 * n;
    }
    out
}

#[test]
fn the_view_names_each_step_and_its_money() {
    let mut w = empty();
    let v = w.history_view();
    assert_eq!(
        (v.undo, v.redo, v.undo_refund, v.redo_cost),
        (None, None, None, None)
    );
    assert!(!v.redo_affordable);
    let start = balance(&w);
    assert!(street(&mut w, (0.0, 0.0), (200.0, 0.0)));
    let cost = start - balance(&w);
    let v = w.history_view();
    assert_eq!(
        (v.undo, v.undo_refund, v.redo),
        (Some(StepKind::Road), Some(cost), None)
    );
    assert!(undo(&mut w));
    let v = w.history_view();
    assert_eq!(
        (v.undo, v.redo, v.redo_cost),
        (None, Some(StepKind::Road), Some(cost))
    );
    assert!(v.redo_affordable);
}

#[test]
fn previews_show_what_would_go_and_come_back_and_change_nothing() {
    let mut w = empty();
    assert!(street(&mut w, (0.0, 0.0), (200.0, 0.0)));
    assert!(w.apply(Command::PlacePlant {
        kind: PlantKind::Solar,
        x: 100.0,
        y: 80.0,
    }));
    let (mut remove, mut restore) = (Vec::new(), Vec::new());
    let hash = w.state_hash();
    assert!(w.write_history_preview(false, &mut remove, &mut restore));
    let outline = &shapes(&remove)[0];
    assert_eq!(outline.len(), 5, "a closed square");
    assert_eq!(outline[0], outline[4]);
    assert!(restore.is_empty());
    assert!(
        !w.write_history_preview(true, &mut remove, &mut restore),
        "nothing to redo"
    );
    assert!(remove.is_empty() && restore.is_empty());
    assert_eq!(w.state_hash(), hash);

    assert!(undo(&mut w) && undo(&mut w));
    assert!(w.write_history_preview(true, &mut remove, &mut restore));
    assert!(remove.is_empty());
    assert_eq!(shapes(&restore), [vec![(0.0, 0.0), (200.0, 0.0)]]);
}
