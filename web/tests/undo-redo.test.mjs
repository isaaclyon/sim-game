import assert from 'node:assert/strict';
import test from 'node:test';

import { tools } from './main-harness.mjs';

const last = (t, type) => t.messages.findLast((m) => m.type === type);

function show(t, history) {
  t.reply({
    type: 'frame',
    agents: new Float32Array(),
    signals: new Float32Array(),
    tickMs: 0,
    stats: { secondOfDay: 0 },
    history,
  });
}

const NONE = { undo: null, redo: null, undoRefund: null, redoCost: null, redoAffordable: false };

test('the undo and redo buttons name the step and its money (ADR 0051)', async () => {
  const t = await tools();
  const { undo, redo } = t.elements;
  assert.equal(undo.disabled, true, 'a city not started yet has nothing to undo');
  show(t, NONE);
  assert.equal(undo.attributes['aria-label'], 'Nothing to undo');
  assert.equal(redo.disabled, true);

  show(t, { ...NONE, undo: 'coal', undoRefund: 20000, redo: 'road', redoCost: 1200 });
  assert.equal(undo.disabled, false);
  assert.equal(undo.attributes['aria-label'], 'Undo: place coal plant');
  assert.match(undo.title, /Gives back \$20,000\.\nKey: Ctrl\+Z$/);
  assert.equal(redo.attributes['aria-label'], 'Redo: build road');
  assert.match(redo.title, /Costs \$1,200, more than the city has\./);
  assert.ok(redo.classList.contains('short'), 'an unaffordable redo is marked');
  show(t, { ...NONE, redo: 'bulldoze', redoCost: 0, redoAffordable: true });
  assert.match(redo.title, /Costs nothing\./);
  assert.ok(!redo.classList.contains('short'));
});

test('buttons and Ctrl+Z, Cmd+Shift+Z send undo and redo in any tool', async () => {
  const t = await tools();
  show(t, { ...NONE, undo: 'road', undoRefund: 500, redo: 'grid', redoCost: 9000 });
  t.pressId('undo');
  assert.equal(last(t, 'undo').id, 1);
  t.key('z', { ctrlKey: true });
  assert.equal(last(t, 'undo').id, 2);
  t.key('Z', { metaKey: true, shiftKey: true });
  assert.equal(last(t, 'redo').id, 3);
  // Not while typing, and Ctrl+Alt+Z is left alone.
  const sent = t.messages.length;
  t.key('z', { ctrlKey: true, target: t.elements['share-url'] });
  t.key('z', { ctrlKey: true, altKey: true });
  assert.equal(t.messages.length, sent);

  // A road being drawn starts again once undo changed the map under it.
  t.key('r');
  t.click(10, 10);
  t.move(30, 10);
  const ask = last(t, 'previewRoad');
  assert.equal(ask.points.length, 6, 'a start clicked, and the cursor');
  t.reply({
    type: 'roadPreview',
    revision: ask.revision,
    points: new Float32Array([10, 10, 30, 10]),
    stops: new Float32Array([10, 10, 0, 30, 10, 0]),
    guides: new Float32Array(),
    angles: new Float32Array(),
    length: 20,
    angle: Number.NaN,
    reclass: new Float32Array(),
  });
  t.key('z', { ctrlKey: true });
  t.reply({ type: 'historyResult', id: 4, ok: true });
  t.move(40, 10);
  assert.equal(last(t, 'previewRoad').points.length, 3, 'the clicked start was dropped');
  assert.equal(t.elements.straight.attributes['aria-pressed'], 'true', 'the tool stays in use');
});

test('a refused undo is marked, and hovering previews what would change', async () => {
  const t = await tools();
  show(t, { ...NONE, undo: 'station', undoRefund: 6000 });
  t.pressId('undo');
  t.reply({ type: 'historyResult', id: 1, ok: false });
  assert.ok(t.elements.undo.classList.contains('refused'));

  t.elements.undo.events.pointerenter();
  const ask = last(t, 'previewHistory');
  assert.equal(ask.redo, false);
  const remove = new Float32Array([2, 0, 0, 10, 0]);
  const restore = new Float32Array();
  t.reply({
    type: 'historyPreview',
    redo: true,
    ok: true,
    remove,
    restore,
    revision: ask.revision,
  });
  assert.equal(
    t.previews.findLast((p) => p.type === 'history'),
    undefined,
    'not this button',
  );
  t.reply({
    type: 'historyPreview',
    redo: false,
    ok: false,
    remove,
    restore,
    revision: ask.revision,
  });
  assert.deepEqual(t.previews.at(-1), { type: 'history', remove, restore, ok: false });
  // A new step asks again for the button still under the pointer.
  show(t, { ...NONE, undo: 'road', undoRefund: 100 });
  assert.ok(last(t, 'previewHistory').revision > ask.revision);
  t.elements.undo.events.pointerleave();
  assert.equal(t.previews.at(-1).remove.length, 0, 'leaving hides it');
  // Undoing the last step disables the button, which ends its preview without a leave.
  t.elements.undo.events.pointerenter();
  const asks = () => t.messages.filter((m) => m.type === 'previewHistory').length;
  const asked = asks();
  show(t, NONE);
  assert.equal(t.previews.at(-1).remove.length, 0);
  show(t, { ...NONE, undo: 'line', undoRefund: 10 });
  assert.equal(asks(), asked, 'a later step does not preview without a hover');
});
