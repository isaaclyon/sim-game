//! Money (ADR 0036): the city's balance, what roads cost to build and keep, and the taxes
//! residents and filled jobs pay.
//!
//! Amounts are whole dollars in `i64`. Rates are per clock hour and settle every clock minute;
//! the fraction of a dollar left over carries to the next minute, so nothing is lost to
//! rounding. Building a road the city cannot afford is refused. Upkeep keeps charging when
//! the balance is negative; debt only stops new roads.

use serde::{Deserialize, Serialize};

use super::World;
use crate::clock::clock_seconds;
use crate::geometry::{Shape, dist};
use crate::road::{RoadClass, RoadNetwork};
use crate::snap::{Join, Plan};

/// What a fresh city starts with.
pub const START_FUNDS: i64 = 50_000;
/// Tax each resident pays per clock hour.
pub const RESIDENT_TAX: i64 = 2;
/// Tax each filled job pays per clock hour, on top of its worker's resident tax.
pub const JOB_TAX: i64 = 1;

/// Dollars per metre of new road.
fn build_rate(class: RoadClass) -> f64 {
    match class {
        RoadClass::Street => 10.0,
        RoadClass::Main => 25.0,
        RoadClass::Alley => 5.0,
    }
}

/// Dollars per metre of road per clock hour.
fn upkeep_rate(class: RoadClass) -> f64 {
    match class {
        RoadClass::Street => 0.10,
        RoadClass::Main => 0.25,
        RoadClass::Alley => 0.05,
    }
}

/// The city's money as the HUD shows it. Rates are dollars per clock hour.
#[derive(Clone, Copy, Default, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Money {
    pub balance: i64,
    pub taxes: i64,
    pub upkeep: i64,
}

#[derive(Serialize, Deserialize)]
pub(super) struct Treasury {
    pub(super) balance: i64,
    /// Dollar-minutes not yet settled as a whole dollar, `0..60`.
    pub(super) carry: i64,
    /// Road upkeep per clock hour; derived from the roads, refreshed whenever they change.
    #[serde(skip)]
    pub(super) upkeep: i64,
}

impl Default for Treasury {
    fn default() -> Self {
        Self {
            balance: START_FUNDS,
            carry: 0,
            upkeep: 0,
        }
    }
}

/// Upkeep per clock hour of every active road.
pub(super) fn road_upkeep(roads: &RoadNetwork) -> i64 {
    let total: f64 = roads
        .active_roads()
        .map(|r| f64::from(roads.lane_len[2 * r as usize]) * upkeep_rate(roads.road_class(r)))
        .sum();
    total.round() as i64
}

fn shape_len(shape: Shape) -> f32 {
    match shape.control {
        Some(_) => shape.lengths().last().copied().unwrap_or(0.0),
        None => dist(shape.a, shape.b),
    }
}

/// What building `plan` as `class` costs: the length of its new legs. Legs that follow an
/// existing road are kept as they are and cost nothing.
pub(super) fn plan_cost(roads: &RoadNetwork, plan: &Plan, class: RoadClass) -> i64 {
    let metres: f64 = plan
        .legs
        .iter()
        .zip(plan.stops.windows(2))
        .filter(|(_, ends)| match (ends[0].join, ends[1].join) {
            (Join::Node(a), Join::Node(b)) => !roads.has_road(a, b),
            _ => true,
        })
        .map(|(&leg, _)| f64::from(shape_len(leg)))
        .sum();
    (metres * build_rate(class)).round() as i64
}

impl World {
    pub fn money(&self) -> Money {
        Money {
            balance: self.treasury.balance,
            taxes: self.residents.population as i64 * RESIDENT_TAX
                + self.residents.employed as i64 * JOB_TAX,
            upkeep: self.treasury.upkeep,
        }
    }

    /// Whether the city can pay `cost` now. A city in debt can afford nothing.
    pub fn can_afford(&self, cost: i64) -> bool {
        cost <= self.treasury.balance
    }

    /// What `BuildRoad` with these arguments would cost, or None if it would be refused for
    /// its shape. Changes nothing.
    pub fn road_cost(&self, ax: f32, ay: f32, bx: f32, by: f32, class: RoadClass) -> Option<i64> {
        self.shape_cost(
            Shape {
                a: (ax, ay),
                b: (bx, by),
                control: None,
            },
            class,
        )
    }

    /// What `BuildCurve` with these arguments would cost; see `road_cost`.
    #[allow(clippy::too_many_arguments)] // Mirrors the coordinate-only WASM command boundary.
    pub fn curve_cost(
        &self,
        ax: f32,
        ay: f32,
        cx: f32,
        cy: f32,
        bx: f32,
        by: f32,
        class: RoadClass,
    ) -> Option<i64> {
        self.shape_cost(
            Shape {
                a: (ax, ay),
                b: (bx, by),
                control: Some((cx, cy)),
            },
            class,
        )
    }

    fn shape_cost(&self, shape: Shape, class: RoadClass) -> Option<i64> {
        crate::snap::plan(&self.roads, shape).map(|p| plan_cost(&self.roads, &p, class))
    }

    pub(super) fn refresh_upkeep(&mut self) {
        self.treasury.upkeep = road_upkeep(&self.roads);
    }

    /// Pays taxes and upkeep for the clock minutes the last tick finished.
    pub(super) fn settle_money(&mut self) {
        let tick = self.totals.tick;
        let minute = |t: u64| clock_seconds(t, self.cfg.dt) / 60;
        let minutes = (minute(tick) - minute(tick.saturating_sub(1))) as i64;
        if minutes == 0 {
            return;
        }
        let m = self.money();
        let t = &mut self.treasury;
        let owed = (m.taxes - m.upkeep) * minutes + t.carry;
        t.balance += owed.div_euclid(60);
        t.carry = owed.rem_euclid(60);
    }
}
