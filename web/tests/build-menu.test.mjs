import assert from 'node:assert/strict';
import test from 'node:test';

import { PlantKind } from '../src/protocol.ts';
import { tools } from './main-harness.mjs';

const pressed = (t, id) => t.elements[id].attributes['aria-pressed'] === 'true';
const last = (t, type) => t.messages.findLast((m) => m.type === type);

test('one entry in the list adds a keyboard-reachable item that starts its tool', async () => {
  const t = await tools({
    catalog({ CATEGORIES }) {
      CATEGORIES.find((c) => c.id === 'services').items.push({
        id: 'test-turbine',
        label: 'Test turbine',
        tip: 'An item added by one list entry.',
        picture: { kind: 'plant', plant: PlantKind.Wind },
        tool: { kind: 'plant', plant: PlantKind.Wind },
      });
    },
  });
  const { elements: el, document } = t;
  const category = el['category-services'];
  assert.equal(category.localName, 'button', 'a hidden category shows once it has an item');
  assert.equal(category.attributes['aria-label'], 'Services');
  // Enter on a focused category opens its items and moves focus to the first one.
  category.focus();
  category.click({ detail: 0 });
  assert.equal(el['items-services'].hidden, false);
  assert.equal(document.activeElement, el['test-turbine']);
  const item = el['test-turbine'];
  assert.equal(item.localName, 'button');
  assert.equal(item.attributes['aria-label'], 'Test turbine');
  assert.match(item.title, /^Test turbine\nAn item added by one list entry\.$/);
  assert.equal(item.querySelector('img').attributes.alt, 'Test turbine');
  assert.ok(
    t.pictures.asked[0].some((p) => p.kind === 'plant' && p.plant === PlantKind.Wind),
    'its picture is drawn from the model the entry names',
  );
  item.click({ detail: 0 });
  assert.ok(pressed(t, 'test-turbine'));
  assert.equal(el['items-services'].hidden, false, 'its own row stays open');
  t.move(10, 20);
  const ask = last(t, 'previewPlant');
  assert.deepEqual([ask.kind, ask.x, ask.y], [PlantKind.Wind, 10, 20]);
  t.click(30, 40);
  assert.deepEqual({ ...last(t, 'placePlant') }, { type: 'placePlant', kind: 2, x: 30, y: 40 });
});

test('a category opens its items without starting a tool; an item starts one', async () => {
  const t = await tools();
  const { elements: el } = t;
  t.pressId('category-roads');
  assert.equal(el['items-roads'].hidden, false);
  assert.equal(el['category-roads'].attributes['aria-expanded'], 'true', 'its row is expanded');
  assert.equal(el['build-options'].hidden, true, 'no options row before a tool is in use');
  t.move(10, 20);
  assert.equal(last(t, 'previewRoad'), undefined, 'opening the category starts nothing');
  t.pressId('street');
  assert.ok(pressed(t, 'street') && pressed(t, 'straight'));
  assert.equal(el['build-options'].hidden, false);
  assert.equal(el['options-roads'].hidden, false);
  assert.match(el['build-hint'].textContent, /Click the road start/);
  t.pressId('curve');
  assert.ok(pressed(t, 'curve') && !pressed(t, 'straight'), 'an option changes the tool in use');
  t.pressId('main-road');
  assert.ok(pressed(t, 'main-road') && pressed(t, 'curve'), 'the shape carries over');
  t.pressId('category-zones');
  assert.ok(!pressed(t, 'main-road'), 'another category puts the road tool away');
  assert.equal(el['items-roads'].hidden, true);
  assert.equal(el['build-options'].hidden, true);
  t.pressId('apartments');
  assert.ok(pressed(t, 'apartments'));
  assert.match(el['build-hint'].textContent, /Click a block/);
  t.pressId('apartments');
  assert.ok(!pressed(t, 'apartments'), 'clicking the item in use puts it away');
  assert.equal(el['items-zones'].hidden, true, 'and closes its row');
  assert.equal(el['build-hint'].textContent, '');
  t.key('x');
  assert.equal(el['items-zones'].hidden, false, 'a key opens its category too');
  t.key('Escape');
  assert.ok(!pressed(t, 'small-shops'));
  assert.equal(el['items-zones'].hidden, true);
  t.pressId('category-power');
  t.key('Escape');
  assert.equal(el['items-power'].hidden, true, 'Esc closes an open row with no tool');
});

test('Bulldoze sits apart at the right of the category bar and switches like B', async () => {
  const t = await tools();
  const bar = t.document.querySelector('nav');
  const ids = bar.children.map((b) => b.id || b.attributes.class);
  assert.deepEqual(ids, [
    'category-roads',
    'category-zones',
    'category-power',
    'divider',
    'bulldoze',
    'views',
  ]);
  t.pressId('bulldoze');
  assert.ok(pressed(t, 'bulldoze'));
  assert.match(t.elements['build-hint'].textContent, /highlighted road, building or power/);
  t.key('b');
  assert.ok(!pressed(t, 'bulldoze'));
  t.key('B');
  t.pressId('bulldoze');
  assert.ok(!pressed(t, 'bulldoze'), 'a second press puts it away');
});

test('Views shows one ground overlay at a time, with traffic and power as switches', async () => {
  const t = await tools();
  const { elements: el } = t;
  const latest = (type) => t.previews.findLast((p) => p.type === type);
  t.pressId('views');
  assert.equal(el['views-panel'].hidden, false);
  assert.equal(el.views.attributes['aria-expanded'], 'true');
  t.pressId('view-noise');
  assert.equal(latest('pollutionView').view, 'noise');
  assert.ok(pressed(t, 'view-noise') && !pressed(t, 'view-plain'));
  t.pressId('view-land');
  assert.ok(pressed(t, 'view-land') && !pressed(t, 'view-noise'), 'one overlay at a time');
  t.pressId('traffic');
  t.pressId('power-view');
  assert.ok(pressed(t, 'traffic') && pressed(t, 'power-view') && pressed(t, 'view-land'));
  assert.equal(latest('powerView').on, true);
  t.key('p');
  assert.ok(pressed(t, 'view-plain'), 'P carries on from the overlay picked in the list');
  assert.ok(el.views.classList.contains('on'), 'traffic and power still show');
  t.key('Escape');
  assert.equal(el['views-panel'].hidden, true);
  t.key('t');
  t.key('o');
  assert.ok(!el.views.classList.contains('on'));
});

test('every control has an accessible name, is a native control, and shows its picture', async () => {
  const t = await tools();
  const all = [...t.document.body.descendants()];
  const controls = all.filter((el) => el.matches('button, input'));
  assert.ok(controls.length > 40);
  for (const el of controls) {
    const name = el.attributes['aria-label'] ?? el.textContent.trim();
    assert.ok(name, `${el.id} has an accessible name`);
    assert.notEqual(el.attributes.tabindex, '-1', `${el.id} can be reached with Tab`);
  }
  const clickable = all.filter((el) => el.onclick || el.events.click);
  assert.deepEqual(
    clickable.filter((el) => !el.matches('button, input')),
    [],
    'only native controls take clicks, so Enter and Space work on all of them',
  );
  const menu = t.elements['build-menu'];
  const buttons = menu.querySelectorAll('button[aria-label]');
  assert.ok(buttons.length >= 17, 'every category, item, Bulldoze and Views');
  for (const b of buttons) {
    const img = b.querySelector('img');
    assert.equal(img.attributes.alt, b.attributes['aria-label']);
    assert.equal(img.hidden, false, `${b.id} shows its picture`);
    assert.match(img.src, /^data:image\/png/);
  }
});

test('putting a tool away keeps focus on screen and closes the panel its options opened', async () => {
  const t = await tools();
  const { elements: el, document } = t;
  const focused = () => document.activeElement.id;
  el['category-zones'].focus();
  el['category-zones'].click({ detail: 0 });
  assert.equal(focused(), 'houses');
  el.houses.click({ detail: 0 });
  t.key('Escape', { target: el.houses });
  assert.equal(el['items-zones'].hidden, true);
  assert.equal(focused(), 'category-zones', 'focus moves to the category of the closed row');
  assert.equal(el['category-zones'].attributes['aria-expanded'], 'false');
  t.key('r');
  t.pressId('snaps');
  assert.equal(el['snaps-panel'].hidden, false);
  el['snap-angle'].focus();
  t.key('b', { target: el['snap-angle'] });
  assert.equal(el['options-roads'].hidden, true);
  assert.equal(el['snaps-panel'].hidden, true, 'no Snaps panel without the road tool');
  assert.equal(el.snaps.attributes['aria-expanded'], 'false');
  assert.equal(focused(), 'category-roads');
});

test('the hint line is not read aloud on every cursor move', async () => {
  const t = await tools();
  const hint = t.elements['build-hint'];
  assert.equal(hint.getAttribute('aria-live'), null);
  assert.equal(hint.getAttribute('role'), null);
});
