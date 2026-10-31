# Roadmap

**Core fantasy: be the mayor of a town that becomes a city.** You start as a village, building
every road by hand. As the town grows you hand districts to councilors and step back to steer,
zooming in to hand-build whenever you want.

**Signature: delegation.** What sets this apart from Cities: Skylines is that you don't have to
place every road. You draw a district, give a councilor a brief ("dense housing", "industry
hub"), and they lay roads and zones inside it. You can override anything they do.

**Arc: village to city, in eras.** Population unlocks each era. Old buildings stay until demand
redevelops them, so the old core and the new streets sit side by side. Version one has two eras,
village and town. More come once that transition works.

**Look: stylized low-poly.** Flat-shaded, procedurally generated meshes, cheap to draw at scale.

**Citizens you can meet.** Click any car or person to see who they are, where they live and work,
and where they're going, and have the camera follow them.

**Sessions:** quick to start for a 15-minute visit, deep enough for a long evening.

**Platform:** desktop only (mouse and keyboard), in the browser first, then Steam. The map is flat
until much later. **1.0 is the Steam release.**

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
- Citizen inspector: click a car to see the citizen's name, home, job and destination; a follow
  camera rides along.
- Demand: a simple R/C/I balance decides what gets built next.
- Day clock and an on-screen time and speed indicator.
- Minimal UI: tool bar (road, zone, bulldoze), population counter.
- Hosted build (e.g. GitHub Pages) so the city is a link.

Done when: someone who has never seen the game opens the link, builds a town of a few
thousand people without instructions, follows a citizen home, and wants to keep going.

## M3 – Looks and feel

The point where the toy starts to look like a place.

- Low-poly art pass: building variety by zone and density, trees, terrain color, water. Build
  the building kit so it can carry an era style (village cottages vs. town blocks) later.
- Lighting and shadows, and a day/night cycle that follows the sim clock (lit windows, headlights).
- Varied vehicles (cars, vans, trucks) and pedestrians as small instanced figures. Pedestrians
  are clickable in the citizen inspector too.
- Ambient sound that changes with the camera (traffic hum, birds, city noise).
- Juice: buildings rising on construction, zone paint feedback, smooth tool previews.

Done when: a screenshot or short clip is something you'd post.

## M4 – Save, load, share

- Save and load. **ADR** for the save format: seed plus command log (ADR 0004), a state dump,
  or both, plus versioning. Scenarios (M5) are built on this format, so plan for a "start state
  plus goals" file.
- Share a city as a link or file. Opening one forks it: the friend keeps building their own copy
  from where you left off.
- Replay-based regression tests built from saved command logs.

Done when: a city survives a reload, and a friend can open a city you sent them and keep
building it.

## M5 – A game loop

Goals and consequences, kept light so they don't crowd out building.

- Money: build costs, taxes, upkeep.
- Services with coverage areas: power, water, roads maintenance, then fire, police, health and
  schools.
- Citizen happiness from services, traffic and pollution, feeding back into demand and growth.
  The citizen inspector shows each person's mood and complaints.
- Data overlays: traffic, coverage, land value, happiness.
- Two modes:
  - **Sandbox:** never fails. Problems slow growth or make people move away, but there is no game
    over.
  - **Scenarios:** a set start and goal ("reach 5,000 with no gridlock", "rescue a bankrupt
    town") that you can fail. Built on M4 save files.

Done when: there is a reason to plan and a way to struggle in scenarios, and sandbox still feels
relaxed.

## M6 – Eras and councilors

The signature milestone: the village becomes a town, and the town gets too big to build alone.

- **Eras.** Village (dirt roads, cottages, small shops) and town (paved streets, shopfronts,
  apartments). Reaching a population threshold unlocks the town era. **ADR** for how eras attach
  to roads, buildings and unlocks.
- **Gradual redevelopment.** Village buildings stay until demand replaces them with town-era
  ones. Mark buildings or areas as heritage to protect them.
- **Era-tied transit.** The town era brings buses and trams, with simple stops and lines.
- **Councilors.** Each era after the village adds one councilor slot. Draw a district, give the
  councilor a brief (dense housing, shops, industry hub, green), and they lay roads and zones
  inside it over time. Their plans show as ghosts as they build; you can override or bulldoze
  anything. **ADR** for the councilor planner: it runs inside the sim and issues ordinary
  commands, so it stays deterministic and replays from a save.
- Councilor UI: district outline, brief picker, progress, and a way to recall them.

Done when: a player takes a village to a town by hand, hires a councilor for a new district,
and is happy with what the councilor builds while they work elsewhere.

## M7 – Scale and Steam (1.0)

- Profile at 200k+ agents: culled or delta snapshots, shader-side vehicle interpolation, wasm
  threads if a profile calls for them (ADR 0002).
- Multi-lane roads and lane changes, highways and interchanges.
- Electron or Tauri wrapper, settings menu, keybinding options.
- Steam page, achievements, workshop (only if mods are in scope). **ADR** for mod support.

Done when: the game is on Steam.

## Parking lot

Ideas worth keeping that aren't scheduled:

- A third era (city: towers, metro, highways) and more councilor slots, once village → town works.
- Councilor personalities (planner, sprawler, green, car-first) and councilors who propose plans
  for approval.
- Procedurally generated terrain from a seed (hills, rivers, coast), then terrain editing.
- Shared-seed scenarios with scores to compare with friends.
- A feed of citizen life events (moving in, changing jobs, complaints).
- Districts and policies, disasters, multiplayer (determinism keeps lockstep open), a photo mode.

Out of scope for now: touch, tablet and phone controls.
