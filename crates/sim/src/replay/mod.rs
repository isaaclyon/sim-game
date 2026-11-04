//! Command logs (ADR 0034): a fresh world's seed and grid, then every player command with the
//! tick it was applied at. Replaying a log on the same build and target rebuilds the same city,
//! which is what the replay regression tests in `tests/replay` check.
//!
//! The text format, one item per line; `#` starts a comment line, blank lines are ignored:
//!
//! ```text
//! sim-game command log 1
//! seed 1
//! grid 12 12
//! 0 spawn 100
//! 120 road 10.5 20 300 20 street
//! 130 curve 0 0 50 50 100 0 main
//! 140 delete 60 20
//! 150 paint 0 0 100 100 residential-low
//! 160 fill 50 50 industrial-high
//! 170 plant 400 40 coal
//! 180 line 400 40 600 40
//! end 3000
//! expect hash 0123456789abcdef population 12 buildings 30 trips 40
//! ```
//!
//! Command lines start with their tick, which never decreases and never passes `end`.
//! Coordinates are `f32` written in their shortest exact form, so they read back bit for bit.
//! `expect` is optional: what the replay must reach at `end`.

mod parse;

use std::fmt::{self, Write};

use crate::power::PlantKind;
use crate::road::RoadClass;
use crate::{Command, Config, World, Zone};

/// Bump when a log of the previous version would read differently.
pub const LOG_VERSION: u32 = 1;
const TITLE: &str = "sim-game command log";
const CLASSES: [(RoadClass, &str); 3] = [
    (RoadClass::Street, "street"),
    (RoadClass::Main, "main"),
    (RoadClass::Alley, "alley"),
];
const PLANTS: [(PlantKind, &str); 3] = [
    (PlantKind::Coal, "coal"),
    (PlantKind::Solar, "solar"),
    (PlantKind::Wind, "wind"),
];
const ZONES: [(Zone, &str); 7] = [
    (Zone::None, "none"),
    (Zone::ResidentialLow, "residential-low"),
    (Zone::ResidentialHigh, "residential-high"),
    (Zone::CommercialLow, "commercial-low"),
    (Zone::CommercialHigh, "commercial-high"),
    (Zone::IndustrialLow, "industrial-low"),
    (Zone::IndustrialHigh, "industrial-high"),
];

/// A session from a fresh generated world, as replayable commands.
#[derive(Clone, Debug, PartialEq)]
pub struct CommandLog {
    pub seed: u64,
    pub cols: u32,
    pub rows: u32,
    /// Every command in the order it was applied, with the tick count at that moment.
    /// Refused commands are kept: replaying them must refuse them again.
    pub commands: Vec<(u64, Command)>,
    /// The tick a replay runs to.
    pub end: u64,
    pub expect: Option<Outcome>,
}

/// What a replay reached, compared against a log's `expect` line.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Outcome {
    pub hash: u64,
    pub population: u32,
    pub buildings: u32,
    pub trips: u64,
}

impl Outcome {
    pub fn of(world: &World) -> Self {
        let stats = world.stats();
        Self {
            hash: world.state_hash(),
            population: stats.population,
            buildings: world.buildings.active_buildings().count() as u32,
            trips: stats.trips_completed,
        }
    }
}

/// Why a log could not be read, with its 1-based line number.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LogError {
    pub line: usize,
    pub message: String,
}

impl fmt::Display for LogError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "line {}: {}", self.line, self.message)
    }
}

impl std::error::Error for LogError {}

impl CommandLog {
    pub fn new(seed: u64, cols: u32, rows: u32) -> Self {
        Self {
            seed,
            cols,
            rows,
            commands: Vec::new(),
            end: 0,
            expect: None,
        }
    }

    /// The fresh world this log starts from: the game's default configuration and this seed.
    pub fn world(&self) -> World {
        let cfg = Config {
            seed: self.seed,
            ..Config::default()
        };
        World::generate(cfg, self.cols, self.rows)
    }

    /// Applies `cmd` to `world` and records it at the world's current tick.
    pub fn apply(&mut self, world: &mut World, cmd: Command) -> bool {
        let tick = world.tick_count();
        self.commands.push((tick, cmd.clone()));
        self.end = tick;
        world.apply(cmd)
    }

    /// Runs `world` up to tick `until`, applying logged commands from index `next` as their
    /// ticks come up, including those at `until`. Returns the index of the next command.
    pub fn advance(&self, world: &mut World, mut next: usize, until: u64) -> usize {
        loop {
            while let Some((tick, cmd)) = self.commands.get(next) {
                if *tick != world.tick_count() {
                    break;
                }
                world.apply(cmd.clone());
                next += 1;
            }
            if world.tick_count() >= until {
                return next;
            }
            world.tick();
        }
    }

    /// The world this log ends in.
    pub fn replay(&self) -> World {
        let mut world = self.world();
        self.advance(&mut world, 0, self.end);
        world
    }

    pub fn parse(text: &str) -> Result<Self, LogError> {
        parse::parse(text)
    }
}

/// Written as the format in the module docs; `parse` reads it back unchanged.
impl fmt::Display for CommandLog {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "{TITLE} {LOG_VERSION}")?;
        writeln!(f, "seed {}", self.seed)?;
        writeln!(f, "grid {} {}", self.cols, self.rows)?;
        for (tick, cmd) in &self.commands {
            writeln!(f, "{tick} {}", command_text(cmd))?;
        }
        writeln!(f, "end {}", self.end)?;
        if let Some(e) = self.expect {
            writeln!(
                f,
                "expect hash {:016x} population {} buildings {} trips {}",
                e.hash, e.population, e.buildings, e.trips
            )?;
        }
        Ok(())
    }
}

fn name<T: PartialEq + Copy>(table: &[(T, &'static str)], value: T) -> &'static str {
    table
        .iter()
        .find(|(v, _)| *v == value)
        .map(|(_, n)| *n)
        .expect("every value is named")
}

fn command_text(cmd: &Command) -> String {
    let mut s = String::new();
    let mut nums = |word: &str, values: &[f32]| {
        s.push_str(word);
        for v in values {
            write!(s, " {v}").expect("writing to a String");
        }
    };
    match *cmd {
        Command::SpawnAgents { count } => format!("spawn {count}"),
        Command::BuildRoad {
            ax,
            ay,
            bx,
            by,
            class,
        } => {
            nums("road", &[ax, ay, bx, by]);
            s + " " + name(&CLASSES, class)
        }
        Command::BuildCurve {
            ax,
            ay,
            cx,
            cy,
            bx,
            by,
            class,
        } => {
            nums("curve", &[ax, ay, cx, cy, bx, by]);
            s + " " + name(&CLASSES, class)
        }
        Command::DeleteRoad { x, y } => {
            nums("delete", &[x, y]);
            s
        }
        Command::PaintZone {
            ax,
            ay,
            bx,
            by,
            zone,
        } => {
            nums("paint", &[ax, ay, bx, by]);
            s + " " + name(&ZONES, zone)
        }
        Command::FillBlock { x, y, zone } => {
            nums("fill", &[x, y]);
            s + " " + name(&ZONES, zone)
        }
        Command::PlacePlant { kind, x, y } => {
            nums("plant", &[x, y]);
            s + " " + name(&PLANTS, kind)
        }
        Command::BuildLine { ax, ay, bx, by } => {
            nums("line", &[ax, ay, bx, by]);
            s
        }
    }
}

#[cfg(test)]
mod tests;
