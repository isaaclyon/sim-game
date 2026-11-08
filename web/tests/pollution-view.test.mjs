import assert from 'node:assert/strict';
import test from 'node:test';
import { PollutionOverlay, pollutionColor } from '../src/pollution-overlay.ts';
import { tools } from './main-harness.mjs';
import { worker } from './worker-harness.mjs';

test('clean land draws nothing except as land value, which reads as high', () => {
  const clean = [0, 0, 0, 255];
  for (const view of ['plain', 'air', 'ground', 'noise'])
    assert.equal(pollutionColor(view, clean)[3], 0, view);
  const [r, g, , a] = pollutionColor('land', clean);
  assert.ok(a > 0 && g > r, 'green');
  const [lr, lg] = pollutionColor('land', [0, 0, 0, 0]);
  assert.ok(lr > lg, 'worthless land is red');
});

test('each view shows only its own kind, stronger where it is worse', () => {
  const noisy = [0, 0, 200, 100];
  assert.equal(pollutionColor('air', noisy)[3], 0);
  assert.equal(pollutionColor('ground', noisy)[3], 0);
  assert.ok(pollutionColor('noise', noisy)[3] > pollutionColor('noise', [0, 0, 100, 100])[3]);
});

test('the happiness view shows nothing where nobody lives, blue where residents are happy', () => {
  const clean = [0, 0, 0, 255];
  assert.equal(pollutionColor('happiness', clean, 0)[3], 0, 'empty cell');
  const [hr, , hb, ha] = pollutionColor('happiness', clean, 255);
  const [sr, , sb] = pollutionColor('happiness', clean, 1);
  assert.ok(ha > 0 && hb > hr, 'happy is blue');
  assert.ok(sr > sb, 'miserable is red');
  assert.equal(pollutionColor('air', [0, 0, 0, 255], 255)[3], 0, 'mood shows only in its view');
});

test('the plain view keeps old factory ground visible under clean air', () => {
  const stained = pollutionColor('plain', [0, 255, 0, 50]);
  const hazy = pollutionColor('plain', [255, 0, 0, 50]);
  assert.ok(stained[3] > hazy[3], 'stains read more strongly than haze');
  assert.ok(hazy[3] > 0);
});

test('the overlay draws one texel per cell and redraws on a view change', () => {
  const overlay = new PollutionOverlay();
  overlay.setCells(new Uint8Array([0, 0, 0, 255, 255, 0, 0, 0]), [0, 0, 32, 2, 1]);
  const texture = () => overlay.group.children[0].material.map;
  assert.equal(texture().image.width, 2);
  assert.equal(texture().image.data[3], 0, 'clean air shows nothing in the plain view');
  overlay.setView('land');
  assert.ok(texture().image.data[3] > 0);
});

test('plain haze darkens with the ground at night; the pollution views stay readable', () => {
  const overlay = new PollutionOverlay();
  overlay.setCells(new Uint8Array([255, 255, 0, 0]), [0, 0, 32, 1, 1]);
  const material = () => overlay.group.children[0].material;
  assert.equal(material().type, 'MeshLambertMaterial', 'lit like the ground');
  assert.equal(material().fog, true);
  const normal = overlay.group.children[0].geometry.getAttribute('normal');
  assert.equal(Math.abs(normal.getY(0)), 1, 'flat, so the light falls on it as on the ground');
  const plainMap = material().map;
  overlay.setView('air');
  assert.equal(material().type, 'MeshBasicMaterial', 'map views ignore the time of day');
  assert.equal(material().fog, false);
  assert.notEqual(material().map, plainMap, 'redrawn for the view');
  overlay.setView('plain');
  assert.equal(material().type, 'MeshLambertMaterial');
});

test('P cycles the pollution views, land value, happiness, traffic and services, then back to plain', async () => {
  const t = await tools();
  const views = [];
  for (let i = 0; i < 8; i++) {
    t.key('p');
    views.push(t.previews.findLast((p) => p.type === 'pollutionView').view);
  }
  assert.deepEqual(views, [
    'air',
    'ground',
    'noise',
    'land',
    'happiness',
    'traffic',
    'services',
    'plain',
  ]);
  t.reply({
    type: 'pollution',
    cells: new Uint8Array(4),
    mood: new Uint8Array(1),
    layout: [0, 0, 32, 1, 1],
  });
  assert.equal(t.previews.findLast((p) => p.type === 'pollution').layout[2], 32);
});

test('the worker sends the pollution grid at start and again as factories pollute', async () => {
  const w = await worker(0);
  w.ready();
  await w.init;
  const grids = () => w.messages.filter((m) => m.type === 'pollution');
  assert.equal(grids().length, 1, 'sent at start');
  const [first] = grids();
  assert.equal(first.cells.length, first.layout[3] * first.layout[4] * 4);
  assert.ok(
    first.cells.every((v, i) => (i % 4 === 3 ? v === 255 : v === 0)),
    'a new city is clean',
  );
  await w.send({ type: 'fillBlock', x: 50, y: 50, zone: 5 });
  for (let i = 0; i < 1500; i++) w.advance(101);
  const latest = grids().at(-1).cells;
  assert.ok(grids().length > 1);
  assert.ok(
    latest.some((v, i) => i % 4 === 0 && v > 0),
    'factories dirty the air',
  );
  assert.ok(
    latest.some((v, i) => i % 4 === 3 && v < 255),
    'and lower land value',
  );
  assert.equal(first.mood.length, 0, 'no mood before anyone lives here');
});
