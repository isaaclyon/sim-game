import assert from 'node:assert/strict';
import test from 'node:test';
import * as THREE from 'three';
import { BuildingLayer, RISE_SECONDS } from '../src/building-kit/layer.ts';
import { PREVIEW_REFUSED_COLOR, ToolPreview } from '../src/tool-preview.ts';
import { FLASH_SECONDS, ZoneOverlay } from '../src/zone-overlay.ts';

const house = (x) => [1, 8, 0, 0, 4, x, 0, x + 12, 0, x + 12, 10, x, 10];
const snapshot = (...xs) => new Float32Array(xs.flatMap(house));
const tiles = (layer) => layer.group.children.filter((m) => m.scale.y === 1).length;

test("a city's first buildings appear at once; later growth rises, then joins its tile", () => {
  const layer = new BuildingLayer(new THREE.MeshBasicMaterial());
  layer.update(snapshot(0), new Uint32Array([0]));
  assert.equal(layer.group.children.length, 1, 'the first snapshot does not rise');
  assert.equal(layer.animate(0), false);

  layer.update(snapshot(0, 20), new Uint32Array([0, 1]), 10);
  const rising = layer.group.children.find((m) => m.scale.y < 1);
  assert.ok(rising, 'the new building rises on its own mesh');
  assert.equal(tiles(layer), 1);

  assert.equal(layer.animate(10 + RISE_SECONDS / 2), true);
  assert.ok(rising.scale.y > 0.5 && rising.scale.y < 1, 'half-way up, already past half height');
  const color = rising.geometry.getAttribute('color');
  assert.ok(color.getX(0) > 0.7, 'pale like scaffolding while it rises');

  assert.equal(layer.animate(10.01 + RISE_SECONDS), false);
  assert.equal(layer.group.children.length, 1, 'topped out buildings join their tile');
  assert.equal(layer.group.children[0].geometry.groups.length, 2);
});

test('without a start time, or when demolished mid-rise, nothing is left rising', () => {
  const layer = new BuildingLayer(new THREE.MeshBasicMaterial());
  layer.update(snapshot(0), new Uint32Array([0]));
  layer.update(snapshot(0, 20), new Uint32Array([0, 1]));
  assert.equal(layer.animate(0), false, 'no time means no rise');
  layer.update(snapshot(0, 20, 40), new Uint32Array([0, 1, 2]), 0);
  assert.equal(layer.group.children.length, 2);
  layer.update(snapshot(0, 20), new Uint32Array([0, 1]), 0.1);
  assert.equal(layer.animate(0.2), false);
  assert.equal(layer.group.children.length, 1);
});

test('a rising building can be inspected', () => {
  const layer = new BuildingLayer(new THREE.MeshBasicMaterial());
  layer.update(snapshot(0), new Uint32Array([0]));
  layer.update(snapshot(0, 20), new Uint32Array([0, 7]), 0);
  layer.animate(RISE_SECONDS * 0.9);
  const ray = new THREE.Raycaster(new THREE.Vector3(26, 100, 5), new THREE.Vector3(0, -1, 0));
  assert.equal(layer.pick(ray), 7);
});

test('painted cells flash a lighter shade, then fade to the plain tint', () => {
  const overlay = new ZoneOverlay();
  const layout = [0, 0, 8, 3, 1];
  overlay.setZones(new Uint8Array([1, 0, 0]), layout, 0);
  assert.equal(overlay.group.children.length, 1, "a city's first zones do not flash");
  overlay.setZones(new Uint8Array([1, 0, 0]), layout, 1);
  assert.equal(overlay.group.children.length, 1, 'an unchanged grid does not flash');

  overlay.setZones(new Uint8Array([1, 3, 0]), layout, 2);
  const flash = overlay.group.children[1];
  assert.equal(flash.visible, true);
  const texels = flash.material.map.image.data;
  assert.deepEqual([...texels.subarray(0, 4)], [0, 0, 0, 0], 'unchanged cells stay plain');
  assert.equal(texels[7], 255, 'the painted cell glows');
  assert.ok(texels[4] > 0x7f, 'lighter than the commercial tint');

  assert.equal(overlay.animate(2 + FLASH_SECONDS / 2), true);
  assert.ok(flash.material.opacity > 0 && flash.material.opacity < 0.75);
  assert.equal(overlay.animate(2 + FLASH_SECONDS), false);
  assert.equal(flash.visible, false);
});

const stops = (...xyj) => new Float32Array(xyj);

test('a road preview is a road-wide ribbon; a curve control guide is only a line', () => {
  const preview = new ToolPreview();
  preview.setRoad(stops(0, 0, 0, 40, 0, 1), null, null, 0);
  assert.equal(preview.ribbon.visible, true);
  preview.ribbon.geometry.computeBoundingBox();
  assert.equal(preview.ribbon.geometry.boundingBox.max.z, 4, 'street half-width');
  preview.setRoad(stops(0, 0, 0, 40, 0, 1), null, null, 1);
  preview.ribbon.geometry.computeBoundingBox();
  assert.equal(preview.ribbon.geometry.boundingBox.max.z, 6, 'main road half-width');

  preview.setRoad(stops(0, 0, 0, 40, 0, -1), null, null, 0);
  assert.equal(preview.ribbon.visible, false);
  assert.equal(preview.line.visible, true);

  const a = new THREE.Vector3(0, 0, 0);
  const b = new THREE.Vector3(10, 0, 0);
  preview.setRoad(stops(), a, b, 0);
  assert.equal(preview.ribbon.visible, true, 'refused roads show where they would go');
  assert.ok(preview.ribbon.material.color.equals(PREVIEW_REFUSED_COLOR));

  preview.setBulldoze(new Float32Array([0, 0, 30, 0]));
  assert.ok(preview.ribbon.visible && preview.ribbon.material.color.equals(PREVIEW_REFUSED_COLOR));
  preview.setRoad(null, null, null);
  assert.equal(preview.ribbon.visible || preview.line.visible, false);
});

test('the hover marker glides toward the latest answer, or jumps under reduced motion', () => {
  const preview = new ToolPreview();
  const x = () => preview.markers.geometry.getAttribute('position').getX(0);
  preview.setRoad(stops(0, 0, 0), null, null);
  assert.equal(x(), 0);
  preview.setRoad(stops(10, 0, 0), null, null);
  assert.equal(x(), 0, 'stays put until the frame');
  preview.update(1 / 60, true);
  assert.ok(x() > 0 && x() < 10, `part of the way: ${x()}`);
  for (let i = 0; i < 30; i++) preview.update(1 / 60, true);
  assert.equal(x(), 10, 'arrives');
  preview.setRoad(stops(20, 0, 0), null, null);
  preview.update(1 / 60, false);
  assert.equal(x(), 20);
  preview.setRoad(stops(0, 0, 1, 30, 0, 1), null, null);
  assert.equal(x(), 0, 'a new set of markers appears in place');
});
