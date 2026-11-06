# 0047. Move-ins follow daily schedules

- Status: Accepted; supersedes the first-activity timing in [0021](0021-citizens-households-and-trips.md)
- Date: 2026-11-05

## Context

A short first-activity delay sends new residents to work even when they move in at midnight.
It makes the clock and morning departure times misleading.

## Decision

New residents use the same daily schedule as returning residents rather than a special
15–120 sim-second first departure. Employed residents leave at their individual 07:00–09:00
clock time; unemployed residents schedule optional shopping at 10:00–16:00. A departure time
already passed means the next day, not an immediate late trip.

Job assignment at home reschedules the pending activity to the next work departure. It does
not interrupt an ongoing trip or activity. Household relocation and recovery to home after
building removal also resume the daily schedule. Trips returning from elsewhere still retry
normally.

## Consequences

Building a residential district at night admits residents and assigns jobs without creating
an immediate commute. New districts may remain quiet until morning. Event heaps, seeded
variation, work durations measured from arrival, and save structure are unchanged. Existing
saved deadlines remain intact until processed or rescheduled. Replay expectations change
because trips and seeded random draws occur at different times.
