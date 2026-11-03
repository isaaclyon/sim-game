import assert from 'node:assert/strict';
import test from 'node:test';
import { ZoneOverlay } from '../src/zone-overlay.ts';
import { tools } from './main-harness.mjs';

test('zone colors and lot lines start hidden, even when new snapshots arrive', () => {
  const overlay = new ZoneOverlay();
  overlay.setZones(new Uint8Array([1]), [0, 0, 10, 1, 1]);
  overlay.setLots(new Float32Array([4, 0, 0, 10, 0, 10, 10, 0, 10]));
  assert.equal(overlay.group.children.length, 2);
  assert.equal(overlay.group.visible, false);
});

test('every zoning brush, including unzone, shows zoning; leaving the tool hides it', async () => {
  const t = await tools();
  const visible = () => t.previews.findLast((p) => p.type === 'zoningVisible')?.visible;
  for (const key of ['z', 'x', 'c', 'u']) {
    t.key(key);
    assert.equal(visible(), true, key);
    t.key('Escape');
    assert.equal(visible(), false);
  }
  for (const key of ['r', 'v', 'b']) {
    t.key('z');
    t.key(key);
    assert.equal(visible(), false, key);
  }
  t.key('z');
  t.click(20, 20, 2);
  assert.equal(visible(), false, 'right-click exits zoning');
});
