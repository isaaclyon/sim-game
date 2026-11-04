//! Reads the command log text format described in `replay/mod.rs`.

use super::{CLASSES, CommandLog, LOG_VERSION, LogError, Outcome, PLANTS, Start, TITLE, ZONES};
use crate::Command;

struct Line<'a> {
    number: usize,
    words: std::str::SplitWhitespace<'a>,
}

impl<'a> Line<'a> {
    fn fail<T>(&self, message: impl Into<String>) -> Result<T, LogError> {
        Err(LogError {
            line: self.number,
            message: message.into(),
        })
    }

    fn word(&mut self, what: &str) -> Result<&'a str, LogError> {
        match self.words.next() {
            Some(w) => Ok(w),
            None => self.fail(format!("missing {what}")),
        }
    }

    fn keyword(&mut self, expected: &str) -> Result<(), LogError> {
        let w = self.word(expected)?;
        if w == expected {
            Ok(())
        } else {
            self.fail(format!("expected `{expected}`, found `{w}`"))
        }
    }

    fn number<T: std::str::FromStr>(&mut self, what: &str) -> Result<T, LogError> {
        let w = self.word(what)?;
        match w.parse() {
            Ok(v) => Ok(v),
            Err(_) => self.fail(format!("`{w}` is not a valid {what}")),
        }
    }

    fn coords<const N: usize>(&mut self) -> Result<[f32; N], LogError> {
        let mut out = [0.0; N];
        for v in &mut out {
            *v = self.number("coordinate")?;
        }
        Ok(out)
    }

    fn named<T: Copy>(&mut self, table: &[(T, &str)], what: &str) -> Result<T, LogError> {
        let w = self.word(what)?;
        match table.iter().find(|(_, n)| *n == w) {
            Some((v, _)) => Ok(*v),
            None => self.fail(format!("unknown {what} `{w}`")),
        }
    }

    fn done(mut self) -> Result<(), LogError> {
        match self.words.next() {
            Some(w) => self.fail(format!("unexpected `{w}`")),
            None => Ok(()),
        }
    }

    fn command(&mut self) -> Result<Command, LogError> {
        let kind = self.word("command")?;
        Ok(match kind {
            "spawn" => Command::SpawnAgents {
                count: self.number("count")?,
            },
            "road" => {
                let [ax, ay, bx, by] = self.coords()?;
                let class = self.named(&CLASSES, "road class")?;
                Command::BuildRoad {
                    ax,
                    ay,
                    bx,
                    by,
                    class,
                }
            }
            "curve" => {
                let [ax, ay, cx, cy, bx, by] = self.coords()?;
                let class = self.named(&CLASSES, "road class")?;
                Command::BuildCurve {
                    ax,
                    ay,
                    cx,
                    cy,
                    bx,
                    by,
                    class,
                }
            }
            "delete" => {
                let [x, y] = self.coords()?;
                Command::DeleteRoad { x, y }
            }
            "paint" => {
                let [ax, ay, bx, by] = self.coords()?;
                let zone = self.named(&ZONES, "zone")?;
                Command::PaintZone {
                    ax,
                    ay,
                    bx,
                    by,
                    zone,
                }
            }
            "fill" => {
                let [x, y] = self.coords()?;
                let zone = self.named(&ZONES, "zone")?;
                Command::FillBlock { x, y, zone }
            }
            "plant" => {
                let [x, y] = self.coords()?;
                let kind = self.named(&PLANTS, "plant kind")?;
                Command::PlacePlant { kind, x, y }
            }
            "line" => {
                let [ax, ay, bx, by] = self.coords()?;
                Command::BuildLine { ax, ay, bx, by }
            }
            other => return self.fail(format!("unknown command `{other}`")),
        })
    }
}

pub(super) fn parse(text: &str) -> Result<CommandLog, LogError> {
    let mut lines = text
        .lines()
        .enumerate()
        .map(|(i, l)| (i + 1, l.trim()))
        .filter(|(_, l)| !l.is_empty() && !l.starts_with('#'))
        .map(|(number, l)| Line {
            number,
            words: l.split_whitespace(),
        });
    let last = text.lines().count();
    let mut next = |what: &str| {
        lines.next().ok_or_else(|| LogError {
            line: last,
            message: format!("missing {what} line"),
        })
    };

    let mut title = next("title")?;
    for word in TITLE.split_whitespace() {
        title.keyword(word)?;
    }
    let version: u32 = title.number("version")?;
    if version != LOG_VERSION {
        return title.fail(format!(
            "unsupported command log version {version}; this build reads version {LOG_VERSION}"
        ));
    }
    title.done()?;

    let mut seed_line = next("seed")?;
    seed_line.keyword("seed")?;
    let seed = seed_line.number("seed")?;
    seed_line.done()?;
    let mut grid = next("map")?;
    let start = match grid.word("map")? {
        "grid" => Start::Grid,
        "map" => Start::Map,
        other => return grid.fail(format!("expected `map` or `grid`, found `{other}`")),
    };
    let (cols, rows) = (grid.number("column count")?, grid.number("row count")?);
    grid.done()?;

    let mut log = CommandLog::new(seed, cols, rows);
    log.start = start;
    let mut previous = 0;
    loop {
        let mut line = next("end")?;
        let first = line.word("tick")?;
        if first == "end" {
            log.end = line.number("end tick")?;
            if log.end < previous {
                return line.fail("`end` comes before the last command");
            }
            line.done()?;
            break;
        }
        let Ok(tick) = first.parse::<u64>() else {
            return line.fail(format!("expected a tick or `end`, found `{first}`"));
        };
        if tick < previous {
            return line.fail("ticks must not decrease");
        }
        previous = tick;
        let cmd = line.command()?;
        line.done()?;
        log.commands.push((tick, cmd));
    }

    if let Some(mut line) = lines.next() {
        line.keyword("expect")?;
        line.keyword("hash")?;
        let hex = line.word("hash")?;
        let Ok(hash) = u64::from_str_radix(hex, 16) else {
            return line.fail(format!("`{hex}` is not a hexadecimal hash"));
        };
        line.keyword("population")?;
        let population = line.number("population")?;
        line.keyword("buildings")?;
        let buildings = line.number("building count")?;
        line.keyword("trips")?;
        let trips = line.number("trip count")?;
        line.done()?;
        log.expect = Some(Outcome {
            hash,
            population,
            buildings,
            trips,
        });
    }
    if let Some(line) = lines.next() {
        return line.fail("unexpected line after `expect`");
    }
    Ok(log)
}
