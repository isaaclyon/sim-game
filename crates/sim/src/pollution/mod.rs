//! Pollution and land value on a coarse grid aligned to the zone grid (ADR 0037).
//!
//! Air pollution and noise come from industrial buildings, coal plants (ADR 0044) and traffic
//! flow and follow their current sources. Ground pollution builds up under and beside
//! industrial buildings and coal plants and fades slowly once they go. Land value falls as pollution rises. Everything here is recomputed on a
//! fixed sim-time cadence and saved. Traffic flow, ground pollution and land value are hashed;
//! air and noise only feed land value and the display.

mod field;

use serde::{Deserialize, Serialize};

use crate::check::{Check, ensure};
use crate::zone::{ZONE_CELL, ZoneGrid};

/// Zone cells per pollution cell side: 32 m cells.
pub const ZONE_CELLS_PER_CELL: u32 = 4;
/// Pollution cell side, metres.
pub const POLLUTION_CELL: f32 = ZONE_CELL * ZONE_CELLS_PER_CELL as f32;
/// Sim seconds between pollution updates.
pub const UPDATE_S: f32 = 10.0;
/// Bytes per cell in `write_snapshot`.
pub const POLLUTION_STRIDE: usize = 4;

/// Air spreads this many cells (with linear falloff), noise this many.
const AIR_RADIUS: usize = 4;
const NOISE_RADIUS: usize = 2;
/// Spread emission that reads as fully polluted.
const AIR_FULL: f32 = 2.0;
const NOISE_FULL: f32 = 1.5;
/// Ground pollution rises to full in about half a sim day and fades over about three.
const GROUND_MAX: u16 = u16::MAX;
const GROUND_RISE: u16 = 92;
const GROUND_FALL: u16 = 16;
/// How much each kind, at full strength, takes off land value.
const AIR_WEIGHT: f32 = 0.6;
const NOISE_WEIGHT: f32 = 0.4;
const GROUND_WEIGHT: f32 = 0.8;

/// Ground pollution a cell is heading toward this update.
#[derive(Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum GroundSource {
    #[default]
    None,
    /// Next to an industrial building: half strength.
    Beside,
    /// Under an industrial building.
    Under,
}

/// Emissions gathered for one update, per cell.
pub(crate) struct Sources {
    pub(crate) air: Vec<f32>,
    pub(crate) noise: Vec<f32>,
    pub(crate) ground: Vec<GroundSource>,
}

/// One place's pollution, each kind and land value in `0..=1` (1 = clean, full land value).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PollutionSample {
    pub air: f32,
    pub ground: f32,
    pub noise: f32,
    pub land_value: f32,
}

#[derive(Default, Clone, Serialize, Deserialize)]
pub struct Pollution {
    pub x0: f32,
    pub y0: f32,
    pub cols: u32,
    pub rows: u32,
    /// Smoothed cars on each road, by road ID (both lanes together). Tombstoned roads keep 0.
    pub(crate) flow: Vec<f32>,
    pub(crate) ground: Vec<u16>,
    /// Air, noise and land value as the last update left them. Saved, because buildings may
    /// have changed since, and growth reads land value until the next update.
    air: Vec<u8>,
    noise: Vec<u8>,
    land: Vec<u8>,
    /// Bumped when the snapshot bytes change, so the frontend knows when to redraw.
    #[serde(skip)]
    pub version: u32,
}

fn unit(v: u8) -> f32 {
    f32::from(v) / 255.0
}

fn byte(v: f32) -> u8 {
    (v.clamp(0.0, 1.0) * 255.0).round() as u8
}

impl Pollution {
    /// A clean grid over `zones`, sharing its origin.
    pub fn covering(zones: &ZoneGrid) -> Self {
        let cols = zones.cols.div_ceil(ZONE_CELLS_PER_CELL);
        let rows = zones.rows.div_ceil(ZONE_CELLS_PER_CELL);
        let n = (cols * rows) as usize;
        Self {
            x0: zones.x0,
            y0: zones.y0,
            cols,
            rows,
            flow: Vec::new(),
            ground: vec![0; n],
            air: vec![0; n],
            noise: vec![0; n],
            land: vec![255; n],
            version: 0,
        }
    }

    pub fn len(&self) -> usize {
        self.ground.len()
    }

    pub fn is_empty(&self) -> bool {
        self.ground.is_empty()
    }

    /// The cell containing `(x, y)`, if it is on the grid.
    pub fn cell_at(&self, x: f32, y: f32) -> Option<usize> {
        let c = ((x - self.x0) / POLLUTION_CELL).floor();
        let r = ((y - self.y0) / POLLUTION_CELL).floor();
        let inside = (0.0..self.cols as f32).contains(&c) && (0.0..self.rows as f32).contains(&r);
        inside.then(|| r as usize * self.cols as usize + c as usize)
    }

    /// The cell holding zone cell `index` of a zone grid `zone_cols` wide.
    pub fn cell_of_zone_cell(&self, index: u32, zone_cols: u32) -> usize {
        let (c, r) = (index % zone_cols, index / zone_cols);
        let (c, r) = (c / ZONE_CELLS_PER_CELL, r / ZONE_CELLS_PER_CELL);
        (r * self.cols + c) as usize
    }

    pub fn sample(&self, x: f32, y: f32) -> Option<PollutionSample> {
        let c = self.cell_at(x, y)?;
        Some(PollutionSample {
            air: unit(self.air[c]),
            ground: unit((self.ground[c] >> 8) as u8),
            noise: unit(self.noise[c]),
            land_value: unit(self.land[c]),
        })
    }

    /// Mean land value of the cells, `0..=255`; 255 for no cells.
    pub(crate) fn land_of(&self, cells: impl Iterator<Item = usize>) -> u8 {
        let (mut sum, mut n) = (0u32, 0u32);
        for c in cells {
            sum += u32::from(self.land[c]);
            n += 1;
        }
        sum.checked_div(n).map_or(255, |m| m as u8)
    }

    pub(crate) fn sources(&self) -> Sources {
        let n = self.len();
        Sources {
            air: vec![0.0; n],
            noise: vec![0.0; n],
            ground: vec![GroundSource::None; n],
        }
    }

    /// Moves ground pollution toward its sources, then rebuilds air, noise and land value.
    pub(crate) fn update(&mut self, src: &Sources) {
        let mut changed = false;
        for (g, s) in self.ground.iter_mut().zip(&src.ground) {
            let target = match s {
                GroundSource::Under => GROUND_MAX,
                GroundSource::Beside => GROUND_MAX / 2,
                GroundSource::None => 0,
            };
            let next = if *g < target {
                g.saturating_add(GROUND_RISE).min(target)
            } else {
                g.saturating_sub(GROUND_FALL).max(target)
            };
            changed |= next >> 8 != *g >> 8;
            *g = next;
        }
        if self.derive(src) || changed {
            self.version = self.version.wrapping_add(1);
        }
    }

    /// Spreads air and noise from `src` and recomputes land value, leaving ground pollution as
    /// it is. Returns whether any of them changed.
    pub(crate) fn derive(&mut self, src: &Sources) -> bool {
        let len = self.len();
        let (cols, rows) = (self.cols as usize, self.rows as usize);
        let air = field::spread(&src.air, cols, rows, AIR_RADIUS);
        let noise = field::spread(&src.noise, cols, rows, NOISE_RADIUS);
        let mut changed = false;
        for c in 0..len {
            let g = self.ground[c];
            let (a, n) = (byte(air[c] / AIR_FULL), byte(noise[c] / NOISE_FULL));
            let ground = unit((g >> 8) as u8);
            let harm = AIR_WEIGHT * unit(a) + NOISE_WEIGHT * unit(n) + GROUND_WEIGHT * ground;
            let land = byte(1.0 - harm);
            changed |= a != self.air[c] || n != self.noise[c] || land != self.land[c];
            self.air[c] = a;
            self.noise[c] = n;
            self.land[c] = land;
        }
        changed
    }

    /// `[air, ground, noise, land_value]` bytes per cell, row-major from the origin; 0 = none
    /// (for land value: worthless), 255 = full.
    pub fn write_snapshot(&self, out: &mut Vec<u8>) {
        out.clear();
        out.reserve(self.len() * POLLUTION_STRIDE);
        for c in 0..self.len() {
            out.extend([
                self.air[c],
                (self.ground[c] >> 8) as u8,
                self.noise[c],
                self.land[c],
            ]);
        }
    }

    /// `[origin_x, origin_y, cell_size, cols, rows]`.
    pub fn layout(&self) -> [f32; 5] {
        [
            self.x0,
            self.y0,
            POLLUTION_CELL,
            self.cols as f32,
            self.rows as f32,
        ]
    }

    pub(crate) fn hash_into(&self, mix: &mut impl FnMut(u64)) {
        mix(self.flow.len() as u64);
        for f in &self.flow {
            mix(f.to_bits() as u64);
        }
        mix(self.ground.len() as u64);
        for (g, l) in self.ground.iter().zip(&self.land) {
            mix(u64::from(*g) << 8 | u64::from(*l));
        }
    }

    /// Internal consistency of a loaded grid against its zone grid and roads (ADR 0033).
    pub(crate) fn check(&self, zones: &ZoneGrid, roads: usize) -> Check {
        let n = self.len();
        ensure(
            self.x0 == zones.x0
                && self.y0 == zones.y0
                && self.cols == zones.cols.div_ceil(ZONE_CELLS_PER_CELL)
                && self.rows == zones.rows.div_ceil(ZONE_CELLS_PER_CELL)
                && n == (self.cols * self.rows) as usize
                && [self.air.len(), self.noise.len(), self.land.len()] == [n; 3],
            "pollution grid",
        )?;
        ensure(
            self.flow.len() <= roads && self.flow.iter().all(|f| f.is_finite() && *f >= 0.0),
            "traffic flow",
        )
    }
}

#[cfg(test)]
mod tests;
