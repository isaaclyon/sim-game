# 0001. Record architecture decisions

- Status: Accepted
- Date: 2026-10-30

## Context

Most code in this project is written by LLM agents, with a human acting as art and experience
director. Agents start each session without memory of earlier sessions, so decisions that live
only in chat history get relitigated or quietly broken.

## Decision

Keep architecture decisions as numbered Markdown files in `docs/adrs/`. `CLAUDE.md` points agents
here. An agent proposing a change that contradicts an ADR should say so and write a superseding
ADR instead of drifting from it.

## Consequences

- Agents and humans have one place to learn why the code looks the way it does.
- Small cost per decision. Only decisions that constrain future work get an ADR, not every choice.
