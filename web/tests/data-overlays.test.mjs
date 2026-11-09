import assert from 'node:assert/strict';
import test from 'node:test';
import { DataOverlays, easeJams, gapColor, jamColor } from '../src/data-overlays.ts';
import { SERVICES } from '../src/palette.ts';
import { Gap, TRAFFIC_STRIDE } from '../src/protocol.ts';
import { worker } from './worker-harness.mjs';

test('roads read green when flowing, yellow when slow and red when jammed', () => {
  const [free, slow, stuck] = [jamColor(0), jamColor(0.25), jamColor(1)];
  assert.ok(free.g > free.r, 'green');
  assert.ok(slow.r > 0.5 && slow.g > 0.3, 'yellow');
  assert.ok(stuck.r > stuck.g * 2, 'red');
  assert.deepEqual(jamColor(2), stuck, 'beyond full stays red');
});

test('jams ease toward each new reading, and roads without cars drop out', () => {
  const first = easeJams(new Map(), new Float32Array([0, 0, 1, 0, 0, 7, 1, 1, 0, 2, 0, 0, 7, 1]));
  assert.deepEqual([...first], [[7, 1]], 'a road shows its first reading at once');
  const second = easeJams(first, new Float32Array([0, 0, 1, 0, 0, 7, 0, 0, 0, 1, 0, 0, 9, 0.4]));
  assert.ok(Math.abs(second.get(7) - 0.5) < 1e-6, 'half way toward the light turning green');
  assert.equal(second.get(9), Math.fround(0.4));
  assert.equal(easeJams(second, new Float32Array()).size, 0);
});

test('lots read green with every service, amber missing one and red missing more', () => {
  assert.equal(gapColor(0), SERVICES.served);
  assert.equal(gapColor(Gap.Power), SERVICES.short);
  assert.equal(gapColor(Gap.Fire), SERVICES.short);
  assert.equal(gapColor(Gap.Water | Gap.Fire), SERVICES.without);
});

test('each data view shows only its own layer, over the plain haze', () => {
  const o = new DataOverlays();
  const [pollution, traffic, services] = o.group.children;
  o.setTraffic(new Float32Array([0, 0, 100, 0, 1, 3, 0.2]));
  assert.equal(traffic.geometry.getAttribute('position').count, 6, 'one quad');
  const xs = Array.from({ length: 6 }, (_, i) => traffic.geometry.getAttribute('position').getX(i));
  assert.equal(Math.min(...xs), -6.5, 'reaches past the end, a main road wide');
  o.setServices(new Float32Array([0, 4, 0, 0, 10, 0, 10, 10, 0, 10]));
  assert.equal(
    services.geometry.getAttribute('position').count,
    6,
    'a square lot is two triangles',
  );
  o.setView('traffic');
  assert.ok(traffic.visible && !services.visible);
  assert.equal(pollution.children[0], undefined, 'no pollution grid yet');
  o.setView('services');
  assert.ok(services.visible && !traffic.visible);
  o.setView('land');
  assert.ok(!services.visible && !traffic.visible);
});

test('the worker sends traffic once a real second and service gaps as the city changes', async () => {
  const w = await worker(300);
  w.ready();
  await w.init;
  const of = (type) => w.messages.filter((m) => m.type === type);
  assert.equal(of('traffic').length, 1, 'sent at start');
  assert.equal(of('services').length, 1);
  for (let i = 0; i < 50; i++) w.advance(101);
  const sent = of('traffic');
  assert.ok(sent.length >= 5 && sent.length <= 6, `about once a second: ${sent.length}`);
  const { segments } = sent.at(-1);
  assert.ok(segments.length > 0 && segments.length % TRAFFIC_STRIDE === 0, 'cars are driving');
  for (let i = 6; i < segments.length; i += TRAFFIC_STRIDE)
    assert.ok(segments[i] >= 0 && segments[i] <= 1);

  await w.send({ type: 'fillBlock', x: 50, y: 50, zone: 1 });
  for (let i = 0; i < 600; i++) w.advance(101);
  const { lots } = of('services').at(-1);
  let buildings = 0;
  for (let i = 0; i < lots.length; i += 2 + 2 * lots[i + 1]) {
    assert.ok(lots[i + 1] >= 3, 'a lot outline');
    buildings++;
  }
  assert.ok(buildings > 0, 'homes grew and report their services');
});
