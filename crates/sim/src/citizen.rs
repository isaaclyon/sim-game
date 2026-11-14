//! Persistent residents and occupancy, separate from temporary vehicle trips (ADR 0021).
use crate::agents::AgentId;
use crate::building::BuildingId;
use crate::clock::CLOCK_SECONDS_PER_SIM_SECOND;
use crate::rng::Rng;
use serde::{Deserialize, Serialize};

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

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Activity {
    Home,
    Work,
    Shop,
    /// A child's day at school (ADR 0055).
    School,
    /// An adult driving household children to or from school (ADR 0055).
    SchoolRun,
}

/// How far a citizen got in school (ADR 0055). Ordered: a higher level meets every lower
/// requirement.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[repr(u8)]
pub enum Education {
    #[default]
    None,
    Primary,
    HighSchool,
    University,
}

#[derive(Default, Serialize, Deserialize)]
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
    /// Tick of birth; negative for people older than the city (ADR 0054).
    pub born: Vec<i64>,
    /// Under age: never works, shops or owns a car, and leaves home only for school
    /// (ADR 0054, 0055).
    pub child: Vec<bool>,
    /// The household the citizen first belonged to; names its surname, so a grown child who
    /// moves out keeps their family name.
    pub family: Vec<HouseholdId>,
    /// Tick by which a grown child who has not found a home of their own leaves the city;
    /// `u64::MAX` for everyone else.
    pub move_by: Vec<u64>,
    /// Highest school level finished (ADR 0055).
    pub education: Vec<Education>,
    /// Tick from which this job seeker may take a job that asks for more education than they
    /// have; `u64::MAX` while not looking for work.
    pub any_job_at: Vec<u64>,
    /// Tick the current or latest trip to work set out (ADR 0058); `u64::MAX` before the first.
    pub trip_start: Vec<u64>,
    /// Clock minutes the last trip to work took, waiting for a route included; 0 = none yet
    /// or no job (ADR 0058).
    pub commute: Vec<u16>,
}
impl Citizens {
    /// A new resident of `household`, born at tick `born`. Children own no car.
    pub(crate) fn push(
        &mut self,
        household: HouseholdId,
        home: BuildingId,
        born: i64,
        child: bool,
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
            .push((6 * 3600 + rng.below(3 * 3600)) / CLOCK_SECONDS_PER_SIM_SECOND as u32);
        self.owns_car.push(false);
        self.car_out.push(false);
        self.walk_limit.push(0.0);
        self.walk_speed.push(rng.range(WALK_SPEED.0, WALK_SPEED.1));
        self.born.push(born);
        self.child.push(child);
        self.family.push(household);
        self.move_by.push(u64::MAX);
        self.education.push(Education::None);
        self.any_job_at.push(u64::MAX);
        self.trip_start.push(u64::MAX);
        self.commute.push(0);
        self.draw_car(id, !child, rng);
        id
    }

    /// Whether `c` brings a car on moving in (never without `may_own`), and the walking limit
    /// that goes with it (ADR 0026).
    pub(crate) fn draw_car(&mut self, c: CitizenId, may_own: bool, rng: &mut Rng) {
        let i = c as usize;
        let owns_car = may_own && rng.below(100) < CAR_OWNERSHIP_PERCENT;
        let (lo, hi) = if owns_car {
            OWNER_WALK_M
        } else {
            CARLESS_WALK_M
        };
        self.owns_car[i] = owns_car;
        self.walk_limit[i] = rng.range(lo, hi);
    }
}

#[derive(Default, Serialize, Deserialize)]
pub struct Households {
    pub active: Vec<bool>,
    pub home: Vec<HomeId>,
    pub members: Vec<Vec<CitizenId>>,
}
#[derive(Default, Serialize, Deserialize)]
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
