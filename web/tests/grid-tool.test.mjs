import assert from 'node:assert/strict';
import test from 'node:test';

import { RoadClass, Snap } from '../src/protocol.ts';
import { tools } from './main-harness.mjs';

const pressed = (t, id) => t.elements[id].attributes['aria-pressed'] === 'true';
const hint = (t) => t.elements['build-hint'].textContent;
const lastGrid = (t) => t.previews.findLast((p) => p.type === 'grid');
/** The message as plain data: arrays from the page's realm compare by value. */
const plain = (m) => JSON.parse(JSON.stringify(m));
const ALL = Snap.Angle | Snap.Guides | Snap.Length;
const empty = new Float32Array();
/** A sim answer to `previewGrid` with `roads` legs (`[ax, ay, aJoin, bx, by, bJoin]` each). */
const answer = (ask, roads, extra = {}) => ({
  type: 'gridPreview',
  revision: ask.revision,
  points: new Float32Array([0, 0, 80, 0]),
  guides: empty,
  angles: empty,
  length: 80,
  angle: 0,
  cols: 2,
  rows: 1,
  roads: new Float32Array(roads),
  dropped: new Float32Array([0, 80, 80, 80]),
  cost: roads.length ? 2400 : null,
  affordable: roads.length > 0,
  ...extra,
});

test('Grid under Roads asks for the first edge, then the area, and builds one grid (ADR 0049)', async () => {
  const t = await tools();
  t.pressId('category-roads');
  t.pressId('main-road');
  t.pressId('grid-road');
  assert.ok(pressed(t, 'grid-road'));
  assert.ok(pressed(t, 'grid-80'), 'blocks start at 80 m');
  assert.match(hint(t), /Click where the grid starts/);

  t.move(1, 2);
  const start = t.messages.at(-1);
  assert.equal(start.type, 'previewRoad', 'the start is guided as a road start');
  t.click(1, 2);
  // One question is in flight at a time: the next goes once this one is answered.
  t.reply({ type: 'roadPreview', revision: start.revision, stops: empty, points: empty });
  assert.match(hint(t), /first edge/);
  t.move(80, 0);
  const edge = t.messages.at(-1);
  assert.deepEqual(plain(edge), {
    type: 'previewGrid',
    revision: edge.revision,
    class: RoadClass.Main,
    cell: 80,
    points: [1, 2, ALL, 80, 0, ALL],
    reach: 10,
  });
  t.click(80, 0);
  t.reply(answer(edge, []));
  assert.match(hint(t), /whole blocks/);
  t.move(160, 70);
  const area = t.messages.at(-1);
  assert.deepEqual(
    plain(area.points),
    [1, 2, ALL, 80, 0, ALL],
    'the raw clicks, guided by the sim',
  );
  assert.deepEqual(plain(area.cursor), [160, 70]);

  t.reply(answer(area, [0, 0, 0, 80, 0, 1]));
  const drawn = lastGrid(t);
  assert.deepEqual([...drawn.roads], [0, 0, 0, 80, 0, 1]);
  assert.deepEqual([...drawn.dropped], [0, 80, 80, 80], 'left-out edges are drawn too');
  assert.equal(drawn.ok, true);
  assert.equal(drawn.roadClass, RoadClass.Main);
  assert.match(hint(t), /Costs \$2,400/);
  assert.equal(t.elements['road-readout'].textContent, '2 × 1 blocks · 80 m · 0°');

  t.click(160, 70);
  const build = t.messages.at(-1);
  assert.deepEqual(plain(build), {
    type: 'buildGrid',
    id: build.id,
    class: RoadClass.Main,
    cell: 80,
    points: [1, 2, ALL, 80, 0, ALL],
    reach: 10,
    cursor: [160, 70],
  });
  assert.match(hint(t), /Building grid/);
  t.reply({ type: 'buildResult', id: build.id, success: true, end: null });
  assert.match(hint(t), /Click where the grid starts/, 'the next grid starts afresh');
  assert.ok(pressed(t, 'grid-road'), 'the tool stays on');
});

test('a refused grid keeps its clicks, and a block size switches to the grid tool', async () => {
  const t = await tools();
  t.pressId('category-roads');
  t.pressId('street');
  t.pressId('grid-road');
  t.click(0, 0);
  t.click(80, 0);
  t.move(90, 90);
  const ask = t.messages.at(-1);
  t.reply(answer(ask, []));
  assert.equal(lastGrid(t).roads.length, 0);
  assert.match(hint(t), /Nothing to build here/);
  t.click(90, 90);
  const build = t.messages.at(-1);
  t.reply({ type: 'buildResult', id: build.id, success: false, end: null });
  assert.match(hint(t), /Nothing to build here.*whole blocks/, 'still choosing the area');

  t.pressId('straight');
  assert.equal(pressed(t, 'grid-road'), false);
  t.pressId('block-size');
  t.pressId('grid-112');
  assert.ok(pressed(t, 'grid-road'), 'a block size starts the grid tool');
  assert.ok(pressed(t, 'grid-112'));
  assert.equal(pressed(t, 'grid-80'), false);
  t.click(0, 0);
  t.move(50, 0);
  assert.equal(t.messages.at(-1).cell, 112);
});
