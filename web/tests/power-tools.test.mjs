import assert from 'node:assert/strict';
import test from 'node:test';

import { PLANT_STRIDE, PlantKind } from '../src/protocol.ts';
import { tools } from './main-harness.mjs';
import { coalSpot, worker } from './worker-harness.mjs';

const last = (t, type) => t.messages.findLast((m) => m.type === type);
const pressed = (t, id) => t.elements[id].attributes['aria-pressed'] === 'true';

test('P cycles coal, solar and wind; clicks place the plant the sim was asked about', async () => {
  const t = await tools();
  t.key('p');
  assert.ok(pressed(t, 'plant-coal'));
  t.move(10, 20);
  const ask = last(t, 'previewPlant');
  assert.deepEqual([ask.kind, ask.x, ask.y], [PlantKind.Coal, 10, 20]);
  t.reply({
    type: 'plantPreview',
    outline: new Float32Array([0, 0, 1, 0]),
    ok: false,
    revision: ask.revision,
  });
  assert.equal(t.previews.findLast((p) => p.type === 'plant').ok, false);
  t.click(30, 40);
  assert.deepEqual(
    { ...last(t, 'placePlant') },
    {
      type: 'placePlant',
      kind: PlantKind.Coal,
      x: 30,
      y: 40,
    },
  );
  t.key('P');
  assert.ok(pressed(t, 'plant-solar') && !pressed(t, 'plant-coal'));
  t.key('p');
  assert.ok(pressed(t, 'plant-wind'));
  t.key('p');
  assert.ok(!pressed(t, 'plant-wind'), 'a fourth press stops placing');
  t.key('p');
  t.key('r');
  assert.ok(!pressed(t, 'plant-coal'), 'another tool switches the plant tool off');
  t.key('l');
  t.key('Escape');
  assert.ok(!pressed(t, 'power-line'), 'Esc stops the line tool');
});

test('a line starts on click, follows the sim preview, and chains from the snapped end', async () => {
  const t = await tools();
  t.key('l');
  t.click(0, 0);
  assert.match(t.elements['road-help'].textContent, /line end/);
  // The click asked about its own point; the move waits for that answer.
  const first = last(t, 'previewLine');
  t.move(100, 0);
  t.reply({ type: 'linePreview', ends: new Float32Array(), revision: first.revision });
  const ask = last(t, 'previewLine');
  assert.deepEqual([ask.ax, ask.ay, ask.bx, ask.by], [0, 0, 100, 0]);
  t.reply({
    type: 'linePreview',
    ends: new Float32Array([0, 0, 1, 98, 0, 1]),
    revision: ask.revision,
  });
  assert.equal(t.previews.findLast((p) => p.type === 'line').ends.length, 6);
  t.click(100, 0);
  assert.deepEqual(
    { ...last(t, 'buildLine') },
    { type: 'buildLine', ax: 0, ay: 0, bx: 100, by: 0 },
  );
  const pending = last(t, 'previewLine');
  t.move(200, 0);
  t.reply({ type: 'linePreview', ends: new Float32Array(), revision: pending.revision });
  const next = last(t, 'previewLine');
  assert.deepEqual(
    [next.ax, next.ay],
    [98, 0],
    'the next span starts where the sim ended this one',
  );
  t.reply({ type: 'linePreview', ends: new Float32Array(), revision: next.revision });
  const before = t.messages.length;
  t.click(200, 0);
  assert.equal(
    t.messages.filter((m, i) => i >= before && m.type === 'buildLine').length,
    0,
    'a refused line is not sent',
  );
  assert.match(t.elements['road-help'].textContent, /Cannot build a line there/);
  t.click(200, 0, 2);
  assert.ok(!pressed(t, 'power-line'), 'right-click stops');
});

test('power snapshots reach the renderer and O toggles the power view', async () => {
  const t = await tools();
  const plants = new Float32Array([0, 1, 2, 22, 0, 1, 1, -22, 3000, 0]);
  t.reply({ type: 'power', plants, lines: new Float32Array(), buildings: new Float32Array() });
  assert.equal(t.previews.findLast((p) => p.type === 'power').plants, plants);
  t.key('o');
  assert.equal(t.previews.findLast((p) => p.type === 'powerView').on, true);
  assert.ok(pressed(t, 'power-view'));
});

test('the worker places plants and lines, answers previews and reports power', async () => {
  const w = await worker(0, 3);
  w.ready();
  await w.init;
  await w.send({ type: 'setSpeed', speed: 0 });
  assert.equal(last(w, 'frame').stats.power.starter, 150);
  await w.send({ type: 'previewPlant', kind: PlantKind.Solar, x: 120, y: 0, revision: 3 });
  const refused = w.messages.at(-1);
  assert.deepEqual([refused.type, refused.ok, refused.revision], ['plantPreview', false, 3]);
  assert.equal(refused.outline.length, 10, 'a closed square outline');

  await w.power();
  const power = last(w, 'power');
  assert.equal(power.plants.length, PLANT_STRIDE);
  assert.equal(power.plants[0], PlantKind.Coal);
  const [fx, fy] = power.plants.subarray(4, 6);
  assert.ok(Math.abs(Math.hypot(fx, fy) - 1) < 1e-5, 'faces its street along a unit direction');
  assert.ok(last(w, 'frame').stats.power.supply > 0);

  const { Sim } = await import('../src/pkg/sim_wasm.js');
  const mirror = new Sim(1, 3, 3);
  const coal = coalSpot(mirror);
  mirror.free();
  const line = { ax: coal.x, ay: coal.y, bx: coal.x - 80, by: coal.y };
  await w.send({ type: 'previewLine', ...line, revision: 4 });
  const preview = w.messages.at(-1);
  assert.equal(preview.type, 'linePreview');
  assert.deepEqual(
    [...preview.ends].filter((_, i) => i % 3 === 2),
    [1, 0],
    'joins the plant',
  );
  assert.deepEqual([...preview.ends.subarray(0, 2)], [...power.plants.subarray(6, 8)]);
  await w.send({ type: 'buildLine', ...line });
  assert.equal(last(w, 'power').lines.length, 4);
  const [ax, ay, bx, by] = last(w, 'power').lines;
  await w.send({ type: 'deleteRoad', x: (ax + bx) / 2, y: (ay + by) / 2 + 1 });
  assert.equal(last(w, 'power').lines.length, 0, 'bulldozing takes the line first');
});
