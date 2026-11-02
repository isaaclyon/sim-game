import assert from 'node:assert/strict';
import test from 'node:test';

import { RoadClass, Zone } from '../src/protocol.ts';
import { tools } from './main-harness.mjs';

test('normal startup has no synthetic traffic and HUD displays authoritative population and unemployment', async () => {
  const t = await tools();
  assert.equal(t.messages[0].type, 'init');
  assert.equal(t.messages[0].agents, 0);
  const stats = {
    day: 1,
    secondOfDay: 21600,
    population: 1234,
    unemployed: 56,
    unemploymentPercent: 4.538087,
    demand: { residential: 0, commercial: 0, industrial: 0 },
    agents: 9,
    parked: 0,
    driving: 2,
    tripsCompleted: 0,
    despawnedStuck: 0,
    pathsFailed: 0,
    despawnedByEdit: 0,
  };
  const show = (changes) => {
    t.reply({
      type: 'frame',
      agents: new Float32Array(),
      signals: new Float32Array(),
      tickMs: 0,
      stats: { ...stats, ...changes },
    });
    t.frame();
  };
  show({});
  assert.match(t.elements.hud.textContent, /population 1,234 {3}unemployed 56 \(4\.5%\)/);
  show({ population: 0, unemployed: 0, unemploymentPercent: 0 });
  assert.match(t.elements.hud.textContent, /population 0 {3}unemployed 0 \(0\.0%\)/);
  assert.match(
    t.elements.hud.textContent,
    /WASD or drag pans, Q\/E turns/,
    'camera help is on screen',
  );
});

test('toolbar shortcuts leave modified, repeated and native control keys alone', async () => {
  const t = await tools();
  for (const options of [
    { ctrlKey: true },
    { metaKey: true },
    { altKey: true },
    { repeat: true },
    { isComposing: true },
  ]) {
    t.key('r', options);
    assert.notEqual(t.elements.road.attributes['aria-pressed'], 'true');
  }
  t.key('R');
  assert.equal(t.elements.road.attributes['aria-pressed'], 'true');
  t.key('r', { repeat: true });
  assert.equal(t.elements.road.attributes['aria-pressed'], 'true');
  const before = t.messages.length;
  const button = { closest: (selector) => (selector.includes('button') ? button : null) };
  t.key(' ', { target: button });
  assert.equal(t.messages.length, before, 'Space on a button must only activate that button');
  const input = { closest: (selector) => (selector.includes('input') ? input : null) };
  t.key('z', { target: input });
  assert.equal(t.elements.road.attributes['aria-pressed'], 'true', 'typing must not switch tools');
  t.key(' ');
  assert.equal(t.messages.at(-1).type, 'setSpeed');
  assert.equal(t.messages.at(-1).speed, 0);
  t.key('Escape');
  assert.equal(t.elements.road.attributes['aria-pressed'], 'false');
});

test('zoning shortcuts and drags stay exclusive with curved roads and bulldozing', async () => {
  const t = await tools();
  t.key('v');
  t.click(0, 0);
  t.click(100, 100);
  t.move(200, 0);
  const old = t.messages.at(-1);
  for (const [key, low] of [
    ['z', Zone.ResidentialLow],
    ['x', Zone.CommercialLow],
    ['c', Zone.IndustrialLow],
  ]) {
    t.key(key);
    assert.equal(t.elements.curve.attributes['aria-pressed'], 'false');
    assert.equal(t.pans.at(-1), false, 'painting disables left-drag panning');
    for (const expected of [low, low + 1]) {
      if (expected !== low) t.key(key);
      t.down(10, 20);
      t.move(50, 60);
      assert.equal(t.zoneDrags.at(-1).zone, expected);
      t.up(50, 60);
      const paint = t.messages.at(-1);
      assert.equal(paint.type, 'paintZone');
      assert.equal(paint.zone, expected);
      assert.deepEqual([paint.ax, paint.ay, paint.bx, paint.by], [10, 20, 50, 60]);
    }
  }
  t.reply({
    type: 'roadPreview',
    revision: old.revision,
    stops: new Float32Array([0, 0, 0, 200, 0, 0]),
  });
  assert.equal(t.previews.at(-1).stops, null, 'late curve previews cannot cover the zone brush');
  t.key('u');
  t.click(10, 20);
  assert.equal(t.messages.at(-1).type, 'fillBlock', 'a click fills the block under it');
  assert.deepEqual([t.messages.at(-1).x, t.messages.at(-1).y], [10, 20]);
  assert.equal(t.messages.at(-1).zone, Zone.None);
  t.down(10, 20);
  t.key('v');
  t.up(50, 60);
  assert.equal(t.pans.at(-1), true);
  assert.equal(t.zoneDrags.at(-1).a, null, 'switching tools clears the drag');
  t.click(0, 0);
  t.click(100, 100);
  t.click(200, 0);
  assert.equal(t.messages.at(-1).type, 'buildCurve', 'V still builds a curve after zoning');
  t.zoneButtons.find((b) => Number(b.dataset.zone) === Zone.ResidentialLow).onclick();
  t.key('b');
  assert.equal(t.pans.at(-1), true);
  assert.ok(t.zoneButtons.every((b) => b.attributes['aria-pressed'] === 'false'));
  t.click(100, 50);
  assert.equal(t.messages.at(-1).type, 'deleteRoad');
  const cells = new Uint8Array([1]);
  t.reply({ type: 'zones', cells, layout: [0, 0, 8, 1, 1] });
  assert.equal(
    t.previews.at(-1).cells,
    cells,
    'zone snapshots remain separate from traffic frames',
  );
});

test('the zone brush outlines the block under the cursor, one question at a time', async () => {
  const t = await tools();
  t.move(5, 5);
  assert.equal(t.messages.length, 1, 'only init: no brush, no block question');
  t.key('z');
  t.move(10, 10);
  const asked = t.messages.at(-1);
  assert.equal(asked.type, 'previewBlock');
  assert.deepEqual([asked.x, asked.y], [10, 10]);
  t.move(20, 20);
  t.move(30, 30);
  assert.equal(t.messages.at(-1), asked, 'waits for the reply in flight');
  const outline = new Float32Array([0, 0, 100, 0, 100, 100, 0, 100]);
  t.reply({ type: 'blockPreview', outline, revision: asked.revision });
  assert.deepEqual(t.previews.at(-1), { type: 'block', outline, zone: Zone.ResidentialLow });
  const latest = t.messages.at(-1);
  assert.deepEqual([latest.type, latest.x, latest.y], ['previewBlock', 30, 30], 'latest only');
  const shown = t.previews.length;
  t.down(30, 30);
  t.reply({ type: 'blockPreview', outline, revision: latest.revision });
  assert.equal(t.previews.length, shown, 'a reply from before the click is not drawn');
  t.up(30, 30);
  assert.equal(t.messages.at(-1).type, 'fillBlock');
  t.move(40, 40);
  const again = t.messages.at(-1);
  assert.equal(again.type, 'previewBlock', 'hovering resumes after the click');
  t.key('Escape');
  t.reply({ type: 'blockPreview', outline, revision: again.revision });
  assert.notEqual(t.previews.at(-1).type, 'block', 'nor once the brush is off');
  const rings = new Float32Array([3, 0, 0, 8, 0, 8, 8]);
  t.reply({ type: 'lots', rings });
  assert.deepEqual(t.previews.at(-1), { type: 'lots', rings });
});

test('switching from curve to bulldoze discards a delayed preview and resets construction', async () => {
  const t = await tools();
  t.key('v');
  t.click(0, 0);
  t.click(100, 100);
  t.move(200, 0);
  const old = t.messages.at(-1);
  assert.equal(old.type, 'previewCurve');
  t.key('b');
  assert.equal(t.elements.curve.attributes['aria-pressed'], 'false');
  assert.equal(t.elements.bulldoze.attributes['aria-pressed'], 'true');
  t.move(100, 50);
  t.reply({
    type: 'roadPreview',
    revision: old.revision,
    stops: new Float32Array([0, 0, 0, 200, 0, 0]),
  });
  assert.equal(t.previews.at(-1).stops, null, 'old curve cannot appear in bulldoze mode');
  const target = t.messages.at(-1);
  assert.equal(target.type, 'previewBulldoze');
  const segment = new Float32Array([0, 0, 100, 50, 200, 0]);
  t.reply({ type: 'bulldozePreview', revision: target.revision, segment });
  assert.equal(t.previews.at(-1).segment, segment);
  t.click(100, 50);
  assert.equal(t.messages.at(-1).type, 'deleteRoad');
  t.key('r');
  t.move(300, 0);
  assert.equal(t.messages.at(-1).type, 'previewSnap', 'switching tools starts a new road');
  t.key('Escape');
  assert.equal(t.elements.road.attributes['aria-pressed'], 'false');
  assert.equal(t.previews.at(-1).stops, null);
});

test('refused curves retain their control, successful curves chain, and late build replies cannot revive a cancelled tool', async () => {
  const t = await tools();
  t.elements.curve.onclick();
  t.click(0, 0);
  t.click(100, 100);
  t.click(200, 0);
  const first = t.messages.at(-1);
  assert.equal(first.type, 'buildCurve');
  t.reply({ type: 'buildResult', id: first.id, success: false });
  assert.match(t.elements['road-help'].textContent, /Cannot build here/);
  t.click(250, 0);
  const retry = t.messages.at(-1);
  assert.equal(retry.ax, 0);
  assert.equal(retry.cx, 100);
  assert.equal(retry.bx, 250);
  t.reply({ type: 'buildResult', id: retry.id, success: true });
  t.click(350, 100);
  t.click(450, 0);
  const chained = t.messages.at(-1);
  assert.equal(chained.ax, 250);
  assert.equal(chained.cx, 350);
  t.key('b');
  t.reply({ type: 'buildResult', id: chained.id, success: true });
  assert.match(t.elements['road-help'].textContent, /remove it/);
  t.move(350, 50);
  assert.equal(t.messages.at(-1).type, 'previewBulldoze');
  t.click(350, 50, 2);
  assert.equal(t.elements.bulldoze.attributes['aria-pressed'], 'false');
});

test('M builds main roads with either road tool until switched off', async () => {
  const t = await tools();
  t.key('m');
  assert.equal(t.elements['main-road'].attributes['aria-pressed'], 'true');
  assert.equal(t.elements.road.attributes['aria-pressed'], 'true', 'picks the road tool');
  assert.match(t.elements['road-help'].textContent, /Main road/);
  t.click(0, 0);
  t.click(100, 0);
  const straight = t.messages.at(-1);
  assert.equal(straight.type, 'buildRoad');
  assert.equal(straight.class, RoadClass.Main);
  t.key('v');
  t.click(0, 0);
  t.click(50, 50);
  t.click(100, 0);
  assert.equal(t.messages.at(-1).class, RoadClass.Main, 'kept across road tools');
  t.elements['main-road'].onclick();
  assert.equal(t.elements['main-road'].attributes['aria-pressed'], 'false');
  t.key('r');
  t.click(0, 0);
  t.click(100, 0);
  assert.equal(t.messages.at(-1).class, RoadClass.Street);
});
