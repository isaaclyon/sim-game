import assert from 'node:assert/strict';
import test from 'node:test';
import { worker } from './worker-harness.mjs';

test('normal worker frames report residents separately from cars', async () => {
  const w = await worker(0);
  w.ready();
  await w.init;
  const frame = () => w.messages.findLast((msg) => msg.type === 'frame');
  assert.equal(frame().stats.population, 0);
  assert.equal(frame().stats.agents, 0);
  // An empty town wants a little of everything (ADR 0024).
  const { residential, commercial, industrial } = frame().stats.demand;
  assert.ok(residential > 0 && commercial > 0 && industrial > 0);
  await w.send({ type: 'fillBlock', x: 50, y: 50, zone: 1 });
  for (let i = 0; i < 1200; i++) w.advance(101);
  assert.ok(frame().stats.population > 0);
  assert.equal(frame().stats.driving, 0);
  assert.ok(frame().stats.demand.residential < residential, 'residents fill housing demand');
});

for (const type of ['buildRoad', 'buildCurve', 'deleteRoad']) {
  test(`${type} publishes reduced population and unemployment immediately when occupied homes are cut while paused`, async () => {
    const w = await worker(0);
    w.ready();
    await w.init;
    await w.send({ type: 'fillBlock', x: 50, y: 50, zone: 2 });
    for (let i = 0; i < 1200; i++) w.advance(101);
    await w.send({ type: 'setSpeed', speed: 0 });
    const before = w.messages.findLast((msg) => msg.type === 'frame');
    assert.ok(before.stats.population > 0);
    const frames = w.messages.filter((msg) => msg.type === 'frame').length;
    const roads = w.messages.findLast((msg) => msg.type === 'roads').segments;
    await w.send({
      type,
      x: (roads[0] + roads[2]) / 2,
      y: (roads[1] + roads[3]) / 2,
      ax: -100,
      ay: 50,
      cx: 125,
      cy: 80,
      bx: 350,
      by: 50,
      class: 0,
      id: 99,
    });
    if (type !== 'deleteRoad') assert.equal(w.messages.at(-1).success, true);
    assert.equal(w.messages.filter((msg) => msg.type === 'frame').length, frames + 1);
    const after = w.messages.findLast((msg) => msg.type === 'frame');
    assert.notEqual(after, before, 'paused construction must publish new statistics');
    assert.equal(after.stats.tick, before.stats.tick);
    assert.ok(after.stats.population < before.stats.population);
    assert.ok(after.stats.unemployed <= after.stats.population);
    assert.notEqual(after.stats.unemployed, before.stats.unemployed);
    assert.equal(
      after.stats.unemploymentPercent,
      after.stats.population === 0
        ? 0
        : Math.fround((after.stats.unemployed * 100) / after.stats.population),
    );
  });
}

test('curve preview, build and bulldoze agree across the worker boundary while paused', async () => {
  const w = await worker();
  w.ready();
  await w.init;
  await w.send({ type: 'setSpeed', speed: 0 });
  const before = w.messages.find((msg) => msg.type === 'roads').segments;
  const coords = { ax: -1000, ay: -1000, cx: -900, cy: -900, bx: -800, by: -1000 };
  await w.send({ type: 'previewCurve', ...coords, revision: 10 });
  const preview = w.messages.at(-1);
  assert.equal(preview.revision, 10);
  assert.ok(preview.stops.length > 6);
  await w.send({ type: 'buildCurve', ...coords, class: 0, id: 20 });
  const result = w.messages.at(-1);
  assert.equal(result.type, 'buildResult');
  assert.equal(result.id, 20);
  assert.equal(result.success, true);
  const added = w.messages.findLast((msg) => msg.type === 'roads').segments.slice(before.length);
  assert.equal(added.length / 5, preview.stops.length / 3 - 1);
  await w.send({ type: 'previewBulldoze', x: -900, y: -950, revision: 11 });
  const selected = w.messages.at(-1);
  assert.equal(selected.revision, 11);
  assert.equal(selected.segment.length / 2, added.length / 5 + 1);
  for (let i = 0; i < added.length / 5; i++) {
    assert.deepEqual(
      [...selected.segment.slice(i * 2, i * 2 + 4)],
      [...added.slice(i * 5, i * 5 + 4)],
    );
  }
  await w.send({ type: 'deleteRoad', x: -900, y: -950 });
  assert.deepEqual(w.messages.at(-2).segments, before);
  assert.equal(w.messages.at(-1).type, 'frame');
  assert.equal(w.messages.at(-1).stats.tick, 0);
  await w.send({ type: 'buildCurve', ...coords, class: 0, id: 21 });
  assert.equal(w.messages.at(-1).success, true, 'a deleted curve does not block rebuilding');
  assert.deepEqual(
    w.messages.findLast((msg) => msg.type === 'roads').segments.slice(before.length),
    added,
  );
  const count = w.messages.length;
  await w.send({ type: 'buildCurve', ...coords, class: 0, id: 22 });
  assert.equal(w.messages.length, count + 1, 'refused builds do not publish a changed network');
  assert.equal(w.messages.at(-1).id, 22);
  assert.equal(w.messages.at(-1).success, false);
});

test('every preview request gets a reply even during WASM initialization', async () => {
  const w = await worker();
  for (const type of ['previewBulldoze', 'previewSnap', 'previewRoad', 'previewCurve']) {
    await w.send({ type, x: 0, y: 0, ax: 0, ay: 0, bx: 100, by: 0, revision: 7 });
    assert.equal(w.messages.at(-1)?.revision, 7);
    const reply = w.messages.pop();
    assert.equal((reply.segment ?? reply.stops).length, 0);
  }
  w.ready();
  await w.init;
  const roads = w.messages.find((msg) => msg.type === 'roads').segments;
  const x = (roads[0] + roads[2]) / 2;
  const y = (roads[1] + roads[3]) / 2;
  await w.send({ type: 'previewBulldoze', x, y, revision: 8 });
  assert.equal(w.messages.at(-1).segment.length, 4);
  await w.send({ type: 'setSpeed', speed: 0 });
  await w.send({ type: 'deleteRoad', x, y });
  assert.equal(w.messages.at(-2).type, 'roads');
  assert.equal(w.messages.at(-2).segments.length, roads.length - 5);
  assert.equal(w.messages.at(-1).type, 'frame', 'paused edits refresh cars and signals');
  assert.equal(w.messages.at(-1).stats.tick, 0, 'deletion does not advance time');
});

test('the worker sends the zone grid on start and after each paint that changes it', async () => {
  const w = await worker();
  w.ready();
  await w.init;
  const first = w.messages.find((msg) => msg.type === 'zones');
  const [x0, y0, cell, cols, rows] = first.layout;
  assert.equal(cell, 8);
  assert.equal(first.cells.length, cols * rows);
  assert.ok(first.cells.every((c) => c === 0));
  // Residential high density over the first 3 x 2 cells.
  await w.send({ type: 'paintZone', ax: x0, ay: y0, bx: x0 + 24, by: y0 + 16, zone: 2 });
  const painted = w.messages.findLast((msg) => msg.type === 'zones');
  assert.notEqual(painted, first);
  assert.equal(w.messages.at(-1).type, 'frame', 'zoning can add alleys, which can move cars');
  assert.deepEqual([...painted.cells.slice(0, 4)], [2, 2, 2, 0]);
  assert.deepEqual([...painted.cells.slice(cols, cols + 4)], [2, 2, 2, 0]);
  assert.equal(painted.cells[2 * cols], 0);
  const count = w.messages.length;
  await w.send({ type: 'paintZone', ax: x0, ay: y0, bx: x0 + 24, by: y0 + 16, zone: 2 });
  assert.equal(w.messages.length, count, 'a paint that changes nothing sends nothing');
  await w.send({ type: 'paintZone', ax: x0, ay: y0, bx: x0 + 24, by: y0 + 16, zone: 9 });
  assert.equal(w.messages.length, count, 'unknown zones are refused');
});

test('a click fills the block under it, and its lots are sent', async () => {
  const w = await worker();
  w.ready();
  await w.init;
  await w.send({ type: 'setSpeed', speed: 0 });
  const lots = w.messages.find((msg) => msg.type === 'lots');
  assert.equal(lots.rings.length, 0, 'no lots before zoning');
  // The generated town's first block lies between its first two parallel roads.
  const roads = w.messages.find((msg) => msg.type === 'roads').segments;
  const [x0, y0] = [Math.min(roads[0], roads[2]), Math.min(roads[1], roads[3])];
  const [x, y] = [x0 + 50, y0 + 50];
  await w.send({ type: 'previewBlock', x, y, revision: 3 });
  const outline = w.messages.at(-1);
  assert.equal(outline.type, 'blockPreview');
  assert.equal(outline.revision, 3);
  assert.ok(outline.outline.length >= 8, 'a closed block has at least four corners');
  await w.send({ type: 'previewBlock', x: x0 - 500, y: y0 - 500, revision: 4 });
  assert.equal(w.messages.at(-1).outline.length, 0, 'no block outside the town');
  const count = w.messages.length;
  await w.send({ type: 'fillBlock', x: x0 - 500, y: y0 - 500, zone: 1 });
  assert.equal(w.messages.length, count, 'filling outside every block changes nothing');
  await w.send({ type: 'fillBlock', x, y, zone: 1 });
  const zones = w.messages.findLast((msg) => msg.type === 'zones');
  assert.ok(zones.cells.some((c) => c === 1));
  const cut = w.messages.findLast((msg) => msg.type === 'lots');
  assert.ok(cut.rings.length > 0);
  // Every ring is a point count of at least three, then that many points, end to end.
  let i = 0;
  while (i < cut.rings.length) {
    assert.ok(cut.rings[i] >= 3);
    i += 1 + 2 * cut.rings[i];
  }
  assert.equal(i, cut.rings.length);
  assert.equal(w.messages.at(-1).type, 'frame');
  assert.equal(w.messages.at(-1).stats.tick, 0, 'filling does not advance time');
});

test('frames carry the sim-owned day clock, which pause holds and speed scales', async () => {
  const w = await worker();
  w.ready();
  await w.init;
  const clock = () => {
    const { stats } = w.messages.findLast((msg) => msg.type === 'frame');
    return [stats.day, stats.secondOfDay];
  };
  // A tick is f32 0.1 s, a hair over 100 ms, so give each step a little slack.
  w.advance(1001);
  assert.deepEqual(clock(), [1, 6 * 3600 + 6], 'one second at 1x is six clock seconds');
  await w.send({ type: 'setSpeed', speed: 0 });
  const frames = w.messages.length;
  w.advance(5000);
  assert.equal(w.messages.length, frames, 'paused: no ticks, no new frames');
  await w.send({ type: 'setSpeed', speed: 3 });
  for (let i = 0; i < 10; i++) w.advance(101);
  assert.deepEqual(clock(), [1, 6 * 3600 + 24], 'one real second at 3x is 18 clock seconds');
});
