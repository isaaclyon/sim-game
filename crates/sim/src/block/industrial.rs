//! Combine new industrial strips, or reconstruct an existing lot during a recut (ADR 0019).
use super::Piece;
use crate::lot::{LotId, Lots};
use crate::poly;
use crate::rng::Rng;
use crate::zone::{Zone, ZoneGrid};

#[derive(Clone, Copy, PartialEq, Eq)]
enum Group {
    Existing(LotId),
    Industrial(Zone),
    Separate,
}

fn group(p: &Piece, grid: &ZoneGrid, lots: &Lots) -> Group {
    let Some(&first) = p.cells.first() else {
        return Group::Separate;
    };
    if let Some(owner) = lots.owner(first) {
        return if p.cells.iter().all(|&c| lots.owner(c) == Some(owner)) {
            Group::Existing(owner)
        } else {
            Group::Separate
        };
    }
    let zone = grid.zone(first);
    if matches!(zone, Zone::IndustrialLow | Zone::IndustrialHigh)
        && p.cells
            .iter()
            .all(|&c| lots.owner(c).is_none() && grid.zone(c) == zone)
    {
        Group::Industrial(zone)
    } else {
        Group::Separate
    }
}

pub(super) fn coalesce(pieces: Vec<Piece>, grid: &ZoneGrid, lots: &Lots, seed: u64) -> Vec<Piece> {
    let mut input = pieces.into_iter().peekable();
    let mut out = Vec::new();
    while let Some(mut p) = input.next() {
        let key = group(&p, grid, lots);
        let count = match key {
            Group::Existing(_) => usize::MAX,
            Group::Industrial(_) => {
                // Local to this road side and start, independent of traffic and command order.
                let lane = u64::from(p.road) * 2 + u64::from(!p.left);
                let mut rng = Rng::new(seed ^ (lane << 32) ^ u64::from(p.span.0.to_bits()));
                match rng.below(10) {
                    0..=3 => 1,
                    4..=7 => 2,
                    _ => 4,
                }
            }
            Group::Separate => 1,
        };
        for _ in 1..count {
            let Some(next) = input.peek() else { break };
            if next.road != p.road
                || next.left != p.left
                || (next.span.0 - p.span.1).abs() > 0.01
                || group(next, grid, lots) != key
            {
                break;
            }
            let mut joined = poly::union(
                std::slice::from_ref(&p.ring),
                std::slice::from_ref(&next.ring),
            );
            // Never bridge a gap, cross a road or wrap another lot in a hole.
            if joined.len() != 1 || joined[0].len() != 1 {
                break;
            }
            let next = input.next().unwrap();
            p.ring = joined[0].remove(0);
            p.rect = poly::bounds(&p.ring);
            p.span.1 = next.span.1;
            p.cells.extend(next.cells);
        }
        p.cells.sort_unstable();
        out.push(p);
    }
    out
}
