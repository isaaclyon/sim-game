import assert from 'node:assert/strict';
import test from 'node:test';
import * as THREE from 'three';
import { WaterKind } from '../src/protocol.ts';
import { WaterLayer } from '../src/water-layer.ts';
import { createWaterPiece } from '../src/water-pieces.ts';

test('pump and tower models fit the footprint the sim reserves, in one draw call each', () => {
  for (const [kind, half] of [
    [WaterKind.Pump, 7],
    [WaterKind.Tower, 5],
  ]) {
    const piece = createWaterPiece(kind, half);
    const bounds = new THREE.Box3().setFromObject(piece);
    assert.ok(bounds.min.x >= -half && bounds.max.x <= half, `kind ${kind} width`);
    assert.ok(bounds.min.z >= -half && bounds.max.z <= half, `kind ${kind} depth`);
    assert.ok(bounds.max.y > 5, 'stands up from the ground');
    assert.ok(piece.isMesh && piece.castShadow);
  }
  assert.equal(
    createWaterPiece(WaterKind.Tower, 5).geometry,
    createWaterPiece(WaterKind.Tower, 6).geometry,
    'one shared geometry per kind',
  );
});

test('dry buildings always get a marker; the water view marks every network member', () => {
  const layer = new WaterLayer();
  const markers = () => layer.group.children[1].geometry.getAttribute('position').count;
  const pieces = new Float32Array([WaterKind.Pump, 0, 0, 7, 0, 1, 2000, 0, 0]);
  const buildings = new Float32Array([10, 10, 20, 0, 1, 30, 30, 20, -1, 0]);
  layer.set(pieces, buildings);
  assert.equal(layer.group.children[0].children.length, 1, 'one pump model');
  assert.equal(markers(), 1, 'only the dry building');
  layer.setView(true);
  assert.equal(markers(), 3, 'the pump and both buildings');
});
