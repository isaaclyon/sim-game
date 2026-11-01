# 0022. Citizen inspection is a read-only query for one selected citizen

- Status: Accepted
- Date: 2026-11-01

## Context

The citizen inspector (M2) shows the name, home, job and current trip of the person driving a
clicked car, and can follow that car. The sim is the source of truth (ADR 0003), but nothing
describes one citizen to the frontend, and citizens have no names. Broadcasting every
citizen's details each frame would cost bandwidth that grows with population for a panel that
shows one person. A car's vehicle slot is lazily allocated and belongs to one citizen for good
(ADR 0021), so the agent snapshot index identifies a car, not a person, until mapped.

## Decision

- The frontend picks a drawn car by its agent snapshot index and sends `inspect` with that
  index and a selection counter. The worker maps it once to a `CitizenId` through
  `World::citizen_of_agent` and keeps the citizen, not the slot. Synthetic benchmark cars and
  unknown slots select nobody.
- `World::citizen_info` answers for that one citizen only: name, whether they still live in
  town, home, job, current activity, location, trip destination, vehicle slot and whether the
  car is on a road. It takes `&self`, draws from no RNG and is not part of `state_hash`.
  Places are building IDs with their original use; a place whose building is gone is reported
  as absent, and a citizen who left town reports no places. Unknown IDs answer nothing.
- The worker sends the answer at once, paused or not, then after every frame and edit while
  something is selected, and nothing otherwise. Replies echo the selection counter so the
  frontend drops answers to an older selection.
- Display names are derived, not stored: a given name from the world seed and `CitizenId`,
  and a surname from the seed and `HouseholdId`, picked from fixed lists by a SplitMix64 hash.
  A shared surname is a display convention for people who share a home, not a family
  relationship. The same seed and IDs give the same names for as long as the lists and the
  hash stay unchanged; changing either renames everyone. Names make no save compatibility
  promise.
- Following is a frontend camera behaviour over the drawn car: it holds while the car is
  hidden, resumes when the same citizen drives again, and stops when the citizen leaves town.

## Consequences

Inspection costs one small query per frame while selected and nothing otherwise, and cannot
change the simulation. Names need no storage or hashing and survive replays from the same seed
and command log, but are only as stable as the naming lists. Future inspector fields (mood,
complaints) extend the same query rather than a broadcast. Pedestrians (M3) will need their
own picking path into the same citizen query.
