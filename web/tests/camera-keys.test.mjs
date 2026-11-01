import assert from 'node:assert/strict';
import test from 'node:test';

import { tools } from './main-harness.mjs';

/** Runs one frame `ms` after the previous one and returns the camera pan it asked for. */
function step(t, ms) {
  t.clock.now += ms;
  const before = t.cameraPans.length;
  t.frame();
  return t.cameraPans.length > before ? t.cameraPans.at(-1) : null;
}
const close = (a, b) => Math.abs(a - b) < 1e-9;

test('held W/A/S/D pans the camera until released, at a frame-rate independent speed', async () => {
  const t = await tools();
  assert.equal(step(t, 16), null, 'no keys, no movement');
  t.key('w');
  const [right, forward] = step(t, 16);
  assert.equal(right, 0);
  assert.ok(forward > 0, 'W moves forward');
  t.key('w', { repeat: true });
  const [, a] = step(t, 8);
  const [, b] = step(t, 8);
  assert.ok(close(a + b, forward), 'two short frames move as far as one long one');
  t.keyup('w');
  assert.equal(step(t, 16), null, 'release stops movement');
  for (const [key, expected] of [
    ['s', [0, -forward]],
    ['a', [-forward, 0]],
    ['d', [forward, 0]],
  ]) {
    t.key(key);
    const pan = step(t, 16);
    assert.ok(close(pan[0], expected[0]) && close(pan[1], expected[1]), `${key} direction`);
    t.keyup(key);
  }
  t.key('W', { shiftKey: true });
  assert.ok(step(t, 16)[1] > 0, 'Shift or Caps Lock still pans');
  t.keyup('w');
  assert.equal(step(t, 16), null, 'release matches whatever case the key reports');
});

test('diagonals are as fast as straight moves and opposite keys cancel', async () => {
  const t = await tools();
  t.key('w');
  const [, straight] = step(t, 16);
  t.key('d');
  const [right, forward] = step(t, 16);
  assert.ok(close(right, forward));
  assert.ok(close(Math.hypot(right, forward), straight), 'diagonal normalised');
  t.key('s');
  t.key('a');
  assert.equal(step(t, 16), null, 'all four cancel out');
  t.keyup('s');
  t.keyup('a');
  t.keyup('d');
  assert.ok(close(step(t, 16)[1], straight));
});

test('focus loss, hidden tabs and long frame gaps cannot leave the camera running', async () => {
  const t = await tools();
  t.key('w');
  const [, normal] = step(t, 16);
  const [, long] = step(t, 5000);
  assert.ok(long <= normal * 10, 'a stalled frame moves at most a tenth of a second');
  t.event('blur');
  assert.equal(step(t, 16), null, 'blur releases held keys');
  t.key('d');
  t.hide();
  assert.equal(step(t, 60000), null, 'hidden tab releases held keys');
  t.key('a');
  const input = { closest: (selector) => (selector.includes('input') ? input : null) };
  t.event('focusin', { target: input });
  assert.equal(step(t, 16), null, 'focusing a text field releases held keys');
  t.event('focusin', { target: { closest: () => null } });
  t.key('w');
  assert.ok(step(t, 16)[1] > 0, 'other focus changes keep movement working');
});

test('browser shortcuts, text entry and native buttons keep their keys', async () => {
  const t = await tools();
  for (const options of [{ ctrlKey: true }, { metaKey: true }, { altKey: true }]) {
    t.key('s', options);
    assert.equal(step(t, 16), null, 'modified keys are browser shortcuts');
  }
  t.key('d');
  t.key('Meta', { metaKey: true });
  assert.equal(step(t, 16), null, 'a modifier drops held keys, which may never report keyup');
  const input = { closest: (selector) => (selector.includes('input') ? input : null) };
  t.key('w', { target: input });
  assert.equal(step(t, 16), null, 'typing never pans');
  t.key('w', { isComposing: true });
  assert.equal(step(t, 16), null);
  const button = { closest: (selector) => (selector.includes('button') ? button : null) };
  t.key('w', { target: button });
  assert.ok(step(t, 16)[1] > 0, 'a focused toolbar button does not block panning');
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
  assert.ok(step(t, 16)[1] > 0, 'pans while paused with a tool active');
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
  t.key('w');
  step(t, 16);
  t.keyup('w');
  t.up(10, 10);
  assert.equal(t.messages.at(-1).type, 'paintZone', 'the camera moved, so it was a drag');
  t.down(10, 10);
  t.up(10, 10);
  assert.equal(t.messages.at(-1).type, 'fillBlock', 'the next still click fills again');
});
