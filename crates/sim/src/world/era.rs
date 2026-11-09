//! The city's era (ADR 0059): what it unlocks, and when the village becomes a town.

use super::{Command, World};
use crate::era::{Era, TOWN_POPULATION};
use crate::road::RoadClass;
use crate::zone::Zone;

impl World {
    /// The city's era now.
    pub fn era(&self) -> Era {
        self.era
    }

    /// The same fresh world, started in `era`: for tests and scenarios that begin past the
    /// village. Play only ever moves a city on by unlocking.
    pub fn with_era(mut self, era: Era) -> Self {
        self.era = era;
        self
    }

    /// Whether the city's era allows building roads of `class`.
    pub fn allows_road(&self, class: RoadClass) -> bool {
        Era::of_road(class) <= self.era
    }

    /// Whether the city's era allows painting `zone`.
    pub fn allows_zone(&self, zone: Zone) -> bool {
        Era::of_zone(zone) <= self.era
    }

    /// False for a command that builds something the city's era has not unlocked. Undo and
    /// redo pass: history only holds edits made in an era already reached.
    pub(super) fn allows(&self, cmd: &Command) -> bool {
        match *cmd {
            Command::BuildRoad { class, .. }
            | Command::BuildCurve { class, .. }
            | Command::BuildGrid { class, .. }
            | Command::SetRoadClass { class, .. } => self.allows_road(class),
            Command::PaintZone { zone, .. } | Command::FillBlock { zone, .. } => {
                self.allows_zone(zone)
            }
            _ => true,
        }
    }

    /// A village with enough residents becomes a town, for good.
    pub(super) fn advance_era(&mut self) {
        if self.era == Era::Village && self.residents.population >= TOWN_POPULATION {
            self.era = Era::Town;
            // The paving bill: roads now cost the town's upkeep.
            self.refresh_upkeep();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::Config;

    #[test]
    fn a_village_becomes_a_town_at_its_population_and_stays_one() {
        let mut w = World::empty(Config::default(), 10, 10);
        let paint = Command::PaintZone {
            ax: 0.0,
            ay: 0.0,
            bx: 40.0,
            by: 40.0,
            zone: Zone::ResidentialHigh,
        };
        // The counter alone, standing in for a grown village: no citizen needs to exist.
        w.residents.population = TOWN_POPULATION - 1;
        w.tick();
        assert_eq!(w.era(), Era::Village);
        assert!(!w.apply(paint.clone()), "apartments wait for the town");

        w.residents.population = TOWN_POPULATION;
        w.tick();
        assert_eq!(w.era(), Era::Town);
        assert_eq!(w.stats().era, Era::Town);
        assert!(w.apply(paint), "the town era unlocks apartments");
    }

    #[test]
    fn road_upkeep_rises_to_the_town_rate_when_the_town_arrives() {
        let mut w = World::empty(Config::default(), 10, 10);
        assert!(w.apply(Command::BuildRoad {
            ax: 0.0,
            ay: 0.0,
            bx: 500.0,
            by: 0.0,
            class: RoadClass::Street,
        }));
        assert_eq!(
            w.money().upkeep,
            25,
            "half the town rate on 500 m of street"
        );
        w.residents.population = TOWN_POPULATION;
        w.tick();
        assert_eq!(
            w.money().upkeep,
            50,
            "paving bill: the town rate from now on"
        );

        w.residents.population = 0;
        w.tick();
        assert_eq!(w.era(), Era::Town, "eras never go back");
    }
}
