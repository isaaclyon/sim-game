//! Power plants and transmission lines (ADR 0041). Only what the player placed is stored here;
//! grids, supply and which buildings are powered are derived from it (`grid`, `world::power`).

mod geom;
pub mod grid;

use serde::{Deserialize, Serialize};

use crate::geometry::Point;
pub use geom::Footprint;
pub(crate) use geom::segment_hits_ring;

pub type PlantId = u32;
pub type LineId = u32;

/// Power everyone shares before any plant's, given to the oldest buildings wherever they stand.
pub const STARTER_POWER: u32 = 150;
/// A plant joins a road whose centreline comes this close to its footprint, metres.
pub const PLANT_REACH: f32 = 12.0;
/// A plant's line terminal stands this far behind its back edge, metres.
pub const TERMINAL_GAP: f32 = 2.0;
/// A coal plant's front edge keeps this far outside the kerb of the road it faces, metres.
pub const COAL_SETBACK: f32 = 1.0;
/// A coal plant snaps to a road whose centreline is this close to the cursor, metres.
pub const COAL_SNAP: f32 = 80.0;
/// A line end joins the plant, road or line end it lies this close to, metres.
pub const LINE_REACH: f32 = 8.0;
/// Shorter lines are refused, metres.
const MIN_LINE: f32 = 4.0;
/// Clock seconds between updates of solar and wind output.
pub const OUTPUT_STEP_S: u64 = 600;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[repr(u8)]
pub enum PlantKind {
    /// Steady output; must touch a road.
    Coal = 0,
    /// Follows the day clock; nothing at night.
    Solar = 1,
    /// Follows a smooth, seeded wind signal.
    Wind = 2,
}

impl PlantKind {
    pub fn from_u8(v: u8) -> Option<Self> {
        [Self::Coal, Self::Solar, Self::Wind]
            .get(v as usize)
            .copied()
    }

    /// Half the side of the square footprint, metres.
    pub fn half_size(self) -> f32 {
        match self {
            Self::Coal => 22.0,
            Self::Solar => 16.0,
            Self::Wind => 5.0,
        }
    }

    /// Industrial jobs it adds to demand (ADR 0025). Nobody is assigned to them yet.
    pub fn jobs(self) -> u32 {
        if self == Self::Coal { 40 } else { 0 }
    }

    /// Output at full sun or full wind, in the units of a building's power demand.
    pub fn peak(self) -> u32 {
        match self {
            Self::Coal => 3000,
            Self::Solar => 800,
            Self::Wind => 500,
        }
    }

    /// Output at `clock_secs` (see `clock::clock_seconds`). Integer math, so every target agrees.
    pub fn output(self, seed: u64, clock_secs: u64) -> u32 {
        let peak = u64::from(self.peak());
        let permille = match self {
            Self::Coal => 1000,
            Self::Solar => {
                // A parabola from 06:00 to 18:00, full at noon.
                let s = clock_secs % 86_400;
                if !(6 * 3600..18 * 3600).contains(&s) {
                    0
                } else {
                    let t = (s - 6 * 3600) * 1000 / (12 * 3600);
                    4 * t * (1000 - t) / 1000
                }
            }
            Self::Wind => {
                let hour = clock_secs / 3600;
                let f = (clock_secs % 3600) * 1000 / 3600;
                let ease = f * f * (3000 - 2 * f) / 1_000_000;
                let (a, b) = (wind(seed, hour), wind(seed, hour + 1));
                (a * (1000 - ease) + b * ease) / 1000
            }
        };
        (peak * permille / 1000) as u32
    }
}

/// Wind strength for an hour, 200–1000 per mille, from the seed alone (SplitMix64).
fn wind(seed: u64, hour: u64) -> u64 {
    let mut z = (seed ^ 0x771d_5eed).wrapping_add(hour.wrapping_mul(0x9E37_79B9_7F4A_7C15));
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    200 + (z ^ (z >> 31)) % 801
}

/// What the player built for power. IDs are indices and append-only; removal tombstones.
#[derive(Default, Serialize, Deserialize)]
pub struct Power {
    pub plant_kind: Vec<PlantKind>,
    pub plant_x: Vec<f32>,
    pub plant_y: Vec<f32>,
    /// Unit direction the plant's front faces: toward its road for coal, +y otherwise.
    pub plant_fx: Vec<f32>,
    pub plant_fy: Vec<f32>,
    pub plant_active: Vec<bool>,
    pub line_ax: Vec<f32>,
    pub line_ay: Vec<f32>,
    pub line_bx: Vec<f32>,
    pub line_by: Vec<f32>,
    pub line_active: Vec<bool>,
    /// Bumped on every change.
    pub version: u32,
}

impl Power {
    pub fn active_plants(&self) -> impl Iterator<Item = PlantId> + '_ {
        let on = &self.plant_active;
        (0..on.len()).filter(|&p| on[p]).map(|p| p as PlantId)
    }

    pub fn active_lines(&self) -> impl Iterator<Item = LineId> + '_ {
        let on = &self.line_active;
        (0..on.len()).filter(|&l| on[l]).map(|l| l as LineId)
    }

    pub fn footprint(&self, p: PlantId) -> Footprint {
        let i = p as usize;
        Footprint {
            centre: (self.plant_x[i], self.plant_y[i]),
            front: (self.plant_fx[i], self.plant_fy[i]),
            half: self.plant_kind[i].half_size(),
        }
    }

    /// Where lines join the plant: just behind its back edge, away from its road.
    pub fn terminal(&self, p: PlantId) -> Point {
        self.footprint(p).behind(TERMINAL_GAP)
    }

    /// Footprints by plant ID; `None` for removed plants.
    pub(crate) fn footprints(&self) -> Vec<Option<Footprint>> {
        (0..self.plant_active.len())
            .map(|p| self.plant_active[p].then(|| self.footprint(p as PlantId)))
            .collect()
    }

    /// Ends by line ID; `None` for removed lines.
    pub(crate) fn lines(&self) -> Vec<Option<(Point, Point)>> {
        (0..self.line_active.len())
            .map(|l| self.line_active[l].then(|| self.line(l as LineId)))
            .collect()
    }

    pub(crate) fn line(&self, l: LineId) -> (Point, Point) {
        let i = l as usize;
        (
            (self.line_ax[i], self.line_ay[i]),
            (self.line_bx[i], self.line_by[i]),
        )
    }

    /// The active plant whose footprint, grown by `margin`, contains the point.
    pub fn plant_at(&self, x: f32, y: f32, margin: f32) -> Option<PlantId> {
        self.active_plants()
            .find(|&p| self.footprint(p).contains((x, y), margin))
    }

    pub(crate) fn add_plant(&mut self, kind: PlantKind, at: Footprint) {
        self.plant_kind.push(kind);
        self.plant_x.push(at.centre.0);
        self.plant_y.push(at.centre.1);
        self.plant_fx.push(at.front.0);
        self.plant_fy.push(at.front.1);
        self.plant_active.push(true);
        self.version = self.version.wrapping_add(1);
    }

    /// Whether a line from `a` to `b` is long enough and duplicates no standing line.
    pub(crate) fn line_fits(&self, a: Point, b: Point) -> bool {
        let same = |l| {
            let (p, q) = self.line(l);
            (p == a && q == b) || (p == b && q == a)
        };
        crate::geometry::dist(a, b) >= MIN_LINE && !self.active_lines().any(same)
    }

    /// Refuses short lines and duplicates of a standing line.
    pub(crate) fn add_line(&mut self, a: Point, b: Point) -> bool {
        if !self.line_fits(a, b) {
            return false;
        }
        self.line_ax.push(a.0);
        self.line_ay.push(a.1);
        self.line_bx.push(b.0);
        self.line_by.push(b.1);
        self.line_active.push(true);
        self.version = self.version.wrapping_add(1);
        true
    }

    pub(crate) fn remove_plant(&mut self, p: PlantId) {
        self.plant_active[p as usize] = false;
        self.version = self.version.wrapping_add(1);
    }

    pub(crate) fn remove_line(&mut self, l: LineId) {
        self.line_active[l as usize] = false;
        self.version = self.version.wrapping_add(1);
    }

    pub(crate) fn hash_into(&self, mix: &mut impl FnMut(u64)) {
        mix(self.plant_active.len() as u64);
        for p in 0..self.plant_active.len() {
            mix(self.plant_active[p] as u64 | (self.plant_kind[p] as u64) << 1);
            mix(u64::from(self.plant_x[p].to_bits()) << 32 | u64::from(self.plant_y[p].to_bits()));
            mix(u64::from(self.plant_fx[p].to_bits()) << 32 | u64::from(self.plant_fy[p].to_bits()));
        }
        mix(self.line_active.len() as u64);
        for l in 0..self.line_active.len() {
            let (a, b) = self.line(l as LineId);
            mix(self.line_active[l] as u64);
            mix(u64::from(a.0.to_bits()) << 32 | u64::from(a.1.to_bits()));
            mix(u64::from(b.0.to_bits()) << 32 | u64::from(b.1.to_bits()));
        }
    }

    /// Internal consistency of loaded power (ADR 0033).
    pub(crate) fn check(&self) -> crate::check::Check {
        use crate::check::{finite, lens};
        let plants = self.plant_active.len();
        lens(
            &[
                self.plant_kind.len(),
                self.plant_x.len(),
                self.plant_y.len(),
                self.plant_fx.len(),
                self.plant_fy.len(),
            ],
            plants,
            "plant fields",
        )?;
        let lines = self.line_active.len();
        let ends = [&self.line_ax, &self.line_ay, &self.line_bx, &self.line_by];
        lens(&ends.map(Vec::len), lines, "line fields")?;
        finite(self.plant_x.iter().chain(&self.plant_y), "plant positions")?;
        let unit = |(x, y): (&f32, &f32)| ((x * x + y * y) - 1.0).abs() < 1e-3;
        crate::check::ensure(
            self.plant_fx.iter().zip(&self.plant_fy).all(unit),
            "plant fronts",
        )?;
        finite(ends.into_iter().flatten(), "line ends")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn solar_follows_the_sun() {
        let at = |h: u64| PlantKind::Solar.output(1, h * 3600);
        assert_eq!(at(3), 0);
        assert_eq!(at(12), 800);
        assert!(at(8) > 0 && at(8) < at(10));
        assert_eq!(at(20), 0);
        assert_eq!(PlantKind::Solar.output(1, 86_400 + 12 * 3600), 800);
    }

    #[test]
    fn wind_varies_smoothly_and_never_stops() {
        let mut last = PlantKind::Wind.output(7, 0);
        let mut seen = std::collections::BTreeSet::new();
        for step in 1..=(48 * 6) {
            let out = PlantKind::Wind.output(7, step * OUTPUT_STEP_S);
            assert!((100..=500).contains(&out));
            assert!(out.abs_diff(last) <= 100, "jumped from {last} to {out}");
            seen.insert(out / 50);
            last = out;
        }
        assert!(seen.len() > 3, "wind should change over two days");
        assert_ne!(
            PlantKind::Wind.output(7, 5 * 3600),
            PlantKind::Wind.output(8, 5 * 3600)
        );
    }

    #[test]
    fn coal_is_steady() {
        assert_eq!(PlantKind::Coal.output(1, 0), 3000);
        assert_eq!(PlantKind::Coal.output(1, 3 * 3600), 3000);
    }
}
