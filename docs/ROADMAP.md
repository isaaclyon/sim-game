# Roadmap

**Core fantasy: growing a city.** In the first ten minutes a player draws a few roads, paints
zones, and watches houses, shops and traffic fill in on their own. The city should feel alive
before it feels like a puzzle or a spreadsheet.

**Look: stylized low-poly.** Flat-shaded, procedurally generated meshes, cheap to draw at scale.

**First share: a playable toy.** A link friends can open to draw roads and zones and watch a city
grow. It has no economy and no way to fail.

Milestones run in order. Each one ends with something you can see in the browser. "Done when"
lines are the acceptance checks. Items marked **ADR** lock in a contract and need an ADR before
code lands (see [adrs/AGENTS.md](adrs/AGENTS.md)).

## Done

- **M0 – Traffic sandbox.** Road graph, A* pathfinding, lane-queue traffic, 50k agents in the
  browser, road tool, camera, determinism hash, CI checks.

## M1 – Roads that work

A growing city makes traffic. The traffic model has to hold up before anything grows on it.

- Intersection rules so circular waits stop gridlocking the network. **ADR** (replaces the known
  gap in ADR 0006).
- Snapping: new roads join existing nodes and split segments they cross.
- Curved roads (Bézier), behind the existing `lane_point` / `lane_heading` queries.
- Road deletion (bulldoze). **ADR** for tombstones or a free list (ADR 0005 leaves this open).
- Road meshes in place of lines: asphalt ribbons with simple intersection pads.

Done when: 50k agents drive for a sim-hour on a hand-drawn network with near-zero gridlock
despawns, and the road tool feels good with snapping, curves and bulldozing.

## M2 – A city that grows (playable toy)

- Zoning brush: residential, commercial, industrial, painted along roads.
- Lots and buildings appear on zoned land over time, as procedural low-poly boxes by zone and
  density. **ADR** for the building and lot data model.
- Citizens get a home and a workplace. Trips run home → work → shop → home on a daily
  schedule, replacing random node-to-node trips. **ADR** for the citizen record vs. trip agent
  split.
- Demand: a simple R/C/I balance decides what gets built next.
- Day clock and an on-screen time and speed indicator.
- Minimal UI: tool bar (road, zone, bulldoze), population counter.
- Hosted build (e.g. GitHub Pages) so the city is a link.

Done when: someone who has never seen the game opens the link, builds a town of a few
thousand people without instructions, and wants to keep going.

## M3 – Looks and feel

The point where the toy starts to look like a place.

- Low-poly art pass: building variety by zone and density, trees, terrain color, water.
- Lighting and shadows, and a day/night cycle that follows the sim clock (lit windows, headlights).
- Varied vehicles (cars, vans, trucks) and pedestrians as small instanced figures.
- Ambient sound that changes with the camera (traffic hum, birds, city noise).
- Juice: buildings rising on construction, zone paint feedback, smooth tool previews.

Done when: a screenshot or short clip is something you'd post.

## M4 – Save, load, share

- Save and load. **ADR** for the save format: seed plus command log (ADR 0004), a state dump,
  or both, plus versioning.
- Share a city as a link or file.
- Replay-based regression tests built from saved command logs.

Done when: a city survives a reload, and a friend can open a city you sent them.

## M5 – A game loop

Goals and consequences, kept light so they don't crowd out the growth fantasy.

- Money: build costs, taxes, upkeep.
- Services with coverage areas: power, water, roads maintenance, then fire, police, health and
  schools.
- Citizen happiness from services, traffic and pollution, feeding back into demand and growth.
- Milestone unlocks by population (new zones, road types, services).
- Data overlays: traffic, coverage, land value, happiness.

Done when: there is a reason to plan and a way to struggle, without it feeling like a chore.

## M6 – Scale and Steam

- Profile at 200k+ agents: culled or delta snapshots, shader-side vehicle interpolation, wasm
  threads if a profile calls for them (ADR 0002).
- Multi-lane roads and lane changes, highways and interchanges.
- Electron or Tauri wrapper, settings menu, controller and keybinding options.
- Steam page, achievements, workshop (only if mods are in scope). **ADR** for mod support.

## Parking lot

Ideas worth keeping that aren't scheduled: public transit (buses first), terrain editing, districts
and policies, disasters, multiplayer (determinism keeps lockstep open), a photo mode.
