//! Blocks, block fill, lots and alleys (ADR 0016).

mod alleys;
mod fill;
mod industrial;
mod lots;

use std::collections::BTreeSet;

use sim::block::{LOT_REACH, MIN_LOT_AREA, ROAD_HALF_WIDTH};
use sim::lot::LotId;
use sim::rng::Rng;
use sim::road::{RoadClass, RoadNetwork};
use sim::{Command, Config, World, Zone};

fn empty() -> World {
    World::new(Config::default(), RoadNetwork::default(), Rng::new(1))
}

fn road(w: &mut World, ax: f32, ay: f32, bx: f32, by: f32) {
    assert!(w.apply(Command::BuildRoad {
        ax,
        ay,
        bx,
        by,
        class: RoadClass::Street
    }));
}

/// A closed rectangle of four roads.
fn rect(w: &mut World, x0: f32, y0: f32, x1: f32, y1: f32) {
    road(w, x0, y0, x1, y0);
    road(w, x1, y0, x1, y1);
    road(w, x1, y1, x0, y1);
    road(w, x0, y1, x0, y0);
}

fn fill(w: &mut World, x: f32, y: f32, zone: Zone) -> bool {
    w.apply(Command::FillBlock { x, y, zone })
}

/// Zoned cells clear of every road that no lot owns.
fn unlotted(w: &World) -> usize {
    (0..w.zones.cells.len() as u32)
        .filter(|&i| w.zones.cells[i as usize] != 0 && w.lots.owner(i).is_none())
        .filter(|&i| {
            let (x, y) = w.zones.centre(i);
            road_dist(w, x, y) >= ROAD_HALF_WIDTH
        })
        .count()
}

fn zoned(w: &World) -> usize {
    w.zones.cells.iter().filter(|&&c| c != 0).count()
}

/// Distance from `(x, y)` to the nearest active road centreline, from the road snapshot.
fn road_dist(w: &World, x: f32, y: f32) -> f32 {
    let mut segs = Vec::new();
    w.write_road_segments(&mut segs);
    segs.chunks(5)
        .map(|s| {
            let (dx, dy) = (s[2] - s[0], s[3] - s[1]);
            let t = (((x - s[0]) * dx + (y - s[1]) * dy) / (dx * dx + dy * dy)).clamp(0.0, 1.0);
            (s[0] + dx * t - x).hypot(s[1] + dy * t - y)
        })
        .fold(f32::MAX, f32::min)
}

/// Every active lot is one simple shape of at least `MIN_LOT_AREA`, out of the roads, facing
/// a road it fronts; no two overlap, and each owns the zone cells whose centres it covers.
fn check_lots(w: &World) -> BTreeSet<LotId> {
    let lots: BTreeSet<LotId> = w.lots.active_lots().collect();
    let mut seen = BTreeSet::new();
    for &lot in &lots {
        let o = w.lots.outline(lot);
        assert!(o.len() >= 3, "lot {lot} has no shape");
        assert!(area(o) >= MIN_LOT_AREA - 0.5, "lot {lot} is {} m²", area(o));
        assert_simple(lot, o);
        // Spikes and slivers, where lots from two roads meet, have hair-thin corners.
        assert!(sharpest(o) > 5.0, "lot {lot} has a {}° corner", sharpest(o));
        for &c in w.lots.cells(lot) {
            assert!(seen.insert(c), "cell {c} is in two lots");
            assert_eq!(w.lots.owner(c), Some(lot));
            assert!(
                inside(o, w.zones.centre(c)),
                "lot {lot} owns cell {c} outside it"
            );
        }
        for (x, y) in o.iter().copied() {
            let d = road_dist(w, x, y);
            assert!(
                d >= ROAD_HALF_WIDTH - 0.05,
                "lot {lot} corner under a road ({d} m)"
            );
        }
        let (cx, cy) = centroid(o);
        assert!(
            road_dist(w, cx, cy) <= LOT_REACH,
            "lot {lot} is out of reach"
        );
        let l = lot as usize;
        let edge = o
            .iter()
            .zip(o.iter().cycle().skip(1))
            .map(|(&a, &b)| seg_dist(a, b, (w.lots.front_x[l], w.lots.front_y[l])))
            .fold(f32::MAX, f32::min);
        assert!(
            edge <= ROAD_HALF_WIDTH + 0.5,
            "lot {lot}'s front is {edge} m from it"
        );
    }
    // No overlaps: a 2 m sample grid hits at most one lot.
    for &a in &lots {
        let oa = w.lots.outline(a);
        let ra = bounds(oa);
        for &b in lots.range(a + 1..) {
            let ob = w.lots.outline(b);
            let rb = bounds(ob);
            let (x0, y0, x1, y1) = (
                ra[0].max(rb[0]),
                ra[1].max(rb[1]),
                ra[2].min(rb[2]),
                ra[3].min(rb[3]),
            );
            let mut y = y0 + 0.5;
            while y < y1 {
                let mut x = x0 + 0.5;
                while x < x1 {
                    let p = (x, y);
                    let deep = |o: &[(f32, f32)]| {
                        inside(o, p)
                            && o.iter()
                                .zip(o.iter().cycle().skip(1))
                                .all(|(&u, &v)| seg_dist(u, v, p) > 0.05)
                    };
                    assert!(!(deep(oa) && deep(ob)), "lots {a} and {b} overlap at {p:?}");
                    x += 2.0;
                }
                y += 2.0;
            }
        }
    }
    lots
}

fn area(o: &[(f32, f32)]) -> f32 {
    o.iter()
        .zip(o.iter().cycle().skip(1))
        .map(|(a, b)| a.0 * b.1 - b.0 * a.1)
        .sum::<f32>()
        .abs()
        / 2.0
}

/// The centre of the lot's area.
fn centroid(o: &[(f32, f32)]) -> (f32, f32) {
    let (mut a, mut cx, mut cy) = (0.0, 0.0, 0.0);
    for (p, q) in o.iter().zip(o.iter().cycle().skip(1)) {
        let c = p.0 * q.1 - q.0 * p.1;
        a += c;
        cx += (p.0 + q.0) * c;
        cy += (p.1 + q.1) * c;
    }
    (cx / (3.0 * a), cy / (3.0 * a))
}

fn bounds(o: &[(f32, f32)]) -> [f32; 4] {
    o.iter()
        .fold([f32::MAX, f32::MAX, f32::MIN, f32::MIN], |r, p| {
            [r[0].min(p.0), r[1].min(p.1), r[2].max(p.0), r[3].max(p.1)]
        })
}

fn inside(o: &[(f32, f32)], p: (f32, f32)) -> bool {
    let mut c = false;
    for (a, b) in o.iter().zip(o.iter().cycle().skip(1)) {
        if (a.1 > p.1) != (b.1 > p.1) && p.0 < a.0 + (p.1 - a.1) / (b.1 - a.1) * (b.0 - a.0) {
            c = !c;
        }
    }
    c
}

fn seg_dist(a: (f32, f32), b: (f32, f32), p: (f32, f32)) -> f32 {
    let (dx, dy) = (b.0 - a.0, b.1 - a.1);
    let len2 = dx * dx + dy * dy;
    let t = if len2 > 0.0 {
        (((p.0 - a.0) * dx + (p.1 - a.1) * dy) / len2).clamp(0.0, 1.0)
    } else {
        0.0
    };
    (a.0 + dx * t - p.0).hypot(a.1 + dy * t - p.1)
}

/// The outline is one ring whose edges cross only where neighbours meet.
fn assert_simple(lot: LotId, o: &[(f32, f32)]) {
    let n = o.len();
    let cross = |a: (f32, f32), b: (f32, f32), c: (f32, f32)| {
        (b.0 - a.0) * (c.1 - a.1) - (b.1 - a.1) * (c.0 - a.0)
    };
    for i in 0..n {
        for j in i + 2..n {
            if i == 0 && j == n - 1 {
                continue;
            }
            let (a, b, c, d) = (o[i], o[(i + 1) % n], o[j], o[(j + 1) % n]);
            let proper =
                cross(a, b, c) * cross(a, b, d) < 0.0 && cross(c, d, a) * cross(c, d, b) < 0.0;
            assert!(!proper, "lot {lot} crosses itself");
        }
    }
}

/// The sharpest corner of the outline, degrees.
fn sharpest(o: &[(f32, f32)]) -> f32 {
    let n = o.len();
    (0..n)
        .map(|i| {
            let (p, c, q) = (o[(i + n - 1) % n], o[i], o[(i + 1) % n]);
            let (a, b) = ((p.0 - c.0, p.1 - c.1), (q.0 - c.0, q.1 - c.1));
            let cos = (a.0 * b.0 + a.1 * b.1) / (a.0.hypot(a.1) * b.0.hypot(b.1));
            cos.clamp(-1.0, 1.0).acos().to_degrees()
        })
        .fold(f32::MAX, f32::min)
}
