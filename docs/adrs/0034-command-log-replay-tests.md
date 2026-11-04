# 0034. Replay regression tests from saved command logs

- Status: Accepted; implemented
- Date: 2026-11-03
- Extends: [0004](0004-deterministic-fixed-timestep-sim.md) and
  [0033](0033-single-city-state-saves.md)

## Context

The sim is deterministic: the same seed plus the same commands at the same ticks give the same
city (ADR 0004). Unit tests check single systems; nothing checks that a whole play session still
ends where it did. ADR 0033 chose full-state saves for loading, so a command log is not a save
format, but it is the smallest record of a session and the natural input for regression tests.
Fixtures checked into the repository must stay readable and reviewable in a diff.

## Decision

- A command log is versioned text owned by `sim::replay`: a title line with the log version,
  the seed and the generated grid size, then one line per player command with the tick it was
  applied at, then `end <tick>` and an optional `expect` line. Coordinates are `f32` in their
  shortest exact decimal form, so they read back bit for bit. Comment lines start with `#`.
- A log always starts from a fresh generated world with the default configuration and that
  seed. The WASM `Sim` records every command for cities started fresh in the current session,
  including refused ones. Loaded or shared cities have no log; starting from a save would tie
  fixtures to the save schema.
- `crates/sim/tests/replays/*.cmdlog` are the saved sessions. `tests/replay` replays each one and
  compares the state hash, population, building count and completed trips with its `expect` line.
  It also saves and loads halfway through and requires the same ending.
- An intended gameplay change that alters outcomes is accepted by rerunning the test with
  `BLESS_REPLAYS=1`, which rewrites the `expect` lines and keeps leading comments. Review the
  differences before committing.
- Developers record new logs in a development build with the city panel's **Command log**
  button, then add an `expect` line by blessing. The button is hidden in production builds.
- Bump the log version when an older log would read differently. Older versions are refused.

## Consequences

Any change that alters how a recorded city evolves fails a fast test, including changes that
unit tests miss. Gameplay tuning now needs a deliberate bless step. Expectations are blessed on
the native target; a log recorded in the browser rebuilds the same commands natively, but its
hash is not promised to match the WASM build (ADR 0033). Logs grow with play length, so
fixtures should stay short sessions. Logs record player commands only; adding a command kind
means extending the text format.
