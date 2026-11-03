# 0033. Save one complete city locally and resume it on return

- Status: Accepted; implemented
- Date: 2026-11-03
- Extends: [0003](0003-sim-and-rendering-are-separate.md) and
  [0004](0004-deterministic-fixed-timestep-sim.md)

## Context

Reloading currently starts a fresh city. Players need to keep building between sessions,
without a save browser or accounts. A seed plus tick-stamped commands can reproduce a city
on the same build, but loading requires replaying its entire history. Gameplay changes such
as shop capacities can also produce a different city from the same actions on a newer build.
The rendered snapshots omit residents, queues and other state needed to resume play.

## Decision

- Save the complete simulation state, not rendered snapshots or a command replay. The sim
  owns encoding, decoding and validation; loading constructs a candidate `World`. The WASM
  wrapper only converts bytes and errors. No simulation state is reconstructed in JavaScript.
- The payload preserves configuration, tick, RNG states, append-only IDs and tombstones,
  roads, zones, lots, buildings and their registered capacities, residents, homes, households,
  journeys, traffic and signal state, and all scheduled events and ordered work queues.
  Rebuild only derived indexes and scratch buffers whose reconstruction cannot change future
  behavior. Do not recalculate existing capacities from the current gameplay formulas.
- Use a versioned envelope identifying the save format and its schema version. Validate the
  payload before installing a loaded world. Incompatible schema changes require a version
  bump; never interpret an incompatible payload as a fresh city. No older-format conversion
  is included initially. The byte codec remains private to the sim's persistence module.
- Store one city in IndexedDB, owned by the existing simulation worker. Persist the opaque
  sim payload and its saved-at timestamp together in one atomic transaction. Browser storage
  is not a second game-state owner. Timestamps are display metadata, never simulation inputs.
- Manual saving and autosaving replace the same slot. Report success only after the storage
  transaction commits. A failed save leaves the previous committed save intact. Serialize
  writes so an older request cannot overwrite a newer city or newer state.
- At startup, read the slot before starting a fresh world. If a save exists, restore it paused;
  no offline simulation runs. Publish complete geometry and a frame, resetting worker version
  caches, clock backlog, frontend interpolation, tool previews and inspector selection.
- A missing slot starts a normal fresh city. A failed read or an invalid/incompatible save
  shows an error and blocks automatic replacement of that slot. Starting over requires an
  explicit confirmed New city action; an error is never permission to erase a city.
- New city requires confirmation before discarding the current city and replacing the slot.
  Cancel leaves both unchanged. Ensure any pending save from the old city cannot later
  overwrite the replacement. A failed replacement write retains the previous stored city and
  reports that the new city is not saved; it must not claim that persistence succeeded.
- This first slice has no named saves, save history, cloud storage, accounts, sharing,
  file import/export, camera persistence or migration framework. Tool and inspector state
  are transient. Resume paused regardless of the speed at which the city was saved.

## Consequences

Cities reopen without replaying hours of simulation. Stored footprints and job capacities
survive tuning changes when the save schema remains compatible; future simulation uses the
new build's rules. Native and WASM saves are not promised identical future hashes across
platforms; determinism checks compare the same build and target (ADR 0004).

Full-state persistence reaches more sim types than command logging and needs careful coverage
of event queues and RNG state. Round-trip equality alone is insufficient: restored worlds
must also evolve identically under subsequent ticks and commands. Corrupt and unsupported
saves must fail without replacing the current world or stored data.

Storage is scoped to the browser profile and site origin. Clearing site data, browser eviction
or using a different browser can lose access to the city. Private-mode or quota failures must
be visible. This is not a backup system; sharing and downloadable backups remain later work.
Saving on tab close is not a reliability guarantee. The initial UI and delivery checks live
in the [M4 implementation plan](../ROADMAP.md#m4--save-load-share).
