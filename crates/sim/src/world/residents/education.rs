//! Education levels and who may fill which job (ADR 0055). Draws come from their own stream,
//! so education never shifts other residents' draws.
use super::super::World;
use crate::citizen::{CitizenId, Education};
use crate::zone::Zone;

/// Percent of arriving adults with each level: none, primary, high school, university.
pub const ARRIVING_MIX: [u32; 4] = [0, 60, 30, 10];
/// Sim seconds a job seeker waits before taking a job that asks for more education than
/// they have: a quarter to half a clock day.
const ANY_JOB_WAIT_S: (f32, f32) = (1_200.0, 2_400.0);

/// The education a job in a building of this use prefers. Low-density work needs none;
/// dense industry prefers primary; dense commerce and teaching prefer high school.
pub fn job_education(zone: Zone) -> Education {
    match zone {
        Zone::IndustrialHigh => Education::Primary,
        Zone::CommercialHigh | Zone::PrimarySchool | Zone::HighSchool => Education::HighSchool,
        _ => Education::None,
    }
}

impl World {
    /// An arriving adult's level, drawn from `ARRIVING_MIX`.
    pub(in crate::world) fn arriving_education(&mut self) -> Education {
        let mut draw = self.residents.education_rng.below(100);
        for (level, &percent) in [
            Education::None,
            Education::Primary,
            Education::HighSchool,
            Education::University,
        ]
        .iter()
        .zip(&ARRIVING_MIX)
        {
            if draw < percent {
                return *level;
            }
            draw -= percent;
        }
        unreachable!("ARRIVING_MIX adds up to 100")
    }

    /// Starts the wait before an unemployed citizen may take any job, unless one is running.
    pub(in crate::world) fn start_seeking(&mut self, c: CitizenId) {
        let i = c as usize;
        if self.citizens.job[i].is_some() || self.citizens.any_job_at[i] != u64::MAX {
            return;
        }
        let (lo, hi) = ANY_JOB_WAIT_S;
        let wait = self.residents.education_rng.range(lo, hi);
        self.citizens.any_job_at[i] = self.totals.tick + self.secs_to_ticks(wait);
    }

    /// Whether `c` may take a job in a building of this use now: at once with enough
    /// education, otherwise once their wait is over.
    pub(in crate::world) fn qualifies(&self, c: CitizenId, zone: Zone) -> bool {
        let i = c as usize;
        self.citizens.education[i] >= job_education(zone)
            || self.totals.tick >= self.citizens.any_job_at[i]
    }
}
