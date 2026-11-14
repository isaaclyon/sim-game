import assert from 'node:assert/strict';
import test from 'node:test';
import { Sim } from '../src/pkg/sim_wasm.js';
import {
  SCHOOL_ROAD_STRIDE,
  SCHOOL_STRIDE,
  SCHOOL_VIEW_STRIDE,
  SchoolArea,
  SchoolKind,
} from '../src/protocol.ts';
import { PRICES, tools } from './main-harness.mjs';
import { worker } from './worker-harness.mjs';

const last = (w, type) => w.messages.findLast((m) => m.type === type);

/** Where a school of `kind` fits beside the generated grid's west edge. */
function schoolSpot(kind) {
  const sim = new Sim(1, 3, 3);
  try {
    for (let y = 30; y < 400; y += 10)
      for (let x = -10; x > -80; x -= 2)
        if (sim.school_preview(kind, x, y)[0] === 1) return { x, y };
  } finally {
    sim.free();
  }
  throw new Error('no room for a school');
}

test('a school is previewed, placed, paid for, drawn, bulldozed and undone', async () => {
  const w = await worker(0);
  w.ready();
  await w.init;
  await w.send({ type: 'setSpeed', speed: 0 });
  assert.equal(last(w, 'schools').schools.length, 0, 'a fresh city has no schools');
  const { prices } = last(w, 'cityReset');
  assert.deepEqual({ ...prices.schools[SchoolKind.Primary] }, { cost: 4000, upkeep: 60 });
  assert.deepEqual({ ...prices.schools[SchoolKind.High] }, { cost: 7000, upkeep: 105 });
  const before = last(w, 'frame').stats;

  const kind = SchoolKind.High;
  const spot = schoolSpot(kind);
  await w.send({ type: 'previewSchool', kind, ...spot, revision: 7 });
  const preview = last(w, 'schoolPreview');
  assert.deepEqual([preview.revision, preview.ok, preview.affordable], [7, true, true]);
  assert.equal(preview.outline.length, 10, 'a closed square');
  await w.send({ type: 'previewSchool', kind, x: 5000, y: 5000, revision: 8 });
  assert.equal(last(w, 'schoolPreview').outline.length, 0, 'no road in reach');

  await w.send({ type: 'placeSchool', kind, ...spot });
  const { schools } = last(w, 'schools');
  assert.equal(schools.length, SCHOOL_STRIDE, '[kind, x, y, half, frontX, frontY]');
  assert.equal(schools[0], kind);
  assert.equal(schools[3], 18, 'a high school site is 36 m');
  const placed = last(w, 'frame').stats;
  assert.equal(placed.balance, before.balance - 7000, 'the school is paid for');
  assert.equal(last(w, 'frame').history.undo, 'high');
  assert.equal(last(w, 'buildings').data[0], 8, 'the school is a building of zone 8');
  await w.send({ type: 'placeSchool', kind, ...spot });
  assert.equal(last(w, 'schools').schools.length, SCHOOL_STRIDE, 'a second cannot overlap');

  const view = last(w, 'schoolView');
  assert.equal(view.schools.length, SCHOOL_VIEW_STRIDE);
  assert.deepEqual([...view.schools.subarray(5, 8)], [kind, 120, 0], 'kind, seats, pupils');
  assert.ok(view.roads.length > 0 && view.roads.length % SCHOOL_ROAD_STRIDE === 0);
  assert.ok(
    view.roads.every((v, i) => i % SCHOOL_ROAD_STRIDE !== 4 || v === SchoolArea.High),
    'roads inside the high school area',
  );
  await w.send({ type: 'inspectBuilding', building: view.schools[8], selection: 3 });
  const { building } = last(w, 'building');
  assert.deepEqual([building.zone, building.seats, building.pupils], [8, 120, 0]);
  assert.equal(last(w, 'frame').stats.education.unschooled, 0, 'no children yet');

  await w.send({ type: 'deleteRoad', x: schools[1], y: schools[2] });
  assert.equal(last(w, 'schools').schools.length, 0, 'bulldozed');
  await w.send({ type: 'undo', id: 1 });
  assert.equal(last(w, 'schools').schools.length, SCHOOL_STRIDE, 'undo puts it back');
  await w.send({ type: 'undo', id: 2 });
  assert.equal(last(w, 'schools').schools.length, 0, 'undoing the placement removes it');
  assert.equal(last(w, 'frame').stats.balance, before.balance, 'and gives the money back');
});

test('the Services menu sells both schools; the tool follows the sim preview', async () => {
  const t = await tools();
  t.reply({ type: 'cityReset', speed: 0, prices: PRICES });
  const hint = () => t.elements['build-hint'].textContent;
  t.pressId('category-services');
  t.pressId('primary-school');
  assert.match(hint(), /^Primary school: \$4,000, then \$60 an hour\. Click beside a road/);
  t.move(30, 40);
  const ask = last(t, 'previewSchool');
  assert.equal(ask.kind, SchoolKind.Primary);
  const outline = new Float32Array([0, 0, 1, 0]);
  const answer = { type: 'schoolPreview', outline, revision: ask.revision };
  t.reply({ ...answer, ok: false, affordable: false });
  assert.match(hint(), /^Not enough money: a primary school costs \$4,000\./);
  assert.equal(t.previews.findLast((p) => p.type === 'plant').ok, false);

  t.pressId('high-school');
  assert.match(hint(), /^High school: \$7,000, then \$105 an hour\./);
  t.click(30, 40);
  const place = last(t, 'placeSchool');
  assert.equal(place.kind, SchoolKind.High);
  const schools = new Float32Array([1, 5, 6, 18, 0, 1]);
  t.reply({ type: 'schools', schools });
  assert.equal(t.previews.findLast((p) => p.type === 'schools').schools, schools);
  t.click(30, 40, 2);
  assert.equal(hint(), '', 'right-click puts the tool away');
});
