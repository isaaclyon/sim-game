//! Money (ADR 0036): the city's balance, what roads and utilities cost to build and keep, the
//! taxes residents and filled jobs pay, and a flat grant every city receives. Power plants,
//! pumps and towers cost money since ADR 0052; power lines stay free.
//!
//! Amounts are whole dollars in `i64`. Rates are per clock hour and settle every clock minute;
//! the fraction of a dollar left over carries to the next minute, so nothing is lost to
//! rounding. Building a road the city cannot afford is refused. Upkeep keeps charging when
//! the balance is negative; debt only stops edits that cost money.

use serde::{Deserialize, Serialize};

use super::{Command, World};
use crate::clock::clock_seconds;
use crate::era::Era;
use crate::geometry::{Shape, dist};
use crate::power::{PlantKind, Power};
use crate::road::{RoadClass, RoadNetwork};
use crate::snap::{Join, Plan};
use crate::water::{Water, WaterKind};

/// What a fresh city starts with.
pub const START_FUNDS: i64 = 150_000;
/// Tax each resident pays per clock hour.
pub const RESIDENT_TAX: i64 = 2;
/// Tax each filled job pays per clock hour, on top of its worker's resident tax.
pub const JOB_TAX: i64 = 1;
/// Grant every city receives per clock hour, whatever its size. It pays for the
/// first roads before taxes can, and covers the upkeep of about 20 km of street.
pub const GRANT: i64 = 2_000;

/// What a village pays for its roads, as a share of the town's rates: unpaved tracks cost half
/// to build and to keep. The town's arrival raises upkeep to the town rate (ADR 0059).
const VILLAGE_ROAD_SHARE: f64 = 0.5;

/// The share of the town's road rates paid in `era`.
fn era_share(era: Era) -> f64 {
    match era {
        Era::Village => VILLAGE_ROAD_SHARE,
        Era::Town => 1.0,
    }
}

/// Dollars per metre of new road in `era`.
fn build_rate(class: RoadClass, era: Era) -> f64 {
    era_share(era)
        * match class {
            RoadClass::Street => 10.0,
            RoadClass::Main => 25.0,
            RoadClass::Alley => 5.0,
        }
}

/// Dollars per metre of road per clock hour in `era`.
fn upkeep_rate(class: RoadClass, era: Era) -> f64 {
    era_share(era)
        * match class {
            RoadClass::Street => 0.10,
            RoadClass::Main => 0.25,
            RoadClass::Alley => 0.05,
        }
}

/// What a power plant costs to build, and its upkeep per clock hour (ADR 0052).
pub fn plant_price(kind: PlantKind) -> (i64, i64) {
    match kind {
        PlantKind::Coal => (20_000, 120),
        PlantKind::Solar => (12_000, 40),
        PlantKind::Wind => (4_000, 15),
    }
}

/// What a pump or water tower costs to build, and its upkeep per clock hour (ADR 0052).
pub fn water_price(kind: WaterKind) -> (i64, i64) {
    match kind {
        WaterKind::Pump => (6_000, 40),
        WaterKind::Tower => (4_000, 20),
    }
}

/// Build cost and upkeep per clock hour of each placeable the build menu sells, in this
/// order: coal, solar and wind plants, pump, water tower, fire station. Lines are free.
pub fn price_list() -> [(i64, i64); 6] {
    use super::fire::{STATION_COST, STATION_UPKEEP};
    [
        plant_price(PlantKind::Coal),
        plant_price(PlantKind::Solar),
        plant_price(PlantKind::Wind),
        water_price(WaterKind::Pump),
        water_price(WaterKind::Tower),
        (STATION_COST, STATION_UPKEEP),
    ]
}

/// Upkeep per clock hour of every standing plant, pump and tower.
fn utility_upkeep(power: &Power, water: &Water) -> i64 {
    let plants: i64 = power
        .active_plants()
        .map(|p| plant_price(power.plant_kind[p as usize]).1)
        .sum();
    let pieces: i64 = water
        .active_pieces()
        .map(|p| water_price(water.kind[p as usize]).1)
        .sum();
    plants + pieces
}

/// The city's money as the HUD shows it. Rates are dollars per clock hour.
#[derive(Clone, Copy, Default, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Money {
    pub balance: i64,
    pub taxes: i64,
    pub grant: i64,
    /// Road and fire station upkeep (ADR 0053).
    pub upkeep: i64,
    /// Upkeep of plants, pumps and towers.
    pub utilities: i64,
}

#[derive(Serialize, Deserialize)]
pub(super) struct Treasury {
    pub(super) balance: i64,
    /// Dollar-minutes not yet settled as a whole dollar, `0..60`.
    pub(super) carry: i64,
    /// Road and fire station upkeep per clock hour; derived from them, refreshed whenever
    /// they change.
    #[serde(skip)]
    pub(super) upkeep: i64,
    /// Utility upkeep per clock hour; derived like `upkeep` from plants, pumps and towers.
    #[serde(skip)]
    pub(super) utilities: i64,
}

impl Default for Treasury {
    fn default() -> Self {
        Self {
            balance: START_FUNDS,
            carry: 0,
            upkeep: 0,
            utilities: 0,
        }
    }
}

/// Upkeep per clock hour of every active road in `era`.
pub(super) fn road_upkeep(roads: &RoadNetwork, era: Era) -> i64 {
    let total: f64 = roads
        .active_roads()
        .map(|r| f64::from(roads.lane_len[2 * r as usize]) * upkeep_rate(roads.road_class(r), era))
        .sum();
    total.round() as i64
}

fn shape_len(shape: Shape) -> f32 {
    match shape.control {
        Some(_) => shape.lengths().last().copied().unwrap_or(0.0),
        None => dist(shape.a, shape.b),
    }
}

/// What building `plan` as `class` costs: the length of its new legs, plus the difference in
/// build rate along followed roads it moves to a dearer class (ADR 0048). Other followed
/// roads, and moves to a cheaper class, cost nothing. Rates are `era`'s.
pub(super) fn plan_cost(roads: &RoadNetwork, plan: &Plan, class: RoadClass, era: Era) -> i64 {
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
    let upgrades: f64 = plan
        .reclass
        .iter()
        .map(|&r| {
            let extra = build_rate(class, era) - build_rate(roads.road_class(r), era);
            f64::from(roads.lane_len[2 * r as usize]) * extra.max(0.0)
        })
        .sum();
    (metres * build_rate(class, era) + upgrades).round() as i64
}

impl World {
    pub fn money(&self) -> Money {
        Money {
            balance: self.treasury.balance,
            taxes: self.residents.population as i64 * RESIDENT_TAX
                + self.residents.employed as i64 * JOB_TAX,
            grant: GRANT,
            upkeep: self.treasury.upkeep,
            utilities: self.treasury.utilities,
        }
    }

    /// Adds `dollars` to the balance outside the economy, for benchmarks that draw large
    /// layouts through `Command`s. Not a `Command`, so players, saves and replays cannot reach
    /// it.
    #[doc(hidden)]
    pub fn add_funds_for_bench(&mut self, dollars: i64) {
        self.treasury.balance = self.treasury.balance.saturating_add(dollars);
    }

    /// Whether the city can pay `cost` now. Free edits, such as moving a road to a cheaper
    /// class (ADR 0048), are always affordable; a city in debt can afford nothing else.
    pub fn can_afford(&self, cost: i64) -> bool {
        cost <= 0 || cost <= self.treasury.balance
    }

    /// Starts a placement preview: `ok` (it fits and the city can pay `cost`, so a click would
    /// place it), then `affordable`, each 1 or 0.
    pub(in crate::world) fn push_placement(&self, fits: bool, cost: i64, out: &mut Vec<f32>) {
        let affordable = self.can_afford(cost);
        out.extend([fits && affordable, affordable].map(|b| f32::from(u8::from(b))));
    }

    /// What building the road through guided raw `[x, y, snap]` points (ADR 0038) would cost,
    /// or None if it would be refused for its shape or era (ADR 0059) or the points are not a
    /// road.
    pub fn draft_cost(&self, raw: &[f32], class: RoadClass) -> Option<i64> {
        if !self.allows_road(class) {
            return None;
        }
        match self.draft_command(raw, class)? {
            Command::BuildRoad { ax, ay, bx, by, .. } => self.road_cost(ax, ay, bx, by, class),
            Command::BuildCurve {
                ax,
                ay,
                cx,
                cy,
                bx,
                by,
                ..
            } => self.curve_cost(ax, ay, cx, cy, bx, by, class),
            _ => None,
        }
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

    /// What `SetRoadClass` at this point would cost (ADR 0050), or None if it would change
    /// nothing. Changes nothing.
    pub fn class_cost(&self, x: f32, y: f32, class: RoadClass) -> Option<i64> {
        self.class_plan(x, y, class)
            .map(|p| plan_cost(&self.roads, &p, class, self.era))
    }

    fn shape_cost(&self, shape: Shape, class: RoadClass) -> Option<i64> {
        crate::snap::plan(&self.roads, shape, Some(class))
            .map(|p| plan_cost(&self.roads, &p, class, self.era))
    }

    pub(super) fn refresh_upkeep(&mut self) {
        self.treasury.upkeep = road_upkeep(&self.roads, self.era) + self.fire_upkeep();
        self.treasury.utilities = utility_upkeep(&self.power, &self.water);
    }

    /// Pays taxes, the grant and upkeep for the clock minutes the last tick finished.
    pub(super) fn settle_money(&mut self) {
        let tick = self.totals.tick;
        let minute = |t: u64| clock_seconds(t, self.cfg.dt) / 60;
        let minutes = (minute(tick) - minute(tick.saturating_sub(1))) as i64;
        if minutes == 0 {
            return;
        }
        let m = self.money();
        let t = &mut self.treasury;
        let owed = (m.taxes + m.grant - m.upkeep - m.utilities) * minutes + t.carry;
        t.balance += owed.div_euclid(60);
        t.carry = owed.rem_euclid(60);
    }
}
