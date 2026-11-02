//! Persistent residents and occupancy, separate from temporary vehicle trips (ADR 0021).
use crate::agents::AgentId;
use crate::building::BuildingId;
use crate::rng::Rng;

pub type CitizenId = u32;
pub type HouseholdId = u32;
pub type HomeId = u32;

/// Chance, in percent, that an adult brings a car of their own when moving in (ADR 0026).
pub const CAR_OWNERSHIP_PERCENT: u32 = 85;
/// Straight-line walking limits, metres: car owners walk below theirs, and adults without a
/// car reach no further than theirs. Drawn per person.
const OWNER_WALK_M: (f32, f32) = (100.0, 400.0);
const CARLESS_WALK_M: (f32, f32) = (500.0, 1500.0);
/// Walking speeds, m/s, drawn per person.
const WALK_SPEED: (f32, f32) = (1.2, 1.6);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Activity {
    Home,
    Work,
    Shop,
}

#[derive(Default)]
pub struct Citizens {
    pub active: Vec<bool>,
    pub household: Vec<HouseholdId>,
    pub job: Vec<Option<BuildingId>>,
    /// Last successfully reached activity location; never advanced on route failure.
    pub location: Vec<BuildingId>,
    pub destination: Vec<Option<BuildingId>>,
    /// Current activity, or intended activity while a destination is set.
    pub activity: Vec<Activity>,
    pub agent: Vec<Option<AgentId>>,
    pub(crate) due: Vec<u64>,
    pub(crate) job_due: Vec<u64>,
    /// Clock seconds since midnight, stable per-person morning departure.
    pub departure: Vec<u32>,
    /// Sim seconds, counted from arrival.
    pub work_duration: Vec<u32>,
    /// Owns one car, used by nobody else (ADR 0026).
    pub owns_car: Vec<bool>,
    /// The car is out with its owner on this round trip; otherwise it is parked at home.
    /// Trips drive while it is out and walk otherwise.
    pub car_out: Vec<bool>,
    /// Straight-line walking limit, metres.
    pub walk_limit: Vec<f32>,
    /// Metres per second on foot.
    pub walk_speed: Vec<f32>,
}
impl Citizens {
    pub(crate) fn push(
        &mut self,
        household: HouseholdId,
        home: BuildingId,
        rng: &mut Rng,
    ) -> CitizenId {
        let id = self.active.len() as CitizenId;
        self.active.push(true);
        self.household.push(household);
        self.job.push(None);
        self.location.push(home);
        self.destination.push(None);
        self.activity.push(Activity::Home);
        self.agent.push(None);
        self.due.push(u64::MAX);
        self.job_due.push(u64::MAX);
        self.departure.push(7 * 3600 + rng.below(2 * 3600));
        self.work_duration
            .push((6 * 3600 + rng.below(3 * 3600)) / 6);
        let owns_car = rng.below(100) < CAR_OWNERSHIP_PERCENT;
        let (lo, hi) = if owns_car {
            OWNER_WALK_M
        } else {
            CARLESS_WALK_M
        };
        self.owns_car.push(owns_car);
        self.car_out.push(false);
        self.walk_limit.push(rng.range(lo, hi));
        self.walk_speed.push(rng.range(WALK_SPEED.0, WALK_SPEED.1));
        id
    }
}

#[derive(Default)]
pub struct Households {
    pub active: Vec<bool>,
    pub home: Vec<HomeId>,
    pub members: Vec<Vec<CitizenId>>,
}
#[derive(Default)]
pub struct Homes {
    pub building: Vec<BuildingId>,
    pub capacity: Vec<u8>,
    pub household: Vec<Option<HouseholdId>>,
}

/// Truncate the distribution, not the sampled size: small households stay common.
pub fn household_size(capacity: u8, rng: &mut Rng) -> u8 {
    let weights = [25, 35, 18, 12, 6, 3, 1];
    let count = capacity.clamp(1, 7) as usize;
    let mut draw = rng.below(weights[..count].iter().sum());
    for (i, &weight) in weights[..count].iter().enumerate() {
        if draw < weight {
            return i as u8 + 1;
        }
        draw -= weight;
    }
    unreachable!()
}
