import assert from 'node:assert/strict';
import test from 'node:test';

import { tools } from './main-harness.mjs';

test('ambient sound hears the camera every frame and gets new buildings', async () => {
  const t = await tools();
  t.frame();
  assert.deepEqual(t.sound.updates.at(-1)[3], { x: 1, y: 2, distance: 300 }, 'where it looks');
  assert.equal(t.sound.updates.at(-1)[2], 12 * 3600, 'noon until the sim reports its clock');
  const data = new Float32Array([1, 8, 0, 3, 0, 0, 1, 0, 0, 1]);
  t.reply({ type: 'buildings', data });
  assert.equal(t.sound.buildings.at(-1), data);
});

test('N turns ambient sound on and off', async () => {
  const t = await tools();
  t.key('n');
  t.key('N');
  assert.equal(t.sound.toggles, 2);
  t.key('n', { repeat: true });
  assert.equal(t.sound.toggles, 2, 'holding N does not flicker the sound');
});

test('T switches cars between paint and traffic colours', async () => {
  const t = await tools();
  t.key('t');
  assert.equal(t.elements.traffic.attributes['aria-pressed'], 'true');
  t.key('T');
  assert.equal(t.elements.traffic.attributes['aria-pressed'], 'false');
  t.elements.traffic.onclick();
  assert.equal(t.elements.traffic.attributes['aria-pressed'], 'true', 'the button does the same');
});
