//! Day clock, derived from the tick count so it has no state of its own to drift or desync.

/// Clock seconds that pass per sim second: a day lasts 4 sim hours, so 4 real hours at 1x and
/// 24 minutes at 10x. Traffic still runs in sim seconds; only the displayed clock is faster.
pub const CLOCK_SECONDS_PER_SIM_SECOND: u64 = 6;
/// The clock reads Day 1, 06:00 at tick 0.
pub const START_SECOND_OF_DAY: u64 = 6 * 3600;
pub const SECONDS_PER_DAY: u64 = 24 * 3600;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ClockTime {
    /// 1-based day number.
    pub day: u32,
    /// Seconds since midnight, `0..SECONDS_PER_DAY`.
    pub second_of_day: u32,
}

/// The clock reading after `tick` ticks of `dt` sim seconds each.
///
/// `dt` is rounded to whole milliseconds once and the rest is integer math, so long runs don't
/// pick up float rounding drift.
pub fn clock_at(tick: u64, dt: f32) -> ClockTime {
    let dt_ms = (f64::from(dt) * 1000.0).round() as u64;
    let clock_secs = tick * dt_ms * CLOCK_SECONDS_PER_SIM_SECOND / 1000 + START_SECOND_OF_DAY;
    ClockTime {
        day: (clock_secs / SECONDS_PER_DAY + 1) as u32,
        second_of_day: (clock_secs % SECONDS_PER_DAY) as u32,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Ticks of `dt` that make up `clock_secs` of clock time.
    fn ticks_for(clock_secs: u64, dt: f32) -> u64 {
        let dt_ms = (f64::from(dt) * 1000.0).round() as u64;
        clock_secs * 1000 / CLOCK_SECONDS_PER_SIM_SECOND / dt_ms
    }

    #[test]
    fn starts_on_day_one_at_six() {
        assert_eq!(
            clock_at(0, 0.1),
            ClockTime {
                day: 1,
                second_of_day: 6 * 3600
            }
        );
    }

    #[test]
    fn rolls_over_at_midnight() {
        let to_midnight = ticks_for(SECONDS_PER_DAY - START_SECOND_OF_DAY, 0.1);
        let before = clock_at(to_midnight - 1, 0.1);
        assert_eq!(before.day, 1);
        assert!(before.second_of_day >= (SECONDS_PER_DAY - 60) as u32);
        assert_eq!(
            clock_at(to_midnight, 0.1),
            ClockTime {
                day: 2,
                second_of_day: 0
            }
        );
    }

    #[test]
    fn a_day_lasts_four_sim_hours() {
        assert_eq!(
            clock_at(4 * 3600 * 10, 0.1),
            ClockTime {
                day: 2,
                second_of_day: 6 * 3600
            }
        );
    }

    #[test]
    fn long_runs_do_not_drift() {
        // A year of 0.1 s ticks lands exactly on 06:00 of day 366; summing f32 dt would not.
        let year = ticks_for(365 * SECONDS_PER_DAY, 0.1);
        assert_eq!(
            clock_at(year, 0.1),
            ClockTime {
                day: 366,
                second_of_day: 6 * 3600
            }
        );
    }

    #[test]
    fn follows_dt() {
        let hour = 3600 / CLOCK_SECONDS_PER_SIM_SECOND;
        assert_eq!(clock_at(hour * 4, 0.25), clock_at(hour * 10, 0.1));
        assert_eq!(clock_at(hour * 4, 0.25).second_of_day, 7 * 3600);
    }
}
