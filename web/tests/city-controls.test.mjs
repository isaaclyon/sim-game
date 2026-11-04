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
  save.onclick();
  assert.deepEqual({ ...t.messages.at(-1) }, { type: 'saveCity' });
  assert.match(t.elements['save-status'].textContent, /Unsaved/);
  t.reply(status({ error: 'Browser storage is full.', dirty: true }));
  assert.match(t.elements['save-status'].textContent, /full/);
  assert.equal(save.disabled, false);
  t.reply(status());
  assert.match(t.elements['save-status'].textContent, /Saved/);
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
  t.elements.road.onclick();
  t.reply({ type: 'cityReset', speed: 0 });
  assert.equal(t.canvas.style.cursor, '');
  assert.deepEqual({ ...t.messages.at(-1) }, { type: 'setSpeed', speed: 0 });
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
