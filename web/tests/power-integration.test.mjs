import assert from 'node:assert/strict';
import test from 'node:test';
import { tools } from './main-harness.mjs';

const last = (t, type) => t.messages.findLast((m) => m.type === type);
const pressed = (t, id) => t.elements[id].attributes['aria-pressed'] === 'true';

test('the game starts empty and pollution, power and traffic shortcuts remain independent', async () => {
  const t = await tools();
  assert.equal(last(t, 'init').grid, false);
  t.key('g');
  assert.ok(pressed(t, 'plant-coal'));
  assert.ok(pressed(t, 'power-view'), 'a power tool shows the power view');
  for (const key of ['p', 'o', 't']) t.key(key);
  assert.ok(pressed(t, 'plant-coal'), 'view keys do not change the construction tool');
  assert.ok(pressed(t, 'view-air') && !pressed(t, 'power-view') && pressed(t, 'traffic'));
  assert.ok(!pressed(t, 'view-plain'), 'one ground overlay at a time');
  assert.equal(t.previews.findLast((p) => p.type === 'pollutionView').view, 'air');
  assert.equal(t.previews.findLast((p) => p.type === 'powerView').on, false);
  t.click(20, 30);
  assert.equal(last(t, 'placePlant').kind, 0, 'P must not switch from coal to solar');

  const plants = new Float32Array([0, 1, 2, 22, 0, 1, 1, -22, 3000, 0]);
  const cells = new Uint8Array([5, 10, 20, 200]);
  t.reply({ type: 'power', plants, lines: new Float32Array(), buildings: new Float32Array() });
  t.reply({ type: 'pollution', cells, layout: [0, 0, 32, 1, 1] });
  assert.equal(t.previews.findLast((p) => p.type === 'power').plants, plants);
  assert.equal(t.previews.findLast((p) => p.type === 'pollution').cells, cells);
});

test('guided roads, plants and zones own their clicks and discard old tool previews', async () => {
  const t = await tools();
  t.key('r');
  t.click(0, 0);
  t.move(100, 0);
  const road = last(t, 'previewRoad');
  t.key('g');
  assert.ok(!pressed(t, 'straight') && !pressed(t, 'street'));
  assert.equal(t.elements['options-roads'].hidden, true, 'road options go with the road tool');
  const before = t.previews.length;
  t.reply({ type: 'roadPreview', revision: road.revision });
  assert.equal(t.previews.length, before, 'a stale road answer cannot draw over the plant');
  t.move(20, 30);
  const plant = last(t, 'previewPlant');
  t.key('r');
  assert.ok(!pressed(t, 'plant-coal'));
  t.reply({
    type: 'plantPreview',
    revision: plant.revision,
    outline: new Float32Array([1, 2]),
    ok: true,
  });
  assert.equal(t.previews.findLast((p) => p.type === 'plant').outline.length, 0);
  t.click(10, 20);
  t.click(110, 20, 0, true);
  assert.deepEqual([...last(t, 'buildRoad').points], [10, 20, 7, 110, 20, 0]);
  t.key('l');
  t.key('z');
  assert.ok(!pressed(t, 'power-line'));
  t.click(50, 50);
  assert.equal(t.messages.at(-1).type, 'fillBlock');
});

test('replacing a city cancels power construction and ignores its in-flight preview', async () => {
  const t = await tools();
  t.key('l');
  t.click(10, 20);
  t.move(100, 20);
  const pending = last(t, 'previewLine');
  t.reply({ type: 'cityReset', speed: 0 });
  assert.ok(!pressed(t, 'power-line'));
  assert.equal(t.canvas.style.cursor, '');
  t.reply({
    type: 'linePreview',
    revision: pending.revision,
    ends: new Float32Array([10, 20, 1, 100, 20, 1]),
  });
  assert.equal(t.previews.findLast((p) => p.type === 'line').ends, null);
  t.key('l');
  t.click(200, 20);
  assert.equal(last(t, 'buildLine'), undefined, 'the old start cannot join the new city');
  assert.equal(last(t, 'previewLine').ax, 200);
});
