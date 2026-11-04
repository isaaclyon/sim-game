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

test("a city's first zones and buildings appear at once; later snapshots flash and rise", async () => {
  const t = await tools();
  const latest = (type) => t.previews.findLast((p) => p.type === type);
  const zones = { type: 'zones', cells: new Uint8Array([1]), layout: [0, 0, 8, 1, 1] };
  const buildings = { type: 'buildings', data: new Float32Array(), ids: new Uint32Array() };
  t.reply(zones);
  t.reply(buildings);
  assert.equal(latest('zones').flash, false);
  assert.equal(latest('buildings').rise, false);
  t.reply(zones);
  t.reply(buildings);
  assert.equal(latest('zones').flash, true);
  assert.equal(latest('buildings').rise, true);
  t.reply({ type: 'cityReset', speed: 1 });
  t.reply(zones);
  t.reply(buildings);
  assert.equal(latest('zones').flash, false, 'a loaded or new city does not flash');
  assert.equal(latest('buildings').rise, false);
});
