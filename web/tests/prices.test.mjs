import assert from 'node:assert/strict';
import test from 'node:test';

import { PRICES, tools } from './main-harness.mjs';

const last = (t, type) => t.messages.findLast((m) => m.type === type);
const tag = (button) => button.children.find((c) => c.attributes.class === 'price');

test('items that place things show their price on the card and in the tooltip', async () => {
  const t = await tools();
  const coal = t.elements['plant-coal'];
  assert.equal(tag(coal).textContent, '', 'no price before the sim sends one');
  t.reply({ type: 'cityReset', speed: 0, prices: PRICES });
  assert.equal(tag(coal).textContent, '$20,000');
  assert.match(coal.title, /Costs \$20,000 to build and \$120 an hour to keep\./);
  assert.equal(tag(t.elements['water-tower']).textContent, '$4,000');
  assert.equal(tag(t.elements['fire-station']).textContent, '$6,000');
  assert.equal(tag(t.elements['power-line']).textContent, 'Free');
  assert.equal(tag(t.elements.street).textContent, '', 'roads are quoted while drawn');
});

test('placing hints name the price, and say so when the city cannot pay', async () => {
  const t = await tools();
  t.reply({ type: 'cityReset', speed: 0, prices: PRICES });
  const hint = () => t.elements['build-hint'].textContent;
  t.key('g');
  assert.match(hint(), /^Coal plant: \$20,000, then \$120 an hour\. Click to place\./);
  t.move(10, 20);
  const ask = last(t, 'previewPlant');
  const outline = new Float32Array([0, 0, 1, 0]);
  const answer = { type: 'plantPreview', outline, revision: ask.revision };
  t.reply({ ...answer, ok: false, affordable: false });
  assert.match(hint(), /^Not enough money: a coal plant costs \$20,000\. Click to place\./);
  t.move(12, 20);
  t.reply({ ...answer, revision: last(t, 'previewPlant').revision, ok: true, affordable: true });
  assert.match(hint(), /^Coal plant: \$20,000/);

  t.pressId('category-services');
  t.pressId('fire-station');
  assert.match(hint(), /^Fire station: \$6,000, then \$120 an hour\./);
  t.move(30, 40);
  const station = last(t, 'previewStation');
  t.reply({
    ...answer,
    type: 'stationPreview',
    revision: station.revision,
    ok: false,
    affordable: false,
  });
  assert.match(hint(), /^Not enough money: a fire station costs \$6,000\./);
});
