//! Finding what an edit changed (ADR 0051, amended): copy every road's shape and class and
//! every piece's active flag before the edit, then compare. IDs are append-only and node
//! positions never change, so a road whose record differs was changed by this edit.

use super::{Record, RoadRecord};
use crate::road::RoadId;
use crate::world::World;

pub(in crate::world) struct Capture {
    roads: Vec<Option<RoadRecord>>,
    plants: Vec<bool>,
    lines: Vec<bool>,
    water: Vec<bool>,
    stations: Vec<bool>,
    balance: i64,
}

/// What an edit removed and added, in kind then ID order.
pub(in crate::world) struct Change {
    pub before: Vec<Record>,
    pub after: Vec<Record>,
    /// Dollars the edit took from the balance.
    pub cost: i64,
}

impl Capture {
    pub(in crate::world) fn of(w: &World) -> Self {
        Self {
            roads: (0..w.roads.lane_count() as RoadId / 2)
                .map(|r| road(w, r))
                .collect(),
            plants: w.power.plant_active.clone(),
            lines: w.power.line_active.clone(),
            water: w.water.active.clone(),
            stations: w.fire.station_active.clone(),
            balance: w.treasury.balance,
        }
    }

    pub(in crate::world) fn diff(&self, w: &World) -> Change {
        let mut change = Change {
            before: Vec::new(),
            after: Vec::new(),
            cost: (self.balance - w.treasury.balance).max(0),
        };
        for r in 0..w.roads.lane_count() as RoadId / 2 {
            let (old, new) = (self.roads.get(r as usize).copied().flatten(), road(w, r));
            if old != new {
                change.before.extend(old.map(Record::Road));
                change.after.extend(new.map(Record::Road));
            }
        }
        let mut pieces = |was: &[bool], now: &[bool], record: &dyn Fn(usize) -> Record| {
            for (i, &on) in now.iter().enumerate() {
                let old = was.get(i).copied().unwrap_or(false);
                if old && !on {
                    change.before.push(record(i));
                } else if on && !old {
                    change.after.push(record(i));
                }
            }
        };
        let (p, f) = (&w.power, &w.fire);
        pieces(&self.plants, &p.plant_active, &|i| {
            let at = p.footprint(i as u32);
            Record::Plant {
                kind: p.plant_kind[i],
                centre: at.centre,
                front: at.front,
            }
        });
        pieces(&self.lines, &p.line_active, &|i| {
            let (a, b) = p.line(i as u32);
            Record::Line { a, b }
        });
        pieces(&self.water, &w.water.active, &|i| {
            let at = w.water.footprint(i as u32);
            Record::Water {
                kind: w.water.kind[i],
                centre: at.centre,
                front: at.front,
            }
        });
        pieces(&self.stations, &f.station_active, &|i| {
            let at = f.footprint(i as u32);
            Record::Station {
                centre: at.centre,
                front: at.front,
            }
        });
        change
    }
}

fn road(w: &World, r: RoadId) -> Option<RoadRecord> {
    w.roads.is_road_active(r).then(|| {
        let s = w.roads.shape(r);
        RoadRecord {
            a: s.a,
            b: s.b,
            control: s.control,
            class: w.roads.road_class(r),
        }
    })
}
