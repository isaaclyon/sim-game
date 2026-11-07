import assert from 'node:assert/strict';
import test from 'node:test';

import { WATER_PIECE_STRIDE, WaterKind } from '../src/protocol.ts';
import { tools } from './main-harness.mjs';
import { pumpSpot, worker } from './worker-harness.mjs';

const last = (t, type) => t.messages.findLast((m) => m.type === type);
const pressed = (t, id) => t.elements[id].attributes['aria-pressed'] === 'true';

const STATS = {
  tick: 0,
  day: 1,
  secondOfDay: 8 * 3600,
  population: 0,
  children: 0,
  unemployed: 0,
  unemploymentPercent: 0,
  demand: { residential: 0, commercial: 0, industrial: 0 },
  power: { supply: 0, starter: 150, used: 0, demand: 0, unpowered: 0, waiting: 0 },
  water: { supply: 2000, used: 1000, demand: 1000, stored: 0, capacity: 0, dry: 0, waiting: 0 },
  fire: {
    enabled: true,
    threshold: 1000,
    stations: 0,
    trucks: 0,
    freeTrucks: 0,
    burning: 0,
    unattended: 0,
    covered: 0,
    saved: 0,
    destroyed: 0,
  },
  agents: 0,
  parked: 0,
  awaitingPath: 0,
  entering: 0,
  driving: 0,
  walking: 0,
  walksCompleted: 0,
  tripsCompleted: 0,
  despawnedStuck: 0,
  pathsFailed: 0,
  despawnedByEdit: 0,
  balance: 50_000,
  taxes: 0,
  grant: 0,
  upkeep: 0,
  utilities: 0,
};

function show(t, water) {
  const stats = { ...STATS, water: { ...STATS.water, ...water } };
  t.reply({
    type: 'frame',
    agents: new Float32Array(),
    signals: new Float32Array(),
    tickMs: 0,
    stats,
  });
  t.frame();
}

test('the Water category places pumps and towers the sim was asked about', async () => {
  const t = await tools();
  const view = () => t.previews.findLast((p) => p.type === 'waterView')?.on ?? false;
  t.pressId('category-water');
  t.pressId('water-pump');
  assert.ok(pressed(t, 'water-pump'));
  assert.equal(view(), true, 'a water tool shows the water view');
  t.move(10, 20);
  const ask = last(t, 'previewWater');
  assert.deepEqual([ask.kind, ask.x, ask.y], [WaterKind.Pump, 10, 20]);
  t.reply({
    type: 'waterPreview',
    outline: new Float32Array([0, 0, 1, 0]),
    ok: true,
    revision: ask.revision,
  });
  assert.equal(t.previews.findLast((p) => p.type === 'plant').ok, true);
  t.click(30, 40);
  assert.deepEqual({ ...last(t, 'placeWater') }, { type: 'placeWater', kind: 0, x: 30, y: 40 });
  t.pressId('water-tower');
  t.click(50, 60);
  assert.equal(last(t, 'placeWater').kind, WaterKind.Tower);
  assert.match(t.elements['build-hint'].textContent, /spare water/);
  t.key('Escape');
  assert.ok(!pressed(t, 'water-tower'));
  assert.equal(view(), false, 'putting the tool away hides the view again');
  t.pressId('views');
  t.pressId('water-view');
  assert.equal(view(), true, 'the Views list switches water markers');
  const pieces = new Float32Array(WATER_PIECE_STRIDE);
  t.reply({ type: 'water', pieces, buildings: new Float32Array() });
  assert.equal(t.previews.findLast((p) => p.type === 'water').pieces, pieces);
});

test('water turns red when dense buildings are dry, and its panel says why', async () => {
  const t = await tools();
  const { elements: el } = t;
  show(t, {});
  assert.equal(el['water-value'].textContent, '50%');
  assert.equal(el.water.classList.contains('warning'), false);
  t.pressId('water');
  assert.equal(el['water-panel'].hidden, false);
  assert.equal(el['water-title'].textContent, 'Water');
  show(t, { supply: 500, demand: 2500, stored: 300, capacity: 20000, dry: 3, waiting: 1 });
  assert.equal(el['water-value'].textContent, '500%');
  assert.equal(el.water.classList.contains('warning'), true);
  assert.match(el.water.attributes['aria-label'], /dry/);
  assert.equal(el['water-title'].textContent, 'Not enough water');
  assert.equal(el['water-stored'].textContent, '300 of 20,000');
  assert.equal(el['water-dry'].textContent, '3 dense buildings without water');
  assert.equal(el['water-waiting'].textContent, '1 lot waiting for water');
  assert.match(el['water-advice'].textContent, /Build a pump/);
  show(t, { supply: 0, demand: 0 });
  assert.equal(el['water-value'].textContent, '0%', 'no dense buildings need no water');
  t.pressId('open-water');
  assert.equal(el['water-panel'].hidden, true);
  assert.equal(el['items-water'].hidden, false, 'Open Water opens the Water category');
});

test('the worker places pumps and towers for money, previews them and bulldozes them', async () => {
  const w = await worker(0, 3);
  w.ready();
  await w.init;
  await w.send({ type: 'setSpeed', speed: 0 });
  const first = last(w, 'frame').stats;
  assert.equal(first.water.supply, 0, 'no starter water');
  assert.equal(last(w, 'water').pieces.length, 0, 'water arrives at startup');
  const { Sim } = await import('../src/pkg/sim_wasm.js');
  const mirror = new Sim(1, 3, 3);
  const spot = pumpSpot(mirror);
  mirror.free();
  await w.send({ type: 'previewWater', kind: WaterKind.Pump, ...spot, revision: 7 });
  const preview = w.messages.at(-1);
  assert.deepEqual([preview.type, preview.ok, preview.revision], ['waterPreview', true, 7]);
  assert.equal(preview.outline.length, 10, 'a closed square outline');

  await w.send({ type: 'placeWater', kind: WaterKind.Pump, ...spot });
  const water = last(w, 'water');
  assert.equal(water.pieces.length, WATER_PIECE_STRIDE);
  assert.equal(water.pieces[0], WaterKind.Pump);
  const stats = last(w, 'frame').stats;
  assert.equal(stats.water.supply, 2000, 'a pump on clean ground gives its full output');
  assert.equal(stats.balance, first.balance - 6_000);
  assert.equal(stats.utilities, first.utilities + 40);

  const [, x, y] = water.pieces;
  await w.send({ type: 'deleteRoad', x, y });
  assert.equal(last(w, 'water').pieces.length, 0, 'bulldoze removes the pump');
  assert.equal(last(w, 'frame').stats.utilities, first.utilities);
});
