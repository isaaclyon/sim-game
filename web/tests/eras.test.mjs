import assert from 'node:assert/strict';
import test from 'node:test';

import { Era } from '../src/protocol.ts';
import { tools } from './main-harness.mjs';

const pressed = (t, id) => t.elements[id].attributes['aria-pressed'] === 'true';
const hint = (t) => t.elements['build-hint'].textContent;
const price = (t, id) => t.elements[id].children.find((c) => c.classList.contains('price'));

/** A frame from the sim with the city's era and population (ADR 0059). */
function frame(t, era, population) {
  t.reply({
    type: 'frame',
    agents: new Float32Array(),
    signals: new Float32Array(),
    tickMs: 1,
    stats: { secondOfDay: 0, population, era, townPopulation: 1500 },
  });
}

test('a village marks main roads and dense zones locked, with the residents still needed', async () => {
  const t = await tools();
  frame(t, Era.Village, 300);
  for (const id of ['main-road', 'apartments', 'stores', 'heavy-industry']) {
    assert.equal(t.elements[id].getAttribute('aria-disabled'), 'true', id);
    assert.match(t.elements[id].title, /Unlocks in the town era, 1,200 more residents needed\./);
    assert.equal(price(t, id).textContent, 'Town');
  }
  for (const id of ['street', 'houses', 'small-shops', 'light-industry', 'unzone'])
    assert.equal(t.elements[id].getAttribute('aria-disabled'), null, id);

  frame(t, Era.Town, 1500);
  for (const id of ['main-road', 'apartments', 'stores', 'heavy-industry']) {
    assert.equal(t.elements[id].getAttribute('aria-disabled'), null, id);
    assert.doesNotMatch(t.elements[id].title, /Unlocks/);
    assert.equal(price(t, id).textContent, '');
  }
});

test('locked tools are refused with a hint, by key or by the menu', async () => {
  const t = await tools();
  frame(t, Era.Village, 1499);
  t.key('m');
  assert.ok(!pressed(t, 'main-road') && !pressed(t, 'street'), 'no road tool starts');
  assert.equal(hint(t), 'Main road. Unlocks in the town era, 1 more resident needed.');
  assert.equal(t.elements['build-options'].hidden, false, 'the hint shows without a tool');
  t.key('Escape');
  assert.equal(t.elements['build-options'].hidden, true, 'Esc dismisses it');

  // The second Z would switch to apartments; the houses brush stays in use.
  t.key('z');
  assert.ok(pressed(t, 'houses'));
  t.key('z');
  assert.ok(pressed(t, 'houses') && !pressed(t, 'apartments'));
  assert.match(hint(t), /^Apartments\. Unlocks in the town era/);

  t.pressId('category-roads');
  t.pressId('street');
  t.pressId('main-road');
  assert.ok(pressed(t, 'street'), 'a street being drawn stays a street');
  t.move(10, 20);
  assert.equal(t.messages.findLast((m) => m.type === 'previewRoad').class, 0);
});

test('the town unlocks them, and roads repaint for the era', async () => {
  const t = await tools();
  frame(t, Era.Village, 0);
  assert.equal(t.rendererEra(), Era.Village, 'village roads are dirt');
  frame(t, Era.Town, 1500);
  assert.equal(t.rendererEra(), Era.Town);
  t.key('m');
  assert.ok(pressed(t, 'main-road'));
  t.key('z');
  t.key('z');
  assert.ok(pressed(t, 'apartments'));
});

test('the town clears a refusal hint, and loading a village puts locked tools away', async () => {
  const t = await tools();
  frame(t, Era.Village, 1499);
  t.key('m');
  assert.match(hint(t), /Unlocks in the town era/);
  frame(t, Era.Town, 1500);
  assert.equal(hint(t), '', 'the hint goes once the town arrives');
  assert.equal(t.elements['build-options'].hidden, true);

  t.key('m');
  assert.ok(pressed(t, 'main-road'));
  frame(t, Era.Village, 10);
  assert.ok(!pressed(t, 'main-road') && !pressed(t, 'street'), 'the main road tool is put away');
  t.key('r');
  assert.ok(pressed(t, 'street'), 'the road tool starts again as a street');

  frame(t, Era.Town, 1500);
  t.key('z');
  t.key('z');
  assert.ok(pressed(t, 'apartments'));
  frame(t, Era.Village, 10);
  assert.ok(!pressed(t, 'apartments'), 'the apartments brush is put away');
});
