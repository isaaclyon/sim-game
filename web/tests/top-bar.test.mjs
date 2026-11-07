import assert from 'node:assert/strict';
import test from 'node:test';

import { tools } from './main-harness.mjs';

const STATS = {
  tick: 0,
  day: 2,
  secondOfDay: 8 * 3600 + 5 * 60,
  population: 1234,
  unemployed: 56,
  unemploymentPercent: 4.538087,
  demand: { residential: 60, commercial: 0, industrial: -60 },
  power: { supply: 0, starter: 150, used: 0, demand: 120, unpowered: 0, waiting: 0 },
  agents: 9,
  parked: 3,
  awaitingPath: 0,
  entering: 0,
  driving: 2,
  walking: 4,
  walksCompleted: 1,
  tripsCompleted: 4,
  despawnedStuck: 7,
  pathsFailed: 8,
  despawnedByEdit: 9,
  balance: 100_000,
  taxes: 0,
  grant: 0,
  upkeep: 0,
};

function show(t, changes = {}) {
  t.reply({
    type: 'frame',
    agents: new Float32Array(),
    signals: new Float32Array(),
    tickMs: 1.5,
    stats: { ...STATS, ...changes },
  });
  t.frame();
}

/** IDs of the page's top-level parts that are showing. */
const showing = (t) =>
  t.document.body.children.filter((el) => !el.hidden).map((el) => el.id || el.localName);

test('while playing, only the top bar and the category bar show', async () => {
  const t = await tools();
  show(t);
  assert.deepEqual(showing(t), ['top-bar', 'build-menu']);
  const menu = t.elements['build-menu'].children.filter((el) => !el.hidden);
  assert.deepEqual(
    menu.map((el) => el.localName),
    ['nav'],
    'no item or options row without a tool',
  );
  const text = t.elements['top-bar'].textContent;
  assert.doesNotMatch(text, /fps|tick|stuck|failed/, 'no developer numbers');
  assert.equal(t.elements.clock.textContent, 'Day 2 · 08:05');
  assert.equal(t.elements['population-value'].textContent, 'Pop 1,234');
});

test('the backtick key shows the developer numbers', async () => {
  const t = await tools();
  show(t);
  const debug = t.elements.debug;
  assert.equal(debug.hidden, true);
  t.key('`');
  show(t);
  assert.equal(debug.hidden, false);
  for (const line of [/fps/, /tick +1\.50 ms/, /stuck cars +7/, /failed routes +8/, /9 cars/])
    assert.match(debug.textContent, line);
  t.key('`');
  assert.equal(debug.hidden, true);
});

test('population opens its details; a second click or Esc closes them', async () => {
  const t = await tools();
  const { elements: el } = t;
  show(t);
  t.pressId('population');
  assert.equal(el['population-panel'].hidden, false);
  assert.equal(el.population.attributes['aria-expanded'], 'true');
  assert.equal(el['pop-unemployed'].textContent, '56 (4.5%)');
  assert.equal(el['pop-driving'].textContent, '2');
  assert.equal(el['pop-walking'].textContent, '4');
  assert.equal(el['pop-parked'].textContent, '3');
  assert.equal(el['pop-trips'].textContent, '4 · 25% on foot');
  t.pressId('population');
  assert.equal(el['population-panel'].hidden, true);
  t.pressId('population');
  t.pressId('money');
  assert.equal(el['population-panel'].hidden, true, 'one panel at a time');
  assert.equal(el['money-panel'].hidden, false);
  t.key('Escape');
  assert.equal(el['money-panel'].hidden, true);
  assert.equal(el.money.attributes['aria-expanded'], 'false');
});

test('a panel opened from the keyboard takes focus, and gives it back when it closes', async () => {
  const t = await tools();
  show(t);
  const { elements: el, document } = t;
  el.population.focus();
  const focused = () => document.activeElement.id;
  el.population.click({ detail: 0 });
  assert.equal(focused(), 'population-panel', 'a panel of figures is read whole');
  t.key('Escape');
  assert.equal(focused(), 'population');
  el['menu-button'].click({ detail: 0 });
  assert.equal(focused(), 'sound', 'the first control that can take focus');
  t.reply({ type: 'cityStatus', ready: true, busy: false, dirty: false, savedAt: 0, error: null });
  t.key('Escape');
  el['menu-button'].click({ detail: 0 });
  assert.equal(focused(), 'save-city');
});

test('power turns red when the town needs more than it has, and its panel says why', async () => {
  const t = await tools();
  const { elements: el } = t;
  show(t);
  assert.equal(el['power-value'].textContent, '80%');
  assert.equal(el.power.classList.contains('warning'), false);
  t.pressId('power');
  assert.equal(el['power-title'].textContent, 'Power');
  show(t, {
    power: { supply: 0, starter: 150, used: 150, demand: 300, unpowered: 4, waiting: 1 },
  });
  assert.equal(el['power-value'].textContent, '200%');
  assert.equal(el.power.classList.contains('warning'), true);
  assert.match(el.power.attributes['aria-label'], /not enough power/);
  assert.equal(el['power-title'].textContent, 'Not enough power');
  assert.equal(el['power-needed'].textContent, '300');
  assert.equal(el['power-available'].textContent, '150 (starter 150)');
  assert.equal(el['power-unpowered'].textContent, '4 buildings without power');
  assert.equal(el['power-waiting'].textContent, '1 lot waiting for power');
  assert.match(el['power-advice'].textContent, /Build a plant/);
  t.pressId('open-power');
  assert.equal(el['power-panel'].hidden, true);
  assert.equal(el['items-power'].hidden, false, 'Open Power opens the Power category');
  assert.equal(el['category-power'].attributes['aria-expanded'], 'true');
});

test('power turns red while lots wait for power, even with supply to spare', async () => {
  const t = await tools();
  const { elements: el } = t;
  show(t, {
    power: { supply: 100, starter: 150, used: 248, demand: 248, unpowered: 0, waiting: 3 },
  });
  assert.equal(el['power-value'].textContent, '99%');
  assert.equal(el.power.classList.contains('warning'), true);
  assert.match(el.power.attributes['aria-label'], /3 lots waiting for power/);
  t.pressId('power');
  assert.equal(el['power-title'].textContent, 'More power needed');
  assert.equal(el['power-title'].classList.contains('problem'), true);
  assert.match(el['power-advice'].textContent, /need more power to grow/);
  show(t, {
    power: { supply: 100, starter: 150, used: 248, demand: 248, unpowered: 0, waiting: 0 },
  });
  assert.equal(el.power.classList.contains('warning'), false);
  assert.equal(el['power-title'].textContent, 'Power');
});

test('Open Power from the keyboard moves focus to the first Power item', async () => {
  const t = await tools();
  const { elements: el, document } = t;
  show(t);
  el.power.focus();
  el.power.click({ detail: 0 });
  el['open-power'].focus();
  el['open-power'].click({ detail: 0 });
  assert.equal(el['items-power'].hidden, false);
  assert.equal(document.activeElement.id, 'plant-coal');
});

test('the save status opens and closes the menu, and Esc closes it from the link field', async () => {
  const t = await tools();
  const { elements: el } = t;
  t.pressId('save-badge');
  assert.equal(el.menu.hidden, false);
  assert.equal(el['save-badge'].attributes['aria-expanded'], 'true');
  t.pressId('save-badge');
  assert.equal(el.menu.hidden, true, 'a second click closes it');
  assert.equal(el['save-badge'].attributes['aria-expanded'], 'false');
  t.pressId('menu-button');
  t.key('Escape', { target: el['share-url'] });
  assert.equal(el.menu.hidden, true);
});

test('R/C/I demand bars grow for demand and shrink back for a surplus', async () => {
  const t = await tools();
  show(t);
  const style = (use) => ({ ...t.elements[`demand-${use}`].style });
  assert.deepEqual(style('residential'), { width: '25.0%', left: '50%' });
  assert.deepEqual(style('commercial'), { width: '0.0%', left: '50%' });
  assert.deepEqual(style('industrial'), { width: '25.0%', left: '25.0%' });
  assert.equal(t.elements['demand-industrial'].classList.contains('surplus'), true);
  assert.equal(
    t.elements.demand.attributes['aria-label'],
    'demand: residential +60, commercial 0, industrial -60',
  );
});

test('speed buttons set the speed and show the one in use', async () => {
  const t = await tools();
  t.pressId('speed-10');
  assert.deepEqual({ ...t.messages.at(-1) }, { type: 'setSpeed', speed: 10 });
  assert.equal(t.elements['speed-10'].attributes['aria-pressed'], 'true');
  assert.equal(t.elements['speed-1'].attributes['aria-pressed'], 'false');
  t.key(' ');
  assert.equal(t.elements['speed-0'].attributes['aria-pressed'], 'true');
});
