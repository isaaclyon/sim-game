import assert from 'node:assert/strict';
import test from 'node:test';

import { RoadClass, Snap, Zone } from '../src/protocol.ts';
import { tools } from './main-harness.mjs';

const ALL = Snap.Angle | Snap.Guides | Snap.Length;
/** A sim answer to `previewRoad`: the guided points, the planned stops and the readout. */
const answer = (ask, points, stops, extra = {}) => ({
  type: 'roadPreview',
  revision: ask.revision,
  points: new Float32Array(points),
  stops: new Float32Array(stops),
  guides: new Float32Array(),
  angles: new Float32Array(),
  length: 0,
  angle: Number.NaN,
  reclass: new Float32Array(),
  ...extra,
});
/** The last road preview drawn, skipping guide lines. */
const lastRoad = (t) => t.previews.findLast((p) => p.type === 'road');

test('normal startup has no synthetic traffic and the top bar shows authoritative population and unemployment', async () => {
  const t = await tools();
  assert.equal(t.messages[0].type, 'init');
  assert.equal(t.messages[0].agents, 0);
  const stats = {
    day: 1,
    secondOfDay: 21600,
    population: 1234,
    unemployed: 56,
    unemploymentPercent: 4.538087,
    demand: { residential: 0, commercial: 0, industrial: 0 },
    power: { supply: 0, starter: 150, used: 0, demand: 0, unpowered: 0, waiting: 0 },
    balance: 100_000,
    taxes: 0,
    grant: 0,
    upkeep: 0,
    agents: 9,
    parked: 0,
    driving: 2,
    walking: 0,
    walksCompleted: 0,
    tripsCompleted: 0,
    despawnedStuck: 0,
    pathsFailed: 0,
    despawnedByEdit: 0,
  };
  const show = (changes) => {
    t.reply({
      type: 'frame',
      agents: new Float32Array(),
      signals: new Float32Array(),
      tickMs: 0,
      stats: { ...stats, ...changes },
    });
    t.frame();
  };
  show({});
  assert.equal(t.elements['population-value'].textContent, 'Pop 1,234');
  assert.equal(t.elements['pop-unemployed'].textContent, '56 (4.5%)');
  show({ population: 0, unemployed: 0, unemploymentPercent: 0 });
  assert.equal(t.elements['population-value'].textContent, 'Pop 0');
  assert.equal(t.elements['pop-unemployed'].textContent, '0 (0.0%)');
  assert.match(t.elements.menu.textContent, /W A S D or drag/, 'camera help is in the ☰ menu');
});

test('toolbar shortcuts leave modified, repeated and native control keys alone', async () => {
  const t = await tools();
  for (const options of [
    { ctrlKey: true },
    { metaKey: true },
    { altKey: true },
    { repeat: true },
    { isComposing: true },
  ]) {
    t.key('r', options);
    assert.notEqual(t.elements.straight.attributes['aria-pressed'], 'true');
  }
  t.key('R');
  assert.equal(t.elements.straight.attributes['aria-pressed'], 'true');
  t.key('r', { repeat: true });
  assert.equal(t.elements.straight.attributes['aria-pressed'], 'true');
  const before = t.messages.length;
  const button = { closest: (selector) => (selector.includes('button') ? button : null) };
  t.key(' ', { target: button });
  assert.equal(t.messages.length, before, 'Space on a button must only activate that button');
  const input = { closest: (selector) => (selector.includes('input') ? input : null) };
  t.key('z', { target: input });
  assert.equal(
    t.elements.straight.attributes['aria-pressed'],
    'true',
    'typing must not switch tools',
  );
  t.key(' ');
  assert.equal(t.messages.at(-1).type, 'setSpeed');
  assert.equal(t.messages.at(-1).speed, 0);
  t.key('Escape');
  assert.equal(t.elements.straight.attributes['aria-pressed'], 'false');
});

test('zoning shortcuts and drags stay exclusive with curved roads and bulldozing', async () => {
  const t = await tools();
  t.key('v');
  t.click(0, 0);
  t.click(100, 100);
  t.move(200, 0);
  const old = t.messages.at(-1);
  for (const [key, low] of [
    ['z', Zone.ResidentialLow],
    ['x', Zone.CommercialLow],
    ['c', Zone.IndustrialLow],
  ]) {
    t.key(key);
    assert.equal(t.elements.curve.attributes['aria-pressed'], 'false');
    assert.equal(t.pans.at(-1), false, 'painting disables left-drag panning');
    for (const expected of [low, low + 1]) {
      if (expected !== low) t.key(key);
      t.down(10, 20);
      t.move(50, 60);
      assert.equal(t.zoneDrags.at(-1).zone, expected);
      t.up(50, 60);
      const paint = t.messages.at(-1);
      assert.equal(paint.type, 'paintZone');
      assert.equal(paint.zone, expected);
      assert.deepEqual([paint.ax, paint.ay, paint.bx, paint.by], [10, 20, 50, 60]);
    }
  }
  t.reply(answer(old, [0, 0, 200, 0], [0, 0, 0, 200, 0, 0]));
  assert.equal(lastRoad(t).stops, null, 'late curve previews cannot cover the zone brush');
  t.key('u');
  t.click(10, 20);
  assert.equal(t.messages.at(-1).type, 'fillBlock', 'a click fills the block under it');
  assert.deepEqual([t.messages.at(-1).x, t.messages.at(-1).y], [10, 20]);
  assert.equal(t.messages.at(-1).zone, Zone.None);
  t.down(10, 20);
  t.key('v');
  t.up(50, 60);
  assert.equal(t.pans.at(-1), true);
  assert.equal(t.zoneDrags.at(-1).a, null, 'switching tools clears the drag');
  t.click(0, 0);
  t.click(100, 100);
  t.click(200, 0);
  const curve = t.messages.at(-1);
  assert.equal(curve.type, 'buildRoad', 'V still builds a curve after zoning');
  assert.equal(curve.points.length, 9, 'start, control and end');
  t.pressId('category-zones');
  t.pressId('houses');
  assert.equal(t.elements.houses.attributes['aria-pressed'], 'true');
  assert.equal(t.pans.at(-1), false, 'the Houses item picks the brush');
  t.key('b');
  assert.equal(t.pans.at(-1), true);
  const zoneItems = t.elements['items-zones'].querySelectorAll('button');
  assert.equal(zoneItems.length, 7);
  assert.ok(zoneItems.every((b) => b.attributes['aria-pressed'] === 'false'));
  t.click(100, 50);
  assert.equal(t.messages.at(-1).type, 'deleteRoad');
  const cells = new Uint8Array([1]);
  t.reply({ type: 'zones', cells, layout: [0, 0, 8, 1, 1] });
  assert.equal(
    t.previews.at(-1).cells,
    cells,
    'zone snapshots remain separate from traffic frames',
  );
});

test('the zone brush outlines the block under the cursor, one question at a time', async () => {
  const t = await tools();
  t.move(5, 5);
  assert.equal(t.messages.length, 1, 'only init: no brush, no block question');
  t.key('z');
  t.move(10, 10);
  const asked = t.messages.at(-1);
  assert.equal(asked.type, 'previewBlock');
  assert.deepEqual([asked.x, asked.y], [10, 10]);
  t.move(20, 20);
  t.move(30, 30);
  assert.equal(t.messages.at(-1), asked, 'waits for the reply in flight');
  const outline = new Float32Array([0, 0, 100, 0, 100, 100, 0, 100]);
  t.reply({ type: 'blockPreview', outline, revision: asked.revision });
  assert.deepEqual(t.previews.at(-1), { type: 'block', outline, zone: Zone.ResidentialLow });
  const latest = t.messages.at(-1);
  assert.deepEqual([latest.type, latest.x, latest.y], ['previewBlock', 30, 30], 'latest only');
  const shown = t.previews.length;
  t.down(30, 30);
  t.reply({ type: 'blockPreview', outline, revision: latest.revision });
  assert.equal(t.previews.length, shown, 'a reply from before the click is not drawn');
  t.up(30, 30);
  assert.equal(t.messages.at(-1).type, 'fillBlock');
  t.move(40, 40);
  const again = t.messages.at(-1);
  assert.equal(again.type, 'previewBlock', 'hovering resumes after the click');
  t.key('Escape');
  t.reply({ type: 'blockPreview', outline, revision: again.revision });
  assert.notEqual(t.previews.at(-1).type, 'block', 'nor once the brush is off');
  const rings = new Float32Array([3, 0, 0, 8, 0, 8, 8]);
  t.reply({ type: 'lots', rings });
  assert.deepEqual(t.previews.at(-1), { type: 'lots', rings });
});

test('switching from curve to bulldoze discards a delayed preview and resets construction', async () => {
  const t = await tools();
  t.key('v');
  t.click(0, 0);
  t.click(100, 100);
  t.move(200, 0);
  const old = t.messages.at(-1);
  assert.equal(old.type, 'previewRoad');
  assert.deepEqual([...old.points], [0, 0, ALL, 100, 100, ALL, 200, 0, ALL]);
  assert.equal(old.plan, true);
  t.key('b');
  assert.equal(t.elements.curve.attributes['aria-pressed'], 'false');
  assert.equal(t.elements.bulldoze.attributes['aria-pressed'], 'true');
  t.move(100, 50);
  t.reply(answer(old, [0, 0, 200, 0], [0, 0, 0, 200, 0, 0]));
  assert.equal(lastRoad(t).stops, null, 'old curve cannot appear in bulldoze mode');
  const target = t.messages.at(-1);
  assert.equal(target.type, 'previewBulldoze');
  const segment = new Float32Array([0, 0, 100, 50, 200, 0]);
  t.reply({ type: 'bulldozePreview', revision: target.revision, segment });
  assert.equal(t.previews.at(-1).segment, segment);
  t.click(100, 50);
  assert.equal(t.messages.at(-1).type, 'deleteRoad');
  t.key('r');
  t.move(300, 0);
  const fresh = t.messages.at(-1);
  assert.equal(fresh.type, 'previewRoad');
  assert.equal(fresh.points.length, 3, 'switching tools starts a new road');
  t.key('Escape');
  assert.equal(t.elements.straight.attributes['aria-pressed'], 'false');
  assert.equal(lastRoad(t).stops, null);
});

test('refused curves retain their control, successful curves chain, and late build replies cannot revive a cancelled tool', async () => {
  const t = await tools();
  t.pressId('category-roads');
  t.pressId('street');
  t.pressId('curve');
  t.click(0, 0);
  t.click(100, 100);
  t.click(200, 0);
  const first = t.messages.at(-1);
  assert.equal(first.type, 'buildRoad');
  t.reply({ type: 'buildResult', id: first.id, success: false, end: null });
  assert.match(t.elements['build-hint'].textContent, /Cannot build here/);
  t.click(250, 0);
  const retry = t.messages.at(-1);
  assert.deepEqual([...retry.points], [0, 0, ALL, 100, 100, ALL, 250, 0, ALL]);
  // The sim reports where the guided end landed; the next road starts there, unsnapped.
  t.reply({ type: 'buildResult', id: retry.id, success: true, end: [248, 0] });
  t.click(350, 100);
  t.click(450, 0);
  const chained = t.messages.at(-1);
  assert.deepEqual([...chained.points], [248, 0, 0, 350, 100, ALL, 450, 0, ALL]);
  t.key('b');
  t.reply({ type: 'buildResult', id: chained.id, success: true, end: [450, 0] });
  assert.match(t.elements['build-hint'].textContent, /remove it/);
  t.move(350, 50);
  assert.equal(t.messages.at(-1).type, 'previewBulldoze');
  t.click(350, 50, 2);
  assert.equal(t.elements.bulldoze.attributes['aria-pressed'], 'false');
});

test('M builds main roads with either road tool until switched off', async () => {
  const t = await tools();
  t.key('m');
  assert.equal(t.elements['main-road'].attributes['aria-pressed'], 'true');
  assert.equal(t.elements.straight.attributes['aria-pressed'], 'true', 'picks the road tool');
  assert.match(t.elements['build-hint'].textContent, /Main road/);
  t.click(0, 0);
  t.click(100, 0);
  const straight = t.messages.at(-1);
  assert.equal(straight.type, 'buildRoad');
  assert.equal(straight.class, RoadClass.Main);
  t.key('v');
  t.click(0, 0);
  t.click(50, 50);
  t.click(100, 0);
  assert.equal(t.messages.at(-1).class, RoadClass.Main, 'kept across road tools');
  t.pressId('street');
  assert.equal(t.elements['main-road'].attributes['aria-pressed'], 'false');
  assert.equal(t.elements.street.attributes['aria-pressed'], 'true');
  assert.equal(t.elements.curve.attributes['aria-pressed'], 'true', 'the shape stays');
  t.key('r');
  t.click(0, 0);
  t.click(100, 0);
  assert.equal(t.messages.at(-1).class, RoadClass.Street);
});

test('road previews are drawn at the width of the chosen road class', async () => {
  const t = await tools();
  t.key('m');
  t.move(10, 20);
  const ask = t.messages.at(-1);
  assert.deepEqual([ask.type, ask.points.length], ['previewRoad', 3]);
  t.reply(answer(ask, [10, 20], [10, 20, 0]));
  assert.equal(lastRoad(t).roadClass, RoadClass.Main);
});

test('snapping can be switched per kind, or held off with Shift, and the readout follows', async () => {
  const t = await tools();
  assert.equal(t.elements['build-options'].hidden, true, 'options belong to a tool in use');
  t.key('r');
  assert.equal(t.elements['build-options'].hidden, false);
  assert.equal(t.elements['options-roads'].hidden, false);
  t.click(0, 0);
  t.move(100, 3);
  const ask = t.messages.at(-1);
  assert.deepEqual([...ask.points], [0, 0, ALL, 100, 3, ALL]);
  const guides = new Float32Array([0, 0, 100, 0]);
  t.reply(answer(ask, [0, 0, 96, 0], [0, 0, 0, 96, 0, 0], { guides, length: 96, angle: 0 }));
  assert.equal(
    t.previews.findLast((p) => p.type === 'guides').lines,
    guides,
    'guide lines are drawn as the sim sent them',
  );
  const { b } = lastRoad(t);
  assert.deepEqual([b.x, b.z], [96, 0], 'the end marker sits on the guided point');
  assert.equal(t.elements['road-readout'].hidden, false);
  assert.equal(t.elements['road-readout'].textContent, '96 m · 0°');
  // Shift asks again at once, with no snapping, without a cursor move.
  t.key('Shift');
  const free = t.messages.at(-1);
  assert.deepEqual([free.type, ...free.points], ['previewRoad', 0, 0, ALL, 100, 3, 0]);
  t.keyup('Shift');
  t.pressId('snaps');
  t.pressId('snap-length');
  assert.equal(t.elements['snap-length'].attributes['aria-pressed'], 'false');
  t.reply(answer(free, [0, 0, 100, 3], [0, 0, 0, 100, 3, 0]));
  const noLength = t.messages.at(-1);
  assert.deepEqual([...noLength.points], [0, 0, ALL, 100, 3, ALL & ~Snap.Length]);
  // A refused plan hides nothing but says why.
  t.reply(answer(noLength, [0, 0, 100, 3], [], { length: 100, angle: 2 }));
  assert.match(t.elements['build-hint'].textContent, /Cannot build here/);
  t.key('Escape');
  assert.equal(t.elements['snaps-panel'].hidden, true, 'Esc closes the Snaps list first');
  assert.equal(t.elements['road-readout'].hidden, false);
  t.key('Escape');
  assert.equal(t.elements['road-readout'].hidden, true);
});

test('a curve control point is guided but not planned', async () => {
  const t = await tools();
  t.key('v');
  t.click(0, 0);
  t.move(50, 50);
  const ask = t.messages.at(-1);
  assert.equal(ask.plan, false);
  t.reply(answer(ask, [0, 0, 48, 48], [], { guides: new Float32Array([0, 0, 48, 48]) }));
  assert.deepEqual(
    [...lastRoad(t).stops],
    [0, 0, 0, 48, 48, -1],
    'a straight guide to the control',
  );
  assert.doesNotMatch(t.elements['build-hint'].textContent, /Cannot build/);
  assert.equal(t.elements['road-readout'].hidden, true);
});

test('the readout flips to the other side of the cursor near the right and bottom edges', async () => {
  const t = await tools();
  t.key('r');
  t.click(0, 0);
  const readout = t.elements['road-readout'];
  const place = (x, y) => {
    t.move(x, y);
    const ask = t.messages.at(-1);
    t.reply(answer(ask, [0, 0, x, y], [0, 0, 0, x, y, 0], { length: 10, angle: 0 }));
    return [readout.style.left, readout.style.top, readout.style.transform];
  };
  assert.deepEqual(place(500, 300), ['516px', '316px', 'translate(0, 0)']);
  assert.deepEqual(place(990, 690), ['974px', '674px', 'translate(-100%, -100%)']);
});

test('angle marks are drawn as the sim sent them and cleared with the tool', async () => {
  const t = await tools();
  t.key('r');
  t.click(0, 0);
  t.move(0, 50);
  const angles = new Float32Array([0, 0, 1, 0, 0, 1]);
  t.reply(answer(t.messages.at(-1), [0, 0, 0, 50], [0, 0, 1, 0, 50, 0], { angles }));
  const marks = () => t.previews.findLast((p) => p.type === 'angles').marks;
  assert.equal(marks(), angles);
  t.key('Escape');
  assert.equal(marks(), null);
});
