//! Pumps and water towers (ADR 0052). Only what the player placed, tower levels, the last
//! update step and when each dense building went dry are stored here; networks, pump output
//! and which buildings get water are derived (`world::water`). Pipes follow roads, so there is
//! no pipe to store.

use serde::{Deserialize, Serialize};

use crate::power::Footprint;

pub type PieceId = u32;

/// What a pump gives on clean ground each update, in the units of a building's water use.
pub const PUMP_OUTPUT: u32 = 2_000;
/// What a tower holds when full: units of water, one unit serves one unit of use for one update.
pub const TOWER_CAPACITY: u32 = 20_000;
/// Clock seconds between water updates: towers fill or drain and dry buildings lose people.
pub const STEP_S: u64 = 600;
/// A dry dense building keeps everyone this long, clock seconds.
pub const DRY_GRACE_S: u64 = 86_400;
/// After the grace period, one household or job leaves a dry building this often, clock seconds.
pub const DRY_INTERVAL_S: u64 = 3 * 3600;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[repr(u8)]
pub enum WaterKind {
    /// Draws water; gives less on polluted ground.
    Pump = 0,
    /// Stores spare water and drains it into shortfalls.
    Tower = 1,
}

impl WaterKind {
    pub fn from_u8(v: u8) -> Option<Self> {
        [Self::Pump, Self::Tower].get(v as usize).copied()
    }

    /// Half the side of the square footprint, metres.
    pub fn half_size(self) -> f32 {
        match self {
            Self::Pump => 7.0,
            Self::Tower => 5.0,
        }
    }
}

/// A pump's output with `ground` the mean ground pollution under it, 0 (clean) to
/// `u16::MAX` (full): linear from full output down to a quarter. Integer math.
pub fn pump_output(ground: u16) -> u32 {
    let full = u64::from(PUMP_OUTPUT);
    let lost = full * 3 * u64::from(ground) / (4 * u64::from(u16::MAX));
    (full - lost) as u32
}

/// Pumps and towers the player placed. IDs are indices and append-only; removal tombstones.
#[derive(Default, Serialize, Deserialize)]
pub struct Water {
    pub kind: Vec<WaterKind>,
    pub x: Vec<f32>,
    pub y: Vec<f32>,
    /// Unit direction the piece's front faces: toward its road.
    pub fx: Vec<f32>,
    pub fy: Vec<f32>,
    pub active: Vec<bool>,
    /// Water stored per piece; always 0 for pumps.
    pub level: Vec<u32>,
    /// Clock step (`clock_seconds / STEP_S`) of the last update.
    pub step: u64,
    /// Per building ID, the tick it last went dry; `u64::MAX` while it has water or ignores it.
    pub dry_since: Vec<u64>,
    /// Bumped whenever pieces are placed or removed.
    pub version: u32,
}

impl Water {
    pub fn active_pieces(&self) -> impl Iterator<Item = PieceId> + '_ {
        let on = &self.active;
        (0..on.len()).filter(|&p| on[p]).map(|p| p as PieceId)
    }

    pub fn footprint(&self, p: PieceId) -> Footprint {
        let i = p as usize;
        Footprint {
            centre: (self.x[i], self.y[i]),
            front: (self.fx[i], self.fy[i]),
            half: self.kind[i].half_size(),
        }
    }

    /// Footprints by piece ID; `None` for removed pieces.
    pub(crate) fn footprints(&self) -> Vec<Option<Footprint>> {
        (0..self.active.len())
            .map(|p| self.active[p].then(|| self.footprint(p as PieceId)))
            .collect()
    }

    /// The active piece whose footprint, grown by `margin`, contains the point.
    pub fn piece_at(&self, x: f32, y: f32, margin: f32) -> Option<PieceId> {
        self.active_pieces()
            .find(|&p| self.footprint(p).contains((x, y), margin))
    }

    pub(crate) fn add(&mut self, kind: WaterKind, at: Footprint) {
        self.kind.push(kind);
        self.x.push(at.centre.0);
        self.y.push(at.centre.1);
        self.fx.push(at.front.0);
        self.fy.push(at.front.1);
        self.active.push(true);
        self.level.push(0);
        self.version = self.version.wrapping_add(1);
    }

    /// Removes a piece; a tower's stored water goes with it.
    pub(crate) fn remove(&mut self, p: PieceId) {
        self.active[p as usize] = false;
        self.level[p as usize] = 0;
        self.version = self.version.wrapping_add(1);
    }

    pub(crate) fn hash_into(&self, mix: &mut impl FnMut(u64)) {
        mix(self.active.len() as u64);
        for p in 0..self.active.len() {
            mix(self.active[p] as u64 | (self.kind[p] as u64) << 1);
            mix(u64::from(self.x[p].to_bits()) << 32 | u64::from(self.y[p].to_bits()));
            mix(u64::from(self.fx[p].to_bits()) << 32 | u64::from(self.fy[p].to_bits()));
            mix(u64::from(self.level[p]));
        }
        mix(self.step);
        mix(self.dry_since.len() as u64);
        for &t in &self.dry_since {
            mix(t);
        }
    }

    /// Internal consistency of loaded water (ADR 0033).
    pub(crate) fn check(&self, buildings: usize) -> crate::check::Check {
        use crate::check::{ensure, finite, lens};
        let n = self.active.len();
        let fields = [
            self.kind.len(),
            self.x.len(),
            self.y.len(),
            self.fx.len(),
            self.fy.len(),
            self.level.len(),
        ];
        lens(&fields, n, "water piece fields")?;
        finite(self.x.iter().chain(&self.y), "water piece positions")?;
        let unit = |(x, y): (&f32, &f32)| ((x * x + y * y) - 1.0).abs() < 1e-3;
        ensure(self.fx.iter().zip(&self.fy).all(unit), "water piece fronts")?;
        let full = |p: usize| match self.kind[p] {
            WaterKind::Pump => self.level[p] == 0,
            WaterKind::Tower => self.level[p] <= TOWER_CAPACITY,
        };
        ensure((0..n).all(full), "tower levels")?;
        ensure(self.dry_since.len() <= buildings, "dry buildings")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pump_output_falls_linearly_to_a_quarter() {
        assert_eq!(pump_output(0), PUMP_OUTPUT);
        assert_eq!(pump_output(u16::MAX), PUMP_OUTPUT / 4);
        let half = pump_output(u16::MAX / 2 + 1);
        assert!(half.abs_diff(PUMP_OUTPUT * 5 / 8) <= 1, "{half}");
        assert!(pump_output(10_000) > pump_output(20_000));
    }
}
