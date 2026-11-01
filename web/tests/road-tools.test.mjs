import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { stripTypeScriptTypes } from 'node:module';
import test from 'node:test';
import { createContext, SourceTextModule, SyntheticModule } from 'node:vm';

import { RoadClass, Zone } from '../src/protocol.ts';

// Exercise main.ts's event handlers without a WebGL context or a running clock.
async function tools() {
  function element() {
    return {
      textContent: '',
      style: {},
      events: {},
      attributes: {},
      classList: { toggle() {} },
      setPointerCapture() {},
      setAttribute(key, value) {
        this.attributes[key] = value;
      },
      addEventListener(key, fn) {
        this.events[key] = fn;
      },
    };
  }
  const elements = Object.fromEntries(
    ['hud', 'spawn', 'road', 'curve', 'main-road', 'bulldoze', 'road-help'].map((id) => [
      id,
      element(),
    ]),
  );
  const zoneButtons = Object.values(Zone).map((zone) => ({
    ...element(),
    dataset: { zone: String(zone) },
  }));
  const canvas = element();
  const pans = [];
  const zoneDrags = [];
  const messages = [];
  const previews = [];
  const events = {};
  let worker;
  class Worker {
    constructor() {
      worker = this;
    }
    postMessage(msg) {
      messages.push(msg);
    }
  }
  class Renderer {
    renderer = { domElement: canvas };
    pick(x, z) {
      return { x, z };
    }
    setPreview(stops) {
      previews.push({ type: 'road', stops });
    }
    setLeftDragPans(on) {
      pans.push(on);
    }
    setZoneDrag(a, b, zone) {
      zoneDrags.push({ a, b, zone });
    }
    setZones(cells, layout) {
      previews.push({ type: 'zones', cells, layout });
    }
    setBulldozePreview(segment) {
      previews.push({ type: 'bulldoze', segment });
    }
  }
  const context = createContext({
    document: {
      body: {},
      getElementById: (id) => elements[id],
      querySelectorAll: (selector) => (selector === '[data-zone]' ? zoneButtons : []),
    },
    URL,
    Worker,
    performance: { now: () => 0 },
    requestAnimationFrame: () => {},
    addEventListener: (key, fn) => {
      events[key] = fn;
    },
  });
  const module = new SourceTextModule(
    stripTypeScriptTypes(readFileSync(new URL('../src/main.ts', import.meta.url), 'utf8')),
    {
      context,
      initializeImportMeta(meta) {
        meta.url = import.meta.url;
        meta.env = { DEV: false };
      },
    },
  );
  await module.link((specifier) =>
    specifier === './protocol'
      ? new SyntheticModule(
          ['RoadClass', 'Zone'],
          function () {
            this.setExport('RoadClass', RoadClass);
            this.setExport('Zone', Zone);
          },
          { context },
        )
      : new SyntheticModule(
          ['Renderer'],
          function () {
            this.setExport('Renderer', Renderer);
          },
          { context },
        ),
  );
  await module.evaluate();
  const move = (x, y) => canvas.events.pointermove({ clientX: x, clientY: y });
  const click = (x, y, button = 0) => {
    const e = { clientX: x, clientY: y, button };
    canvas.events.pointerdown(e);
    canvas.events.pointerup(e);
  };
  return {
    elements,
    messages,
    previews,
    zoneButtons,
    pans,
    zoneDrags,
    down: (x, y) => canvas.events.pointerdown({ clientX: x, clientY: y, button: 0 }),
    up: (x, y) => canvas.events.pointerup({ clientX: x, clientY: y, button: 0 }),
    move,
    click,
    reply: (data) => worker.onmessage({ data }),
    key: (key) => events.keydown({ key }),
  };
}

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
