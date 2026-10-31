# 0008. Permissive-license dependencies only

- Status: Accepted
- Date: 2026-10-30

## Context

A Steam release is possible. Copyleft code (GPL, AGPL) in the shipped game would require
releasing the game under the same license. Several of the best reference projects are copyleft.

## Decision

- Dependencies must be MIT, Apache-2.0, BSD, ISC, Zlib or similar permissive licenses.
- LGPL libraries need a separate decision before use.
- Copyleft projects are reading material only. Learn ideas from them; don't copy or port code.
  This includes Egregoria (GPL-3.0), Citybound (AGPL-3.0) and movsim (GPL-3.0).
- Projects with unclear licenses (e.g. lo-th/3d.city) are treated as copyleft until verified.

## Consequences

- We keep the option to ship commercially without relicensing.
- Agents must check a license before adding a dependency or adapting code from another repo.
- Some good code is off-limits; we reimplement from first principles or from permissive sources
  like A/B Street and osm2streets (Apache-2.0).
