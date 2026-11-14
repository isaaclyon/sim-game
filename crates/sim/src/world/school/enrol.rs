//! Enrolment, stages and graduating (ADR 0055). Everything runs off events: nothing scans
//! children on the traffic tick unless seats or roads changed.

use std::cmp::Reverse;

use super::SchoolKind;
use crate::citizen::{CitizenId, Education};
use crate::world::World;
use crate::world::residents::family::CHILDHOOD_DAYS;

/// Stage events handled per tick at most; the rest wait for the next tick.
const PER_TICK: usize = 32;

impl World {
    /// Ticks in one school stage: half a childhood.
    fn stage_ticks(&self) -> u64 {
        self.days_to_ticks(CHILDHOOD_DAYS) / 2
    }

    /// Ticks since child `c` was born.
    fn child_age(&self, c: CitizenId) -> u64 {
        (self.totals.tick as i64 - self.citizens.born[c as usize]).max(0) as u64
    }

    /// The stage child `c` is in by age.
    pub(in crate::world) fn school_stage(&self, c: CitizenId) -> SchoolKind {
        if self.child_age(c) < self.stage_ticks() {
            SchoolKind::Primary
        } else {
            SchoolKind::High
        }
    }

    /// A child was born or arrived: they look for a seat at once. An arriving child counts
    /// as schooled elsewhere for the part of their stage already behind them.
    pub(in crate::world) fn child_joined(&mut self, c: CitizenId) {
        let i = c as usize;
        let (age, stage) = (self.child_age(c), self.stage_ticks());
        self.schools.grow(c);
        self.schools.schooled[i] = if age < stage { age } else { age - stage };
        if age < stage {
            let at = (self.totals.tick + stage - age).max(self.totals.tick + 1);
            self.schools.stages.push(Reverse((at, c)));
        }
        self.try_enrol(c);
    }

    /// Stage changes due by now, then, if seats or roads changed, another look for every
    /// child without a seat.
    pub(in crate::world) fn tick_schools(&mut self) {
        let now = self.totals.tick;
        for _ in 0..PER_TICK {
            match self.schools.stages.peek() {
                Some(&Reverse((at, c))) if at <= now => {
                    self.schools.stages.pop();
                    self.start_high_school(c);
                }
                _ => break,
            }
        }
        if std::mem::take(&mut self.schools.pending) {
            self.enrol_waiting();
        }
    }

    /// Child `c` reaches high school age: primary ends and they look for a high school.
    fn start_high_school(&mut self, c: CitizenId) {
        let i = c as usize;
        if !self.citizens.active[i] || !self.citizens.child[i] {
            return;
        }
        self.finish_stage(c, Education::Primary);
        self.try_enrol(c);
    }

    /// Child `c` comes of age: high school ends.
    pub(in crate::world) fn finish_school(&mut self, c: CitizenId) {
        self.finish_stage(c, Education::HighSchool);
    }

    /// Ends `c`'s stage. They gain `level` if they hold a seat now and were enrolled for at
    /// least half the stage, time schooled before arriving included.
    fn finish_stage(&mut self, c: CitizenId, level: Education) {
        let i = c as usize;
        let seated = self.schools.school_of(c).is_some();
        self.leave_school(c);
        if let Some(schooled) = self.schools.schooled.get_mut(i) {
            let enough = std::mem::take(schooled) * 2 >= self.days_to_ticks(CHILDHOOD_DAYS) / 2;
            if seated && enough && self.citizens.education[i] < level {
                self.citizens.education[i] = level;
            }
        }
    }

    /// Frees `c`'s seat, if they hold one, keeping the time they spent in it.
    pub(in crate::world) fn leave_school(&mut self, c: CitizenId) {
        let i = c as usize;
        let Some(s) = self.schools.seat.get_mut(i).and_then(Option::take) else {
            return;
        };
        let now = self.totals.tick;
        let schools = &mut self.schools;
        schools.schooled[i] += now.saturating_sub(schools.since[i]);
        // A seat came free at a full school: someone waiting may take it.
        if schools.pupils[s as usize] == schools.seats(s) {
            schools.pending = true;
        }
        schools.pupils[s as usize] -= 1;
    }

    /// Child `c` moved home within the city: they give up their seat and look again.
    pub(in crate::world) fn child_moved(&mut self, c: CitizenId) {
        if self.citizens.child[c as usize] {
            self.leave_school(c);
            self.try_enrol(c);
        }
    }

    /// Enrols child `c`, if they have no seat, at the nearest school of their stage by road
    /// distance whose area covers their home, which they can walk to and which has a free
    /// seat. Ties go to the lower school ID.
    pub(in crate::world) fn try_enrol(&mut self, c: CitizenId) -> bool {
        let i = c as usize;
        if !self.citizens.active[i]
            || !self.citizens.child[i]
            || self.schools.school_of(c).is_some()
        {
            return false;
        }
        let kind = self.school_stage(c);
        let home = self.home_building(c);
        let Some(node) = self.building_node(home) else {
            return false;
        };
        let reach = self.citizens.walk_limit[i];
        let mut best: Option<(f32, u32)> = None;
        for s in self.schools.active_schools() {
            let k = s as usize;
            if self.schools.kind[k] != kind || self.schools.pupils[k] >= self.schools.seats(s) {
                continue;
            }
            let Some(d) = self.school_distance(s, node) else {
                continue;
            };
            if best.is_some_and(|(old, _)| d >= old)
                || self.front_distance(home, self.schools.building[k]) > reach
            {
                continue;
            }
            best = Some((d, s));
        }
        let Some((_, s)) = best else {
            return false;
        };
        self.schools.grow(c);
        self.schools.seat[i] = Some(s);
        self.schools.since[i] = self.totals.tick;
        self.schools.pupils[s as usize] += 1;
        true
    }

    /// Every child without a seat looks again, in citizen ID order.
    fn enrol_waiting(&mut self) {
        if self.schools.active_schools().next().is_none() {
            return;
        }
        for c in 0..self.citizens.active.len() as CitizenId {
            self.try_enrol(c);
        }
    }

    /// Frees every seat at school `s`, which is closing.
    pub(super) fn release_pupils(&mut self, s: u32) {
        for c in 0..self.schools.seat.len() as CitizenId {
            if self.schools.seat[c as usize] == Some(s) {
                self.leave_school(c);
            }
        }
        self.schools.pending = true;
    }
}
