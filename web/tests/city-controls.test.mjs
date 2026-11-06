import assert from 'node:assert/strict';
import { test } from 'node:test';
import { tools } from './main-harness.mjs';

const status = (overrides = {}) => ({
  type: 'cityStatus',
  ready: true,
  busy: false,
  dirty: false,
  savedAt: 1000,
  error: null,
  ...overrides,
});

test('city buttons wait for startup; saving is acknowledged only by worker status', async () => {
  const t = await tools();
  const save = t.elements['save-city'];
  assert.equal(save.disabled, true);
  t.reply(status({ busy: true }));
  assert.equal(save.disabled, true);
  assert.match(t.elements['save-status'].textContent, /Saving/);
  t.reply(status({ dirty: true, savedAt: null }));
  assert.equal(t.elements.menu.hidden, true, 'the menu waits for the ☰ button');
  t.pressId('menu-button');
  assert.equal(t.elements.menu.hidden, false);
  assert.equal(t.elements['menu-button'].attributes['aria-expanded'], 'true');
  t.press(save);
  assert.deepEqual({ ...t.messages.at(-1) }, { type: 'saveCity' });
  assert.match(t.elements['save-status'].textContent, /Unsaved/);
  t.key('Escape');
  assert.equal(t.elements.menu.hidden, true, 'Esc closes the menu');
  const badge = t.elements['save-badge'];
  t.reply(status({ error: 'Browser storage is full.', dirty: true }));
  t.frame();
  assert.match(t.elements['save-status'].textContent, /full/);
  assert.match(badge.textContent, /full/);
  assert.equal(badge.dataset.error, 'true', 'a failed save shows red in the top bar');
  assert.equal(t.elements.menu.hidden, true, 'without opening the menu');
  assert.equal(save.disabled, false);
  t.reply(status());
  t.frame();
  assert.match(t.elements['save-status'].textContent, /Saved/);
  assert.equal(badge.dataset.error, 'false');
  t.press(badge);
  assert.equal(t.elements.menu.hidden, false, 'the save status opens the menu too');
});

test('new city requires confirmation; cancel sends nothing and storage errors allow starting over', async () => {
  const t = await tools();
  t.reply(status({ ready: false, error: 'Saved city is damaged.' }));
  const button = t.elements['new-city'];
  assert.equal(button.disabled, false);
  assert.equal(t.elements['save-city'].disabled, true);
  const count = t.messages.length;
  t.confirm.answer = false;
  button.onclick();
  assert.equal(t.messages.length, count);
  assert.match(t.confirm.prompts.at(-1), /replaces|replace/);
  t.confirm.answer = true;
  button.onclick();
  assert.deepEqual({ ...t.messages.at(-1) }, { type: 'newCity' });
});

test('city reset clears tools, follow and interpolation, and synchronizes pause controls', async () => {
  const t = await tools();
  t.cars.at = 3;
  t.click(5, 5);
  const selection = t.messages.at(-1).selection;
  t.reply({
    type: 'citizen',
    selection,
    citizen: {
      id: 1,
      agent: 3,
      name: 'Ada',
      resident: true,
      home: null,
      job: null,
      location: null,
      destination: null,
      activity: 0,
      driving: true,
      onFoot: false,
    },
  });
  t.key('f');
  assert.equal(t.cars.following.at(-1), true);
  const frame = {
    type: 'frame',
    agents: new Float32Array(5),
    signals: new Float32Array(),
    stats: {
      day: 1,
      secondOfDay: 43200,
      population: 0,
      unemployed: 0,
      unemploymentPercent: 0,
      demand: { residential: 0, commercial: 0, industrial: 0 },
      power: { supply: 0, starter: 150, used: 0, demand: 0, unpowered: 0, waiting: 0 },
      balance: 100_000,
      taxes: 0,
      grant: 0,
      upkeep: 0,
      driving: 0,
      parked: 0,
      walking: 0,
      walksCompleted: 0,
      tripsCompleted: 0,
      despawnedStuck: 0,
      pathsFailed: 0,
      despawnedByEdit: 0,
    },
    tickMs: 0,
  };
  t.reply(frame);
  t.pressId('category-roads');
  t.pressId('street');
  assert.equal(t.canvas.style.cursor, 'crosshair');
  t.reply({ type: 'cityReset', speed: 0 });
  assert.equal(t.canvas.style.cursor, '');
  assert.equal(t.elements['items-roads'].hidden, true, 'the build menu closes');
  assert.deepEqual({ ...t.messages.at(-1) }, { type: 'setSpeed', speed: 0 });
  assert.equal(t.elements['speed-0'].attributes['aria-pressed'], 'true', 'pause shows');
  assert.equal(t.elements['speed-1'].attributes['aria-pressed'], 'false');
  assert.equal(t.previews.filter((p) => p.type === 'zoningVisible').at(-1).visible, false);
  t.reply(frame);
  t.frame();
  assert.equal(
    t.cars.draws.at(-1)[0],
    undefined,
    'new city does not interpolate from the old city',
  );
  assert.equal(t.cars.following.at(-1), false);
});
