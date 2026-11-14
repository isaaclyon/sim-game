//! Placing and bulldozing schools, and the schools snapshot and previews (ADR 0055).

use super::{SCHOOL_STRIDE, SITE_INSET, START_BLOCKS, SchoolKind};
use crate::frontage::frontage;
use crate::power::{COAL_SETBACK, COAL_SNAP, Footprint};
use crate::world::World;

impl World {
    /// Where a school asked for at the cursor would stand: beside the nearest road in reach,
    /// facing it, as a fire station does (ADR 0042). `None` off the map or away from roads.
    pub fn school_spot(&self, kind: SchoolKind, x: f32, y: f32) -> Option<Footprint> {
        if !(x.is_finite() && y.is_finite() && x.abs() < 1e6 && y.abs() < 1e6) {
            return None;
        }
        let f = frontage(&self.roads, (x, y), COAL_SNAP)?;
        let half = kind.half_size();
        Some(Footprint {
            centre: f.out(f.kerb + COAL_SETBACK + half),
            front: f.front,
            half,
        })
    }

    /// Whether a school fits at its `school_spot`: beside a road and clear of roads, plants,
    /// pumps, towers, stations and schools. Says nothing about money.
    pub fn school_fits(&self, kind: SchoolKind, x: f32, y: f32) -> bool {
        self.school_spot(kind, x, y)
            .is_some_and(|at| self.spot_fits(at, true))
    }

    /// Places a school, clearing the land under it. Refused where it does not fit or when the
    /// city cannot pay.
    pub(in crate::world) fn place_school(&mut self, kind: SchoolKind, x: f32, y: f32) -> bool {
        let Some(at) = self.school_spot(kind, x, y) else {
            return false;
        };
        if !self.spot_fits(at, true) || !self.can_afford(kind.cost()) {
            return false;
        }
        self.treasury.balance -= kind.cost();
        self.add_school(kind, at);
        true
    }

    /// Adds a school and its building at `at` and clears the land under it, free of charge.
    pub(in crate::world) fn add_school(&mut self, kind: SchoolKind, at: Footprint) {
        self.claim_land(at);
        // The building stands inside the site; its first edge is the one facing the road.
        let r = Footprint {
            half: at.half - SITE_INSET,
            ..at
        }
        .ring();
        let building = self.buildings.add_service(
            kind.zone(),
            kind.height(),
            &[r[2], r[3], r[0], r[1]],
            self.era,
        );
        let s = &mut self.schools;
        s.kind.push(kind);
        s.x.push(at.centre.0);
        s.y.push(at.centre.1);
        s.fx.push(at.front.0);
        s.fy.push(at.front.1);
        s.active.push(true);
        s.building.push(building);
        s.blocks.push(START_BLOCKS);
        s.pupils.push(0);
        s.pending = true;
        s.bump();
        self.ensure_schools();
        self.refresh_upkeep();
    }

    /// Bulldozes the school under the point.
    pub(in crate::world) fn bulldoze_school(&mut self, x: f32, y: f32) -> bool {
        let Some(s) = self.school_at(x, y) else {
            return false;
        };
        self.remove_school(s);
        true
    }

    /// Removes school `s` and its building; its pupils lose their seats and its teachers
    /// their jobs.
    pub(in crate::world) fn remove_school(&mut self, s: u32) {
        self.release_pupils(s);
        self.schools.active[s as usize] = false;
        self.buildings
            .remove_service(self.schools.building[s as usize]);
        self.schools.bump();
        self.ensure_schools();
        self.refresh_upkeep();
    }

    /// The active school whose site holds the point.
    pub(in crate::world) fn school_at(&self, x: f32, y: f32) -> Option<u32> {
        if !(x.is_finite() && y.is_finite()) {
            return None;
        }
        self.schools
            .active_schools()
            .find(|&s| self.schools.footprint(s).contains((x, y), 0.0))
    }

    /// Whether a school overlaps the footprint, so nothing else is placed there.
    pub(in crate::world) fn school_overlaps(&self, at: Footprint) -> bool {
        self.schools
            .active_schools()
            .any(|s| self.schools.footprint(s).overlaps(at))
    }

    /// Whether a school stands on the lot, so nothing grows there.
    pub(in crate::world) fn school_blocks(&self, lot: crate::lot::LotId) -> bool {
        let ring = self.lots.outline(lot);
        self.schools
            .active_schools()
            .any(|s| self.schools.footprint(s).hits_ring(ring))
    }

    /// Bumped whenever a school is placed or removed.
    pub fn schools_version(&self) -> u32 {
        self.schools.version
    }

    /// `[kind, x, y, half_size, front_x, front_y]` per active school (`SCHOOL_STRIDE`): its
    /// square site, turned to face its road.
    pub fn write_schools(&self, out: &mut Vec<f32>) {
        out.clear();
        let s = &self.schools;
        out.reserve(s.active.len() * SCHOOL_STRIDE);
        for i in s.active_schools().map(|i| i as usize) {
            let kind = s.kind[i];
            out.extend([
                f32::from(kind as u8),
                s.x[i],
                s.y[i],
                kind.half_size(),
                s.fx[i],
                s.fy[i],
            ]);
        }
    }

    /// `[ok, affordable, x0, y0, …]`: whether a school would fit and can be paid for, then
    /// the closed outline of the site where it would stand; empty away from roads.
    pub fn write_school_preview(&self, kind: SchoolKind, x: f32, y: f32, out: &mut Vec<f32>) {
        out.clear();
        let Some(at) = self.school_spot(kind, x, y) else {
            return;
        };
        self.push_placement(self.spot_fits(at, true), kind.cost(), out);
        let ring = at.ring();
        for p in ring.iter().chain(ring.first()) {
            out.extend([p.0, p.1]);
        }
    }

    /// The closed outline of the school under the point; false if there is none.
    pub(in crate::world) fn write_school_bulldoze(
        &self,
        x: f32,
        y: f32,
        out: &mut Vec<f32>,
    ) -> bool {
        let Some(s) = self.school_at(x, y) else {
            return false;
        };
        let ring = self.schools.footprint(s).ring();
        for p in ring.iter().chain(ring.first()) {
            out.extend([p.0, p.1]);
        }
        true
    }
}
