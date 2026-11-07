//! Matching recorded roads to the network and putting them back (ADR 0051).

use super::{Record, RoadRecord};
use crate::block::{Rect, shape_rect, union};
use crate::geometry::{Point, dist};
use crate::road::{NodeId, RoadId, RoadNetwork};
use crate::snap::{self, EXACT_EPS};

/// The road records in `list`.
fn roads(list: &[Record]) -> Vec<RoadRecord> {
    list.iter()
        .filter_map(|r| match *r {
            Record::Road(road) => Some(road),
            _ => None,
        })
        .collect()
}

/// The area the road records in both lists cover, if any.
pub(super) fn area(remove: &[Record], restore: &[Record]) -> Option<Rect> {
    roads(remove)
        .into_iter()
        .chain(roads(restore))
        .map(|r| shape_rect(r.shape()))
        .reduce(union)
}

/// Removes the roads `remove` names and builds the ones `restore` names, all exactly where
/// they stood. A road in both at the same place only changes class, in place. `split` hears of
/// each road split, with the road continuing it. Returns the roads removed, or None if a road
/// to remove is not on the map or one to restore does not fit; `net` is then half changed, so
/// callers try on a copy first.
pub(super) fn replay(
    net: &mut RoadNetwork,
    remove: &[Record],
    restore: &[Record],
    split: &mut impl FnMut(RoadId, RoadId),
) -> Option<Vec<RoadId>> {
    let (gone, back) = (roads(remove), roads(restore));
    let mut taken = vec![false; net.lane_count() / 2];
    let mut matched = Vec::with_capacity(gone.len());
    for &g in &gone {
        let ids = find(net, g, &taken)?;
        for &id in &ids {
            taken[id as usize] = true;
        }
        matched.push(ids);
    }
    let mut reclassed = vec![false; back.len()];
    let mut removed = Vec::new();
    for (g, ids) in gone.into_iter().zip(matched) {
        match (0..back.len()).find(|&j| !reclassed[j] && back[j].same_place(g)) {
            Some(j) => {
                reclassed[j] = true;
                for id in ids {
                    net.set_class(id, back[j].class);
                }
            }
            None => {
                for id in ids {
                    net.remove_road(id);
                    removed.push(id);
                }
            }
        }
    }
    for (b, done) in back.into_iter().zip(reclassed) {
        if !done {
            let plan = snap::exact_plan(net, b.shape())?;
            snap::apply(net, &plan, b.class, &mut *split);
        }
    }
    Some(removed)
}

/// Active roads of the record's class, not yet `taken`, that together run exactly along it
/// from end to end. Zoning may have cut alleys into it since, so it may be several pieces.
fn find(net: &RoadNetwork, record: RoadRecord, taken: &[bool]) -> Option<Vec<RoadId>> {
    let shape = record.shape();
    let r = shape_rect(shape);
    let inside = |p: Point| {
        r[0] - EXACT_EPS <= p.0
            && p.0 <= r[2] + EXACT_EPS
            && r[1] - EXACT_EPS <= p.1
            && p.1 <= r[3] + EXACT_EPS
    };
    let along = |p: Point| {
        let t = shape.nearest(p);
        (dist(shape.point(t), p) < EXACT_EPS).then_some(t)
    };
    let mut pieces: Vec<(f32, NodeId, NodeId, RoadId)> = Vec::new();
    for road in net.active_roads() {
        let s = net.shape(road);
        if taken[road as usize] || net.road_class(road) != record.class || !inside(s.a) {
            continue;
        }
        let (Some(t), Some(u), Some(_)) = (along(s.a), along(s.b), along(s.point(0.5))) else {
            continue;
        };
        let (a, b) = net.road_ends(road);
        pieces.push(if t <= u {
            (t, a, b, road)
        } else {
            (u, b, a, road)
        });
    }
    pieces.sort_by(|p, q| p.0.total_cmp(&q.0));
    let node = |n: NodeId| (net.node_x[n as usize], net.node_y[n as usize]);
    let (first, last) = (pieces.first()?, pieces.last()?);
    let whole = dist(node(first.1), shape.a) < EXACT_EPS
        && dist(node(last.2), shape.b) < EXACT_EPS
        && pieces.windows(2).all(|w| w[0].2 == w[1].1);
    whole.then(|| pieces.into_iter().map(|p| p.3).collect())
}
