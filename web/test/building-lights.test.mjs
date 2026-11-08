import assert from 'node:assert/strict';
import test from 'node:test';
import * as THREE from 'three';
import { buildBuildingGeometry } from '../src/building-kit/index.ts';
import { seeded } from '../src/building-kit/kit.ts';
import { LIGHT_SEED, lightSchedule } from '../src/building-kit/lights.ts';
import { Lighting, nightMaterial } from '../src/lighting.ts';

const ring = (i) => [
  [i * 23, 0],
  [i * 23 + 18, 0],
  [i * 23 + 18, 14],
  [i * 23, 14],
];
const active = ([on, off], hour) => {
  const elapsed = (hour - on + 24) % 24;
  return elapsed > 0.15 && elapsed < off - on - 0.15;
};

test('whole-building evening shares follow zoning, with varied stable schedules', () => {
  for (let zone = 1; zone <= 6; zone++) {
    const schedules = Array.from({ length: 10000 }, (_, i) =>
      lightSchedule(zone, seeded(ring(i), LIGHT_SEED)),
    );
    const share = schedules.filter((s) => active(s, 20)).length / schedules.length;
    assert.ok(Math.abs(share - [0.7, 0.45, 0.15][Math.floor((zone - 1) / 2)]) < 0.025);
    assert.ok(schedules.filter((s) => active(s, 3)).length < (share * schedules.length) / 2);
    assert.ok(new Set(schedules.map((s) => s[0])).size > 100);
    const ordinary = schedules.filter(([on, off]) => on > 0 && off < 29);
    for (const [on, off] of ordinary) {
      assert.ok(on >= 17 && on < 19);
      const earliest = [23, 21, 20.5][Math.floor((zone - 1) / 2)];
      assert.ok(off >= earliest && off < earliest + 2);
    }
    assert.deepEqual(
      lightSchedule(zone, seeded(ring(1), LIGHT_SEED)),
      lightSchedule(zone, seeded(ring(1), LIGHT_SEED)),
    );
  }
});

test('light participation is independent of wall colour', () => {
  for (const [zone, colours, expected] of [
    [1, 5, 0.7],
    [3, 4, 0.45],
    [5, 3, 0.15],
  ]) {
    const bins = Array.from({ length: colours }, () => [0, 0]);
    for (let i = 0; i < 10000; i++) {
      const footprint = ring(i);
      const bin = bins[Math.floor(seeded(footprint)() * colours)];
      bin[0]++;
      if (lightSchedule(zone, seeded(footprint, LIGHT_SEED))[1] > 0) bin[1]++;
    }
    for (const [total, lit] of bins) assert.ok(Math.abs(lit / total - expected) < 0.05);
  }
});

test('overnight intervals stay lit across midnight and turn off before daylight', () => {
  const schedules = Array.from({ length: 1000 }, (_, i) =>
    lightSchedule(1, seeded(ring(i), LIGHT_SEED)),
  );
  const overnight = schedules.filter(([, off]) => off > 29);
  assert.ok(overnight.length > 0);
  for (const s of overnight) {
    assert.equal(active(s, 23.999), true);
    assert.equal(active(s, 0), true);
    assert.equal(active(s, 7), false);
  }
  const scene = new THREE.Scene();
  const lighting = new Lighting(scene);
  lighting.setTime(23.5 * 3600);
  const clock = lighting.lightHour;
  for (let i = 0; i < 3; i++) lighting.update(scene, new THREE.Vector3(i, 0, 0), 300);
  assert.equal(clock.value, 23.5);
  lighting.setTime(0);
  assert.equal(clock.value, 0);
});

test('all faces in a building share its schedule, including storefronts, across rebuilds', () => {
  const data = new Float32Array([
    3,
    8,
    0,
    0,
    4,
    ...ring(7).flat(),
    2,
    15,
    0,
    0,
    4,
    ...ring(8).flat(),
  ]);
  const g = buildBuildingGeometry(data, undefined, { compact: true });
  const [on, off] = [g.getAttribute('lightOn'), g.getAttribute('lightOff')];
  assert.equal(on.count, g.getAttribute('position').count);
  // Bytes in eighths of an hour: within four clock minutes.
  const hours = (v) => v * (255 / 8);
  for (const [index, group] of g.groups.entries()) {
    const expected = lightSchedule(index === 0 ? 3 : 2, seeded(ring(index + 7), LIGHT_SEED));
    for (let k = group.start; k < group.start + group.count; k++) {
      const i = g.index.getX(k);
      assert.ok(Math.abs(hours(on.getX(i)) - expected[0]) < 1 / 15);
      assert.ok(Math.abs(hours(off.getX(i)) - expected[1]) < 1 / 15);
    }
  }
  const again = buildBuildingGeometry(data, undefined, { compact: true });
  assert.deepEqual(on.data.array, again.getAttribute('lightOn').data.array);
  assert.deepEqual(off.data.array, again.getAttribute('lightOff').data.array);
});

test('scheduled buildings use a separate shader program without changing vehicle lamps', () => {
  const night = { value: 1 },
    clock = { value: 23.5 };
  const buildings = nightMaterial(night, clock),
    vehicles = nightMaterial(night);
  assert.notEqual(buildings.customProgramCacheKey(), vehicles.customProgramCacheKey());
  const shader = {
    uniforms: {},
    vertexShader: '#include <begin_vertex>',
    fragmentShader: 'vec3 totalEmissiveRadiance = emissive;',
  };
  buildings.onBeforeCompile(shader);
  assert.equal(shader.uniforms.lightHour, clock);
  assert.match(shader.vertexShader, /mod\(lightHour - on \+ 24.0, 24.0\)/);
  assert.match(shader.vertexShader, /smoothstep/);
  clock.value = 0;
  assert.equal(shader.uniforms.lightHour.value, 0);
});
