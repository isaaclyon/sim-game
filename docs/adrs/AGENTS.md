# Writing ADRs

ADRs are for decisions that are **hard to reverse**. If a decision could be undone in an afternoon
without touching much else, it doesn't need an ADR; a code comment or commit message is enough.

## Write an ADR when the decision

- is expensive to undo: changing it would mean rewriting many files, migrating data, or breaking
  saves or replays
- becomes a contract other code depends on (snapshot formats, id schemes, the command boundary)
- locks in a language, platform, engine, or core dependency
- carries legal or distribution consequences (licensing, store requirements)
- trades away a future option (multiplayer, mod support, platform targets)

## Don't write an ADR for

- library picks that sit behind one module and could be swapped in a day
- tuning values (speeds, spacing, budgets, colors)
- UI layout, controls, or visual style
- refactors, naming, file organization
- anything still experimental. Prototype first; write the ADR when the choice hardens.

When unsure, ask: "If we got this wrong, how much work would it be to change in six months?"
Hours means no ADR. Weeks, or "we'd have to break saves," means write one.

## Format

Copy an existing ADR. Keep it short: Status, Date, Context, Decision, Consequences. Number it
next in sequence and add a row to `README.md`.

## Changing a decision

Don't rewrite an accepted ADR. Write a new one that supersedes it, and set the old one's status
to `Superseded by NNNN`. Typo fixes and added links are fine to edit in place.
