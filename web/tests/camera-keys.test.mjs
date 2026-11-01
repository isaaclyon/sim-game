import assert from 'node:assert/strict';
import test from 'node:test';

import { tools } from './main-harness.mjs';

/** Runs one frame `ms` after the previous one; returns the keys' [right, forward, turn] ask. */
function step(t, ms) {
  t.clock.now += ms;
  t.frame();
  const asked = t.cameraPans.at(-1);
  assert.ok(close(asked[3], ms / 1000), 'the camera gets the real frame time to ease over');
  return asked.slice(0, 3);
}
const close = (a, b) => Math.abs(a - b) < 1e-9;
const still = [0, 0, 0];

test('held W/A/S/D/Q/E ask the camera to move until released', async () => {
  const t = await tools();
  assert.deepEqual(step(t, 16), still, 'no keys, no movement');
  for (const [key, expected] of [
    ['w', [0, 1, 0]],
    ['s', [0, -1, 0]],
    ['a', [-1, 0, 0]],
    ['d', [1, 0, 0]],
    ['q', [0, 0, 1]],
    ['e', [0, 0, -1]],
  ]) {
    t.key(key);
    assert.deepEqual(step(t, 16), expected, key);
    t.key(key, { repeat: true });
    assert.deepEqual(step(t, 16), expected, `${key} held`);
    t.keyup(key);
    assert.deepEqual(step(t, 16), still, `${key} released`);
  }
  t.key('W', { shiftKey: true });
  assert.deepEqual(step(t, 16), [0, 1, 0], 'Shift or Caps Lock still moves');
  t.keyup('w');
  assert.deepEqual(step(t, 16), still, 'release matches whatever case the key reports');
  t.key('w');
  t.key('d');
  t.key('q');
  assert.deepEqual(step(t, 16), [1, 1, 1], 'keys combine; the camera evens out diagonals');
  t.key('s');
  t.key('a');
  t.key('e');
  assert.deepEqual(step(t, 16), still, 'opposite keys cancel');
});

test('focus loss and hidden tabs stop the camera at once', async () => {
  const t = await tools();
  t.key('w');
  step(t, 16);
  t.event('blur');
  assert.equal(t.cameraPans.at(-1), 'stop', 'blur stops without a glide');
  assert.deepEqual(step(t, 16), still, 'and releases held keys');
  t.key('e');
  t.hide();
  assert.equal(t.cameraPans.at(-1), 'stop');
  assert.deepEqual(step(t, 60000), still, 'hidden tab releases held keys');
  t.key('a');
  const input = { closest: (selector) => (selector.includes('input') ? input : null) };
  t.event('focusin', { target: input });
  assert.equal(t.cameraPans.at(-1), 'stop');
  assert.deepEqual(step(t, 16), still, 'focusing a text field releases held keys');
  t.event('focusin', { target: { closest: () => null } });
  t.key('w');
  assert.deepEqual(step(t, 16), [0, 1, 0], 'other focus changes keep movement working');
});

test('browser shortcuts, text entry and native buttons keep their keys', async () => {
  const t = await tools();
  for (const options of [{ ctrlKey: true }, { metaKey: true }, { altKey: true }]) {
    t.key('s', options);
    t.key('q', options);
    assert.deepEqual(step(t, 16), still, 'modified keys are browser shortcuts');
  }
  t.key('d');
  t.key('Meta', { metaKey: true });
  assert.deepEqual(step(t, 16), still, 'a modifier drops held keys, which may never report keyup');
  const input = { closest: (selector) => (selector.includes('input') ? input : null) };
  t.key('w', { target: input });
  t.key('e', { target: input });
  assert.deepEqual(step(t, 16), still, 'typing never moves the camera');
  t.key('w', { isComposing: true });
  assert.deepEqual(step(t, 16), still);
  const button = { closest: (selector) => (selector.includes('button') ? button : null) };
  t.key('w', { target: button });
  assert.deepEqual(step(t, 16), [0, 1, 0], 'a focused toolbar button does not block panning');
  const before = t.messages.length;
  t.key(' ', { target: button });
  assert.equal(t.messages.length, before, 'Space still only activates the button');
});

test('panning works paused and with tools, and keeps tool previews under the cursor', async () => {
  const t = await tools();
  t.key(' ');
  assert.equal(t.messages.at(-1).speed, 0);
  t.key('r');
  t.key('w');
  assert.equal(t.elements.road.attributes['aria-pressed'], 'true', 'tool keys still work');
  assert.deepEqual(step(t, 16), [0, 1, 0], 'pans while paused with a tool active');
  t.key('b');
  t.key('v');
  assert.equal(t.elements.curve.attributes['aria-pressed'], 'true');
  t.move(10, 10);
  const asked = t.messages.at(-1);
  assert.equal(asked.type, 'previewSnap');
  t.clock.cameraMoved = true;
  step(t, 16);
  assert.equal(t.messages.at(-1), asked, 'one preview question in flight at a time');
  t.reply({ type: 'roadPreview', revision: asked.revision, stops: new Float32Array([1, 1, 0]) });
  const again = t.messages.at(-1);
  assert.notEqual(again, asked, 'the reply sends the queued question about the moved camera');
  assert.deepEqual([again.type, again.x, again.y], ['previewSnap', 10, 10]);
  const count = t.messages.length;
  step(t, 16);
  step(t, 16);
  assert.equal(t.messages.length, count, 'still one question in flight while moving');
  t.clock.cameraMoved = false;
  t.reply({ type: 'roadPreview', revision: again.revision, stops: new Float32Array([1, 1, 0]) });
  assert.equal(t.messages.length, count + 1, 'the moves while waiting collapse into one');
  t.reply({ type: 'roadPreview', revision: again.revision, stops: new Float32Array([1, 1, 0]) });
  step(t, 16);
  assert.equal(t.messages.length, count + 1, 'a still camera asks nothing');
  t.event('blur');
  t.key('z');
  t.move(20, 20);
  const block = t.messages.at(-1);
  t.reply({ type: 'blockPreview', revision: block.revision, outline: new Float32Array() });
  t.clock.cameraMoved = true;
  step(t, 16);
  assert.equal(t.messages.at(-1).type, 'previewBlock', 'the zone brush outline follows too');
  assert.notEqual(t.messages.at(-1), block);
  t.canvas.events.pointerleave();
  const left = t.messages.length;
  step(t, 16);
  assert.equal(t.messages.length, left, 'nothing to refresh once the cursor leaves');
});

test('keyboard panning during a zone press paints the rectangle it shows', async () => {
  const t = await tools();
  t.key('z');
  t.down(10, 10);
  t.up(10, 10);
  assert.equal(t.messages.at(-1).type, 'fillBlock', 'a still click fills the block');
  t.down(10, 10);
  t.key('q');
  step(t, 16);
  t.keyup('q');
  t.up(10, 10);
  assert.equal(t.messages.at(-1).type, 'paintZone', 'the camera moved, so it was a drag');
  t.down(10, 10);
  t.up(10, 10);
  assert.equal(t.messages.at(-1).type, 'fillBlock', 'the next still click fills again');
});
