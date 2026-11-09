import assert from 'node:assert/strict';
import test from 'node:test';
import { worker } from './worker-harness.mjs';

const replies = (w) => w.messages.filter((m) => m.type === 'building');
async function town() {
  const w = await worker(0, 6);
  w.ready();
  await w.init;
  await w.power();
  await w.water(2);
  for (const [x, zone] of [
    [60, 2],
    [180, 4],
    [300, 6],
  ])
    await w.send({ type: 'fillBlock', x, y: 60, zone });
  for (let i = 0; i < 1500; i++) w.advance(101);
  await w.send({ type: 'setSpeed', speed: 0 });
  return w;
}
function records(w) {
  const { data, ids } = w.messages.findLast((m) => m.type === 'buildings');
  const out = [];
  for (let o = 0, i = 0; o < data.length; o += 6 + 2 * data[o + 5], i++)
    out.push({ id: ids[i], zone: data[o] });
  assert.equal(ids.length, out.length, 'one stable ID per packed snapshot record');
  return out;
}

test('building queries cross WASM and worker with live counts, and replace citizen queries', async () => {
  const w = await town();
  const all = records(w);
  assert.ok(all.some((b) => b.zone === 2));
  assert.ok(all.some((b) => b.zone === 4));
  assert.ok(all.some((b) => b.zone === 6));
  for (const b of all) {
    await w.send({ type: 'inspectBuilding', building: b.id, selection: 1 });
    const { building, selection } = replies(w).at(-1);
    assert.equal(selection, 1);
    assert.equal(building.building, b.id);
    assert.equal(building.zone, b.zone);
    assert.ok(building.capacity > 0);
    assert.ok(building.occupied >= 0 && building.occupied <= building.capacity);
    assert.equal(building.closing, false);
  }
  const selected = all.find((b) => b.zone === 2);
  await w.send({ type: 'inspectBuilding', building: selected.id, selection: 2 });
  const before = replies(w).at(-1).building;
  assert.ok(before.capacity > 7 && before.occupied > 0, 'whole apartment building');
  // A paused repaint refreshes counts immediately without changing the original use.
  await w.send({ type: 'fillBlock', x: 60, y: 60, zone: 6 });
  const closing = replies(w).at(-1).building;
  assert.equal(replies(w).at(-1).selection, 2);
  assert.equal(closing.zone, 2);
  assert.equal(closing.closing, true);
  assert.equal(closing.capacity, before.capacity);
  assert.equal(closing.occupied, before.occupied);
  await w.send({ type: 'setSpeed', speed: 10 });
  // Wide residential lots hold more households, which leave gradually during closure.
  for (let i = 0; i < 2000 && replies(w).at(-1).building !== null; i++) w.advance(101);
  assert.equal(replies(w).at(-1).building, null, 'closure removal clears the selected ID');
  const count = replies(w).length;
  w.advance(101);
  assert.equal(replies(w).length, count, 'demolished buildings stop streaming');
  await w.send({
    type: 'inspectBuilding',
    building: all.find((b) => b.zone === 4).id,
    selection: 3,
  });
  await w.send({ type: 'inspect', agent: 1e6, selection: 4 });
  assert.equal(w.messages.findLast((m) => m.type === 'citizen').citizen, null);
  const cleared = replies(w).length;
  w.advance(101);
  assert.equal(replies(w).length, cleared, 'citizen selection replaces building selection');
  await w.send({ type: 'inspectBuilding', building: 1e6, selection: 5 });
  assert.equal(replies(w).at(-1).building, null, 'unknown stable ID');
  await w.send({ type: 'inspectBuilding', building: null, selection: 6 });
  const end = replies(w).length;
  w.advance(101);
  assert.equal(replies(w).length, end, 'no selection means no queries');
});

test('paused demolition clears building selection without selecting a replacement ID', async () => {
  const w = await town();
  const { id } = records(w)[0];
  await w.send({ type: 'inspectBuilding', building: id, selection: 7 });
  assert.equal(replies(w).at(-1).building.building, id);
  const roads = w.messages.findLast((m) => m.type === 'roads').segments;
  for (let o = 0; o < roads.length && replies(w).at(-1).building; o += 5)
    await w.send({
      type: 'deleteRoad',
      x: (roads[o] + roads[o + 2]) / 2,
      y: (roads[o + 1] + roads[o + 3]) / 2,
    });
  assert.equal(replies(w).at(-1).building, null);
  assert.equal(replies(w).at(-1).selection, 7);
});
