import assert from 'node:assert/strict';
import test from 'node:test';
import {
  CAR_DETAIL_PX,
  Detail,
  detail,
  pixelsPerMetre,
  WALKER_DETAIL_PX,
  WALKER_HIDE_PX,
} from '../src/agent-lod.ts';
import { VEHICLES, vehicleModel, vehiclePaint } from '../src/vehicle-models.ts';
import { FIGURE, hairStyle, sizeOf, swing } from '../src/walker-figure.ts';

const bounds = (g) => {
  g.computeBoundingBox();
  return g.boundingBox;
};

test('every vehicle fits a queue slot, stands on the road and faces +x', () => {
  for (const v of VEHICLES) {
    const body = bounds(v.body());
    const trim = bounds(v.trim());
    const min = body.min.clone().min(trim.min);
    const max = body.max.clone().max(trim.max);
    assert.ok(Math.abs(min.y) < 1e-6, `${v.name} stands on the road`);
    // The sim queues cars 7 m front to front, so anything longer would overlap its neighbour.
    assert.ok(max.x - min.x <= 6.1 && max.x - min.x >= 3.9, `${v.name} is ${max.x - min.x} m long`);
    assert.ok(max.z - min.z <= 2.3, `${v.name} fits a lane`);
    assert.ok(Math.abs(max.x + min.x) < 0.2, `${v.name} is centred on its agent`);
  }
});

test('a citizen always gets the same car, and every model is on the road', () => {
  const seen = new Map();
  for (let id = 0; id < 20000; id++) {
    const m = vehicleModel(id);
    assert.equal(vehicleModel(id), m);
    assert.equal(vehiclePaint(id), vehiclePaint(id));
    seen.set(m, (seen.get(m) ?? 0) + 1);
  }
  const total = VEHICLES.reduce((s, v) => s + v.weight, 0);
  VEHICLES.forEach((v, m) => {
    const share = (seen.get(m) ?? 0) / 20000;
    assert.ok(Math.abs(share - v.weight / total) < 0.02, `${v.name} share ${share}`);
  });
});

test('walkers are person-sized and only swing their legs while moving', () => {
  const head = bounds(FIGURE.head());
  const leg = bounds(FIGURE.leg());
  assert.ok(head.max.y > 1.6 && head.max.y < 1.9);
  assert.ok(Math.abs(leg.min.y + 0.9) < 1e-6 && leg.max.y === 0, 'legs hang from the hip');
  assert.deepEqual(swing(3.2, 0, 7), [0, -0], 'standing still');
  const strides = Array.from({ length: 20 }, (_, i) => swing(i * 0.2, 1, 7)[0]);
  assert.ok(Math.max(...strides) > 0.4 && Math.min(...strides) < -0.4, 'a full stride');
  const [legSwing, armSwing] = swing(0.4, 1, 7);
  assert.ok(Math.sign(legSwing) === -Math.sign(armSwing), 'arms swing against legs');
});

test('walkers vary in hair and size, and keep it for life', () => {
  const styles = [0, 0, 0];
  for (let id = 0; id < 9000; id++) {
    styles[hairStyle(id)]++;
    const [height, build] = sizeOf(id);
    assert.deepEqual(sizeOf(id), [height, build]);
    assert.ok(height >= 0.92 && height <= 1.08 && build >= 0.9 && build <= 1.1);
  }
  assert.ok(
    styles.every((n) => n > 600),
    `hair styles ${styles}`,
  );
  const hair = bounds(FIGURE.hair());
  const head = bounds(FIGURE.head());
  assert.ok(hair.max.y > head.max.y, 'hair sits on top of the head');
});

test('far cars and walkers draw simpler, and the farthest walkers not at all', () => {
  const scale = pixelsPerMetre({ fov: 45 }, 1000);
  assert.equal(detail(4.6, 100, scale, CAR_DETAIL_PX), Detail.Full);
  assert.equal(detail(4.6, 1000, scale, CAR_DETAIL_PX), Detail.Simple);
  assert.equal(detail(4.6, 1e6, scale, CAR_DETAIL_PX), Detail.Simple, 'cars never vanish');
  const walker = (d) => detail(1.75, d, scale, WALKER_DETAIL_PX, WALKER_HIDE_PX);
  assert.deepEqual([walker(50), walker(300), walker(2000)], [2, 1, 0]);
  for (const v of VEHICLES) {
    const far = v.far().getAttribute('position').count / 3;
    const near =
      (v.body().getAttribute('position').count + v.trim().getAttribute('position').count) / 3;
    assert.ok(far <= near / 5, `${v.name} far model is ${far} triangles`);
  }
});
