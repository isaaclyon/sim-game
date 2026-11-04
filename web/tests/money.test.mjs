import assert from 'node:assert/strict';
import test from 'node:test';

import { RoadClass, Snap } from '../src/protocol.ts';
import { tools } from './main-harness.mjs';
import { worker } from './worker-harness.mjs';

test('a fresh game city is an empty map with starting funds, and roads are quoted and charged', async () => {
  const w = await worker(0, 20, undefined, false);
  w.ready();
  await w.init;
  const roads = w.messages.findLast((msg) => msg.type === 'roads');
  assert.equal(roads.segments.length, 0, 'no roads to start with');
  const [x0, y0, x1, y1] = roads.bounds;
  assert.ok(x0 <= 0 && y0 <= 0 && x1 >= 19 * 120 && y1 >= 19 * 120, 'the map has a size');
  const frame = () => w.messages.findLast((msg) => msg.type === 'frame');
  const start = frame().stats.balance;
  assert.ok(start > 0);
  assert.equal(frame().stats.upkeep, 0);

  // Snap flags 0: the points stay where they are drawn.
  const street = { points: [0, 0, 0, 1000, 0, 0], class: RoadClass.Street };
  await w.send({ type: 'previewRoad', revision: 3, plan: true, ...street });
  const quote = w.messages.at(-1);
  assert.equal(quote.type, 'roadPreview');
  assert.deepEqual([quote.cost, quote.affordable], [10_000, true]);
  // The price is for the guided road: length snapping turns 1,003 m into 1,000 m.
  const snapped = [0, 0, Snap.Length, 1003, 0, Snap.Length];
  await w.send({ type: 'previewRoad', revision: 3, plan: true, class: 0, points: snapped });
  assert.equal(w.messages.at(-1).cost, 10_000);
  await w.send({ type: 'setSpeed', speed: 0 });
  await w.send({ type: 'buildRoad', id: 1, ...street });
  assert.equal(w.messages.findLast((m) => m.type === 'buildResult').success, true);
  assert.equal(frame().stats.balance, start - 10_000, 'paused builds publish the new balance');
  assert.equal(frame().stats.upkeep, 100);

  const main = { points: [0, 500, 0, 9000, 500, 0], class: RoadClass.Main };
  await w.send({ type: 'previewRoad', revision: 4, plan: true, ...main });
  assert.deepEqual([w.messages.at(-1).cost, w.messages.at(-1).affordable], [225_000, false]);
  await w.send({ type: 'buildRoad', id: 2, ...main });
  assert.equal(w.messages.findLast((m) => m.type === 'buildResult').success, false);
  assert.equal(frame().stats.balance, start - 10_000);
});

const stats = {
  day: 1,
  secondOfDay: 21600,
  population: 10,
  unemployed: 0,
  unemploymentPercent: 0,
  demand: { residential: 0, commercial: 0, industrial: 0 },
  agents: 0,
  parked: 0,
  driving: 0,
  walking: 0,
  walksCompleted: 0,
  tripsCompleted: 0,
  despawnedStuck: 0,
  pathsFailed: 0,
  despawnedByEdit: 0,
  balance: 12_345,
  taxes: 300,
  upkeep: 120,
};

function show(t, changes) {
  t.reply({
    type: 'frame',
    agents: new Float32Array(),
    signals: new Float32Array(),
    tickMs: 0,
    stats: { ...stats, ...changes },
  });
  t.frame();
}

test('the HUD shows the balance, net income and debt', async () => {
  const t = await tools();
  show(t, {});
  assert.match(
    t.elements.hud.textContent,
    /money {6}\$12,345 {3}\+\$180\/h \(taxes \$300, upkeep \$120\)\n/,
  );
  show(t, { balance: -40, taxes: 0, upkeep: 120 });
  assert.match(t.elements.hud.textContent, /money {6}-\$40 {3}-\$120\/h .*in debt: no new roads/);
});

test('the road tool shows what a road costs and says when the city cannot pay', async () => {
  const t = await tools();
  t.key('r');
  t.click(0, 0);
  t.move(100, 0);
  const ask = t.messages.at(-1);
  assert.equal(ask.type, 'previewRoad');
  assert.equal(ask.class, RoadClass.Street);
  const stops = new Float32Array([0, 0, 0, 100, 0, 0]);
  const preview = (revision, cost, affordable) => ({
    type: 'roadPreview',
    revision,
    stops,
    points: new Float32Array([0, 0, 100, 0]),
    guides: new Float32Array(),
    angles: new Float32Array(),
    length: 100,
    angle: Number.NaN,
    cost,
    affordable,
  });
  t.reply(preview(ask.revision, 1000, true));
  assert.match(t.elements['road-help'].textContent, /Costs \$1,000\. Click the road end/);

  t.key('m');
  const again = t.messages.at(-1);
  assert.equal(again.type, 'previewRoad', 'switching class asks for a new quote');
  assert.equal(again.class, RoadClass.Main);
  t.reply(preview(again.revision, 2500, false));
  assert.match(t.elements['road-help'].textContent, /Not enough money: costs \$2,500\./);
  t.click(100, 0);
  const build = t.messages.at(-1);
  assert.equal(build.type, 'buildRoad');
  t.reply({ type: 'buildResult', id: build.id, success: false });
  const help = t.elements['road-help'].textContent;
  assert.match(help, /Not enough money/);
  assert.doesNotMatch(help, /Cannot build here/, 'refused for money, not for its shape');
});
