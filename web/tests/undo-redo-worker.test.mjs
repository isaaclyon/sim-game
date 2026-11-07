import assert from 'node:assert/strict';
import test from 'node:test';
import { worker } from './worker-harness.mjs';

const last = (w, type) => w.messages.findLast((m) => m.type === type);

test('undo and redo take a road back and build it again, with its money (ADR 0051)', async () => {
  const w = await worker(0);
  w.ready();
  await w.init;
  await w.send({ type: 'setSpeed', speed: 0 });
  const start = last(w, 'frame').stats.balance;
  assert.deepEqual(
    { ...last(w, 'frame').history },
    {
      undo: null,
      redo: null,
      undoRefund: null,
      redoCost: null,
      redoAffordable: false,
    },
  );
  await w.send({ type: 'buildRoad', id: 1, points: [-1000, -1000, 0, -800, -1000, 0], class: 0 });
  const built = last(w, 'roads').segments.length;
  const cost = start - last(w, 'frame').stats.balance;
  assert.ok(cost > 0);
  assert.deepEqual(
    { ...last(w, 'frame').history },
    {
      undo: 'road',
      redo: null,
      undoRefund: cost,
      redoCost: null,
      redoAffordable: false,
    },
  );

  await w.send({ type: 'previewHistory', redo: false, revision: 4 });
  const preview = last(w, 'historyPreview');
  assert.equal(preview.ok, true);
  assert.equal(preview.revision, 4);
  assert.deepEqual([...preview.remove], [2, -1000, -1000, -800, -1000]);
  assert.equal(preview.restore.length, 0);

  await w.send({ type: 'undo', id: 7 });
  assert.deepEqual({ ...last(w, 'historyResult') }, { type: 'historyResult', id: 7, ok: true });
  assert.ok(last(w, 'roads').segments.length < built, 'the road is gone at once, paused');
  const frame = last(w, 'frame');
  assert.equal(frame.stats.balance, start);
  assert.equal(frame.history.redo, 'road');
  assert.equal(frame.history.redoAffordable, true);

  await w.send({ type: 'undo', id: 8 });
  assert.equal(last(w, 'historyResult').ok, false, 'nothing left to undo');
  await w.send({ type: 'redo', id: 9 });
  assert.equal(last(w, 'historyResult').ok, true);
  assert.equal(last(w, 'roads').segments.length, built);
  assert.equal(last(w, 'frame').stats.balance, start - cost);
});
