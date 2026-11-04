//! Spreading point emissions over the grid.

/// Spreads `src` (row-major, `cols` × `rows`) with a separable tent of `radius` cells: a cell
/// `d` cells away along each axis gets `(1 - d / (radius + 1))` of the emission per axis. The
/// peak weight is 1, so one source reads as its own emission in its own cell.
pub(super) fn spread(src: &[f32], cols: usize, rows: usize, radius: usize) -> Vec<f32> {
    let weight = |d: usize| 1.0 - d as f32 / (radius + 1) as f32;
    let mut across = vec![0.0; src.len()];
    for r in 0..rows {
        let row = r * cols;
        for c in 0..cols {
            let v = src[row + c];
            if v == 0.0 {
                continue;
            }
            for x in c.saturating_sub(radius)..(c + radius + 1).min(cols) {
                across[row + x] += v * weight(c.abs_diff(x));
            }
        }
    }
    // Row by row, so both rows are read and written in memory order.
    let mut out = vec![0.0; src.len()];
    for r in 0..rows {
        let from = &across[r * cols..(r + 1) * cols];
        if from.iter().all(|&v| v == 0.0) {
            continue;
        }
        for y in r.saturating_sub(radius)..(r + radius + 1).min(rows) {
            let w = weight(r.abs_diff(y));
            for (o, &v) in out[y * cols..(y + 1) * cols].iter_mut().zip(from) {
                *o += v * w;
            }
        }
    }
    out
}
