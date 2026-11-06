import assert from 'node:assert/strict';
import test from 'node:test';

import { RoadClass } from '../src/protocol.ts';
import { tools } from './main-harness.mjs';

const pressed = (t, id) => t.elements[id].attributes['aria-pressed'] === 'true';
const hint = (t) => t.elements['build-hint'].textContent;
const lastUpgrade = (t) => t.previews.findLast((p) => p.type === 'upgrade');

test('Upgrade under Roads highlights the road under the cursor and a click sets its class (ADR 0050)', async () => {
  const t = await tools();
  t.pressId('category-roads');
  t.pressId('main-road');
  const option = t.elements['upgrade-road'];
  assert.equal(option.localName, 'button', 'a native, keyboard-reachable control');
  assert.equal(option.textContent, 'Upgrade', 'named by its label');
  t.pressId('upgrade-road');
  assert.ok(pressed(t, 'upgrade-road'));
  assert.ok(pressed(t, 'main-road'), 'the item picks the class roads take');
  assert.equal(pressed(t, 'straight'), false);
  assert.match(hint(t), /Click a road to make it a main road/);

  t.move(50, 3);
  const ask = t.messages.at(-1);
  assert.deepEqual(
    { ...ask },
    { type: 'previewRoadClass', x: 50, y: 3, class: RoadClass.Main, revision: ask.revision },
  );
  const segment = new Float32Array([0, 0, 100, 0]);
  t.reply({
    type: 'roadClassPreview',
    revision: ask.revision,
    segment,
    cost: 1500,
    affordable: true,
  });
  assert.deepEqual(lastUpgrade(t), {
    type: 'upgrade',
    segment,
    ok: true,
    roadClass: RoadClass.Main,
  });
  assert.match(hint(t), /Costs \$1,500/);

  t.click(50, 3);
  assert.deepEqual(
    { ...t.messages.at(-1) },
    { type: 'setRoadClass', x: 50, y: 3, class: RoadClass.Main },
  );
  assert.ok(pressed(t, 'upgrade-road'), 'the tool stays on for the next road');

  t.move(60, 3);
  const dear = t.messages.at(-1);
  t.reply({
    type: 'roadClassPreview',
    revision: dear.revision,
    segment,
    cost: 90_000,
    affordable: false,
  });
  assert.equal(lastUpgrade(t).ok, false, 'drawn in the refusal red');
  assert.match(hint(t), /Not enough money: costs \$90,000/);
});

test('Upgrade drops stale answers, follows the class keys and leaves the other keys alone', async () => {
  const t = await tools();
  t.pressId('category-roads');
  t.pressId('street');
  t.pressId('upgrade-road');
  t.move(10, 0);
  const old = t.messages.at(-1);
  t.key('m');
  assert.ok(pressed(t, 'main-road'));
  assert.ok(pressed(t, 'upgrade-road'), 'M switches the class, not the tool');
  assert.match(hint(t), /make it a main road/);
  const segment = new Float32Array([0, 0, 100, 0]);
  t.reply({ type: 'roadClassPreview', revision: old.revision, segment, cost: 0, affordable: true });
  assert.equal(lastUpgrade(t), undefined, 'an answer for the old class is dropped');
  t.move(10, 0);
  assert.deepEqual(
    [t.messages.at(-1).type, t.messages.at(-1).class],
    ['previewRoadClass', RoadClass.Main],
  );

  t.key('r');
  assert.ok(pressed(t, 'straight'), 'R still starts a straight road');
  assert.equal(pressed(t, 'upgrade-road'), false);
  // One question is in flight at a time; its late answer is not drawn by the road tool.
  const late = t.messages.at(-1);
  const drawn = t.previews.length;
  t.reply({
    type: 'roadClassPreview',
    revision: late.revision,
    segment,
    cost: 0,
    affordable: true,
  });
  assert.equal(t.previews.length, drawn);
  t.move(20, 0);
  assert.equal(t.messages.at(-1).type, 'previewRoad');
  t.pressId('upgrade-road');
  t.key('Escape');
  assert.equal(pressed(t, 'upgrade-road'), false);
  assert.equal(hint(t), '');
});
