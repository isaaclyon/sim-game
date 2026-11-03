import assert from 'node:assert/strict';
import test from 'node:test';
import * as THREE from 'three';
import { buildBuildingGeometry } from '../src/building-kit/index.ts';
import { daylight, Lighting, nightMaterial } from '../src/lighting.ts';
import { createSimScene } from '../src/scene.ts';
import { VEHICLES } from '../src/vehicle-models.ts';

test('daylight is smooth through dawn, dusk and midnight', () => {
  assert.equal(daylight(0), 0);
  assert.equal(daylight(43200), 1);
  assert.equal(daylight(21600), 0.5);
  assert.equal(daylight(64800), 0.5);
  for (let t = 0; t <= 86400; t += 60) {
    assert.ok(daylight(t) >= 0 && daylight(t) <= 1);
    assert.ok(Math.abs(daylight(t) - daylight(t - 1)) < 0.001);
    assert.equal(daylight(t), daylight(t + 86400));
  }
});

test('paused lighting holds time while shadows follow the reflected camera focus', () => {
  const scene = createSimScene();
  const lighting = new Lighting(scene);
  const focus = new THREE.Vector3(120, 0, -90);
  lighting.setTime(43200);
  lighting.update(scene, focus, 300);
  assert.equal(lighting.night.value, 0);
  assert.equal(lighting.sun.castShadow, true);
  const direction = lighting.sun.position.clone().sub(lighting.sun.target.position);
  focus.x += 50;
  lighting.update(scene, focus, 300);
  assert.ok(
    direction.distanceTo(lighting.sun.position.clone().sub(lighting.sun.target.position)) < 1e-9,
  );
  assert.equal(lighting.sun.target.position.z, 90);
  assert.equal(lighting.sun.target.position.x, 170);
  lighting.setTime(0);
  lighting.update(scene, focus, 300);
  assert.equal(lighting.night.value, 1);
  assert.equal(lighting.sun.intensity, 0);
  assert.equal(lighting.sun.castShadow, false);
  assert.ok(lighting.ambient.intensity > 0);
});

test('buildings emit only on window faces, with stable geometry across rebuilds', () => {
  const data = new Float32Array([2, 15, 0, 0, 4, 0, 0, 18, 0, 18, 14, 0, 14]);
  const g = buildBuildingGeometry(data);
  const glow = g.getAttribute('nightEmission');
  assert.equal(glow.count, g.getAttribute('position').count);
  assert.ok(glow.array.some((v) => v > 0));
  const normal = g.getAttribute('normal');
  for (let i = 0; i < glow.count; i++)
    if (glow.getX(i) > 0) assert.ok(Math.abs(normal.getY(i)) < 1e-6);
  assert.deepEqual(glow.array, buildBuildingGeometry(data).getAttribute('nightEmission').array);
});

test('every vehicle has forward white lamps and rear red lamps at both detail levels', () => {
  for (const v of VEHICLES)
    for (const geometry of [v.trim, v.far]) {
      const g = geometry();
      const glow = g.getAttribute('nightEmission');
      const p = g.getAttribute('position');
      let front = 0,
        rear = 0;
      for (let i = 0; i < glow.count; i++) {
        if (glow.getX(i) === 0) continue;
        if (p.getX(i) > 0) {
          front++;
          assert.ok(glow.getY(i) > 0.5);
        } else {
          rear++;
          assert.ok(glow.getY(i) < 0.1);
        }
      }
      assert.ok(front > 0 && rear > 0, v.name);
    }
});

test('emission shader keeps the shared clock uniform for later updates', () => {
  const night = { value: 0 };
  const shader = {
    uniforms: {},
    vertexShader: '#include <begin_vertex>',
    fragmentShader: 'vec3 totalEmissiveRadiance = emissive;',
  };
  nightMaterial(night).onBeforeCompile(shader);
  night.value = 1;
  assert.equal(shader.uniforms.night.value, 1);
  assert.match(shader.vertexShader, /vNightEmission = nightEmission/);
  assert.match(shader.fragmentShader, /vNightEmission \* night/);
});
