# 0039. Road edit history lives in the sim and is saved with the city

- Status: Superseded by [0051](0051-road-edit-history-revised.md)
- Date: 2026-11-04

## Context

M4.5 adds many-step undo and redo for road edits: builds, upgrades and bulldozes. Building a
road can split other roads, remove cars and re-cut lots, which can remove buildings. Road and
lane IDs are append-only (ADR 0005, 0013). Saves hold one complete city (ADR 0033) and replay
tests compare command logs, including a save and load halfway through (ADR 0034).

## Decision (proposed)

- Undo restores roads only, as a new edit: it removes the roads an edit added and rejoins roads
  the edit split, with new IDs. Buildings and lots the edit removed are not restored; lots
  regrow under the normal rules. Redo re-applies the edit the same way.
- The history is sim state, stored per edit as the shapes and classes added and removed, not
  as world snapshots. `Undo` and `Redo` are ordinary commands, so command logs replay them.
- The history is saved with the city, so undo works after a reload and a mid-replay save and
  load stays exact. This bumps the save format version.
- Zoning edits are not part of the history.

## Consequences

- Rejoining split roads needs a new road-network operation (merging two roads at a node with
  no other roads), the riskiest new sim piece.
- Saves grow with history; a cap on depth (to be chosen) bounds it.
- A redo stack is cleared by any new road edit, as in most editors.
