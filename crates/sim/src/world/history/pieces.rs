//! Matching recorded plants, lines, pumps, towers and stations, and putting them back
//! (ADR 0051, amended).

use super::Record;
use crate::geometry::{Point, dist};
use crate::power::{Footprint, PlantKind, segment_hits_ring};
use crate::road::RoadNetwork;
use crate::snap::EXACT_EPS;
use crate::world::World;
use crate::world::fire::STATION_HALF;

/// A standing piece, by ID.
#[derive(Clone, Copy)]
pub(super) enum Piece {
    Plant(u32),
    Line(u32),
    Water(u32),
    Station(u32),
}

pub(super) fn footprint(record: Record) -> Option<Footprint> {
    let (centre, front, half) = match record {
        Record::Plant {
            kind,
            centre,
            front,
        } => (centre, front, kind.half_size()),
        Record::Water {
            kind,
            centre,
            front,
        } => (centre, front, kind.half_size()),
        Record::Station { centre, front } => (centre, front, STATION_HALF),
        Record::Road(_) | Record::Line { .. } => return None,
    };
    Some(Footprint {
        centre,
        front,
        half,
    })
}

impl World {
    /// The standing pieces the records in `remove` name, or None if one is gone.
    pub(super) fn find_pieces(&self, remove: &[Record]) -> Option<Vec<Piece>> {
        let same = |p: Point, q: Point| dist(p, q) < EXACT_EPS;
        let mut found = Vec::new();
        for &record in remove {
            let piece = match record {
                Record::Road(_) => continue,
                Record::Plant { kind, centre, .. } => {
                    let p = &self.power;
                    p.active_plants()
                        .find(|&i| {
                            p.plant_kind[i as usize] == kind && same(p.footprint(i).centre, centre)
                        })
                        .map(Piece::Plant)
                }
                Record::Line { a, b } => self
                    .power
                    .active_lines()
                    .find(|&l| {
                        let (p, q) = self.power.line(l);
                        (same(p, a) && same(q, b)) || (same(p, b) && same(q, a))
                    })
                    .map(Piece::Line),
                Record::Water { kind, centre, .. } => {
                    let w = &self.water;
                    w.active_pieces()
                        .find(|&i| {
                            w.kind[i as usize] == kind && same(w.footprint(i).centre, centre)
                        })
                        .map(Piece::Water)
                }
                Record::Station { centre, .. } => self
                    .fire
                    .active_stations()
                    .find(|&s| same(self.fire.footprint(s).centre, centre))
                    .map(Piece::Station),
            };
            found.push(piece?);
        }
        Some(found)
    }

    /// Whether each piece in `restore` fits where it stood once the roads are `net`, by the
    /// checks placing it by hand passes. Money is the caller's.
    pub(super) fn pieces_fit(&self, net: &RoadNetwork, restore: &[Record]) -> bool {
        restore.iter().all(|&record| match record {
            Record::Road(_) => true,
            Record::Line { a, b } => self.power.line_fits(a, b),
            Record::Plant { kind, .. } => {
                let at = footprint(record).expect("a plant has a footprint");
                self.spot_fits_on(net, at, kind == PlantKind::Coal)
            }
            Record::Water { .. } | Record::Station { .. } => {
                let at = footprint(record).expect("a roadside piece has a footprint");
                self.spot_fits_on(net, at, true)
            }
        })
    }

    pub(super) fn remove_piece(&mut self, piece: Piece) {
        match piece {
            Piece::Plant(p) => self.power.remove_plant(p),
            Piece::Line(l) => self.power.remove_line(l),
            Piece::Water(p) => self.water.remove(p),
            Piece::Station(s) => self.remove_station(s),
        }
    }

    /// Puts a piece back as placing it does: clearing the land under it, free of charge.
    pub(super) fn restore_piece(&mut self, record: Record) {
        let at = footprint(record);
        match record {
            Record::Road(_) => {}
            Record::Line { a, b } => {
                if self.power.add_line(a, b) {
                    self.clear_buildings(|ring| segment_hits_ring(a, b, ring));
                }
            }
            Record::Plant { kind, .. } => {
                let at = at.expect("a plant has a footprint");
                self.power.add_plant(kind, at);
                self.claim_land(at);
            }
            Record::Water { kind, .. } => {
                let at = at.expect("a pump or tower has a footprint");
                self.water.add(kind, at);
                self.claim_land(at);
            }
            Record::Station { .. } => self.add_station(at.expect("a station has a footprint")),
        }
    }
}
