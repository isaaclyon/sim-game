import assert from 'node:assert/strict';
import test from 'node:test';

import { Activity, Zone } from '../src/protocol.ts';
import { tools } from './main-harness.mjs';

const resident = {
  id: 12,
  name: 'Ada Moreno',
  resident: true,
  home: { building: 4, zone: Zone.ResidentialLow },
  job: { building: 40, zone: Zone.IndustrialLow },
  activity: Activity.Work,
  location: { building: 4, zone: Zone.ResidentialLow },
  destination: { building: 40, zone: Zone.IndustrialLow },
  agent: 3,
  driving: true,
  onFoot: false,
  age: 34,
  child: false,
  grewUp: false,
  wantsHome: false,
  mood: 62,
  complaints: [],
  commute: 0,
};
const inspects = (t) => t.messages.filter((m) => m.type === 'inspect');
/** Clicks car `agent` in idle mode and answers as the worker would. */
function pickCar(t, agent, citizen = { ...resident, agent }) {
  t.cars.at = agent;
  t.click(5, 5);
  const ask = inspects(t).at(-1);
  t.reply({ type: 'citizen', selection: ask.selection, citizen });
  return ask;
}
const panel = (t) => t.card();
const showCars = (t) =>
  t.reply({
    type: 'frame',
    agents: new Float32Array(40),
    signals: new Float32Array(),
    tickMs: 0,
    stats: {
      day: 1,
      secondOfDay: 0,
      population: 0,
      children: 0,
      unemployed: 0,
      unemploymentPercent: 0,
      demand: { residential: 0, commercial: 0, industrial: 0 },
      power: { supply: 0, starter: 150, used: 0, demand: 0, unpowered: 0, waiting: 0 },
      water: { supply: 0, used: 0, demand: 0, stored: 0, capacity: 0, dry: 0, waiting: 0 },
      fire: {
        enabled: true,
        threshold: 1000,
        stations: 0,
        trucks: 0,
        freeTrucks: 0,
        burning: 0,
        unattended: 0,
        covered: 0,
        saved: 0,
        destroyed: 0,
      },
      happiness: { average: 50, complaints: [0, 0, 0, 0, 0, 0, 0] },
      balance: 100_000,
      taxes: 0,
      grant: 0,
      upkeep: 0,
      utilities: 0,
      driving: 0,
      parked: 0,
      walking: 0,
      walksCompleted: 0,
      tripsCompleted: 0,
      despawnedStuck: 0,
      pathsFailed: 0,
      despawnedByEdit: 0,
    },
  });
const step = (t, ms = 16) => {
  t.clock.now += ms;
  t.frame();
};

test('clicking a car in idle mode shows its resident from the sim', async () => {
  const t = await tools();
  assert.equal(t.elements.inspector.hidden, true, 'no panel before a selection');
  const ask = pickCar(t, 3);
  assert.equal(ask.agent, 3);
  assert.equal(t.elements.inspector.hidden, false);
  assert.match(panel(t), /^Ada Moreno$/m);
  assert.equal(
    t.elements.inspector.getAttribute('aria-labelledby'),
    'inspector-title',
    'screen readers name the card by its title',
  );
  assert.match(panel(t), /^Home: house #4$/m);
  assert.match(panel(t), /^Job: factory #40$/m);
  assert.match(panel(t), /^Driving to work at factory #40$/m);
  showCars(t);
  step(t);
  assert.equal(t.cars.drawnSelected.at(-1), 3, 'the selected car is highlighted');
});

test('the panel tells the truth about each state', async () => {
  const t = await tools();
  const ask = pickCar(t, 3);
  const show = (changes) => {
    t.reply({ type: 'citizen', selection: ask.selection, citizen: { ...resident, ...changes } });
    return panel(t);
  };
  assert.match(show({ job: null }), /^Job: unemployed$/m);
  assert.match(show({}), /^Age 34$/m);
  assert.match(show({}), /^Mood: Content \(62\)$/m);
  assert.doesNotMatch(show({}), /Complaints/, 'nothing to complain about');
  const unhappy = show({ mood: 21, complaints: [0, 3, 6], commute: 75 });
  assert.match(unhappy, /^Mood: Miserable \(21\)$/m);
  assert.match(unhappy, /^Complaints: no power, dirty air, long commute$/m);
  assert.match(unhappy, /^Job: factory #40, 75 min commute$/m);
  assert.match(
    show({ age: 19, grewUp: true, wantsHome: true }),
    /^Age 19, grew up here: looking for a home of their own$/m,
  );
  assert.match(show({ age: 21, grewUp: true }), /^Age 21, grew up here: has a home of their own$/m);
  const child = show({
    job: null,
    age: 7,
    child: true,
    activity: Activity.Home,
    destination: null,
  });
  assert.match(child, /^Age 7, a child: lives at home$/m);
  assert.doesNotMatch(child, /Job:/);
  assert.match(child, /^At home$/m);
  assert.match(show({ driving: false, onFoot: true }), /^Walking to work at factory #40$/m);
  assert.match(
    show({ driving: false }),
    /^Waiting to drive to work at factory #40$/m,
    'a trip that has not reached the road yet',
  );
  assert.match(show({ driving: false, destination: null, activity: Activity.Home }), /^At home$/m);
  assert.match(
    show({
      driving: false,
      destination: null,
      activity: Activity.Shop,
      location: { building: 9, zone: Zone.CommercialHigh },
    }),
    /^Shopping at store #9$/m,
  );
  const gone = show({
    resident: false,
    home: null,
    job: null,
    location: null,
    destination: null,
    agent: null,
    driving: false,
  });
  assert.match(gone, /Ada Moreno/);
  assert.match(gone, /^Left town$/m);
  assert.doesNotMatch(gone, /house|factory/, 'no stale places after leaving');
  assert.equal(t.elements.follow.disabled, true, 'nobody to follow');
});

test('a live update rewrites only what changed, inside the region screen readers announce', async () => {
  const t = await tools();
  const ask = pickCar(t, 3);
  const live = t.elements['inspector-body'];
  assert.equal(live.getAttribute('aria-live'), 'polite');
  assert.ok(live.contains(t.elements['inspector-title']), 'the name is read with the lines');
  assert.ok(live.contains(t.elements['inspector-lines']));
  const lines = () => t.elements['inspector-lines'].children;
  const texts = () => lines().map((li) => li.querySelector('span'));
  const before = texts();
  for (let i = 0; i < 5; i++)
    t.reply({ type: 'citizen', selection: ask.selection, citizen: { ...resident } });
  assert.deepEqual(texts(), before, 'unchanged details are not rewritten');
  const kept = before.map((span) => span.childNodes[0]);
  t.reply({
    type: 'citizen',
    selection: ask.selection,
    citizen: { ...resident, destination: null, location: resident.job },
  });
  assert.match(panel(t), /^At work, factory #40$/m);
  const changed = texts().filter((span, i) => span.childNodes[0] !== kept[i]);
  assert.equal(changed.length, 1, 'only the line that changed is read again');
});

test('stale replies, empty ground, Esc and close deselect predictably', async () => {
  const t = await tools();
  const first = pickCar(t, 3);
  t.cars.at = 8;
  t.click(5, 5);
  t.reply({ type: 'citizen', selection: first.selection, citizen: resident });
  assert.notEqual(inspects(t).at(-1).selection, first.selection);
  assert.equal(t.elements.inspector.hidden, true, 'a reply to an older selection is ignored');
  t.reply({
    type: 'citizen',
    selection: inspects(t).at(-1).selection,
    citizen: { ...resident, agent: 8 },
  });
  assert.equal(t.elements.inspector.hidden, false);
  t.cars.at = null;
  t.click(5, 5);
  assert.equal(inspects(t).at(-1).agent, null, 'clicking empty ground deselects');
  assert.equal(t.elements.inspector.hidden, true);
  pickCar(t, 3);
  t.key('Escape');
  assert.equal(inspects(t).at(-1).agent, null, 'Esc deselects');
  assert.equal(t.elements.inspector.hidden, true);
  pickCar(t, 3);
  t.pressId('inspector-close');
  assert.equal(t.elements.inspector.hidden, true, 'close deselects');
  const count = t.messages.length;
  t.cars.at = null;
  t.click(5, 5);
  t.key('Escape');
  assert.equal(t.messages.length, count, 'nothing to clear twice');
  pickCar(t, 3, null);
  assert.equal(t.elements.inspector.hidden, true, 'a car without a resident shows nothing');
});

test('cars are picked only without a tool, and drags still pan', async () => {
  const t = await tools();
  t.cars.at = 3;
  for (const key of ['r', 'v', 'b', 'z']) {
    t.key(key);
    const count = inspects(t).length;
    t.click(5, 5);
    assert.equal(inspects(t).length, count, `${key} keeps its clicks`);
    t.key('Escape');
  }
  const count = inspects(t).length;
  t.down(5, 5);
  t.up(50, 50);
  assert.equal(inspects(t).length, count, 'a drag pans instead of selecting');
  pickCar(t, 3);
  t.key('r');
  assert.equal(t.elements.inspector.hidden, false, 'tools leave the selection alone');
  t.key('Escape');
  assert.equal(t.elements.inspector.hidden, false, 'Esc first cancels the tool');
});

test('follow rides with the car, holds while it is hidden and stops when its resident leaves', async () => {
  const t = await tools();
  const ask = pickCar(t, 3);
  step(t);
  assert.equal(t.cars.follows.length, 0, 'selecting does not move the camera');
  t.key('f');
  assert.equal(t.elements.follow.attributes['aria-pressed'], 'true');
  assert.equal(t.cars.following.at(-1), true, 'zoom stays on the car');
  assert.ok(t.cameraPans.includes('stopPan'), 'leftover keyboard pan speed is cleared');
  step(t, 20);
  assert.deepEqual(t.cars.follows.at(-1), [3, 0.02]);
  t.cars.visible = false;
  t.reply({
    type: 'citizen',
    selection: ask.selection,
    citizen: { ...resident, driving: false, destination: null, location: resident.job },
  });
  step(t);
  assert.equal(t.elements.follow.attributes['aria-pressed'], 'true', 'an arrival holds follow');
  assert.match(panel(t), /^At work, factory #40$/m);
  t.reply({
    type: 'citizen',
    selection: ask.selection,
    citizen: { ...resident, resident: false, agent: null, home: null, job: null, location: null },
  });
  assert.equal(t.elements.follow.attributes['aria-pressed'], 'false', 'leaving town ends follow');
  assert.equal(t.cars.following.at(-1), false);
  const follows = t.cars.follows.length;
  step(t);
  assert.equal(t.cars.follows.length, follows, 'never tracks a departed slot');
});

test('panning releases follow; turning and zooming keep it', async () => {
  const t = await tools();
  pickCar(t, 3);
  t.pressId('follow');
  t.key('q');
  step(t);
  t.keyup('q');
  assert.equal(t.elements.follow.attributes['aria-pressed'], 'true', 'Q/E orbit the car');
  t.key('w');
  step(t);
  assert.equal(t.elements.follow.attributes['aria-pressed'], 'false', 'WASD takes the camera back');
  assert.equal(t.cars.following.at(-1), false);
  t.keyup('w');
  t.key('f');
  t.userPan();
  assert.equal(t.elements.follow.attributes['aria-pressed'], 'false', 'a mouse drag does too');
  t.key('f');
  t.key('f');
  assert.equal(t.elements.follow.attributes['aria-pressed'], 'false', 'F toggles');
});

test('a pan key held when follow starts waits for release instead of fighting it', async () => {
  const t = await tools();
  pickCar(t, 3);
  t.key('d');
  step(t);
  t.key('f');
  t.key('d', { repeat: true });
  step(t);
  assert.equal(t.elements.follow.attributes['aria-pressed'], 'true');
  assert.deepEqual(t.cameraPans.at(-1).slice(0, 3), [0, 0, 0]);
  t.keyup('d');
  t.key('d');
  step(t);
  assert.equal(t.elements.follow.attributes['aria-pressed'], 'false', 'a fresh press pans again');
});

test('F obeys the same guards as other shortcuts and needs a selection', async () => {
  const t = await tools();
  t.key('f');
  assert.equal(t.elements.follow.attributes['aria-pressed'], 'false', 'nothing selected');
  pickCar(t, 3);
  const input = { closest: (selector) => (selector.includes('input') ? input : null) };
  for (const options of [
    { ctrlKey: true },
    { metaKey: true },
    { altKey: true },
    { repeat: true },
    { isComposing: true },
    { target: input },
  ]) {
    t.key('f', options);
    assert.notEqual(t.elements.follow.attributes['aria-pressed'], 'true');
  }
  t.key('F');
  assert.equal(t.elements.follow.attributes['aria-pressed'], 'true');
});

test('a new selection drops follow and the old car', async () => {
  const t = await tools();
  pickCar(t, 3);
  t.key('f');
  pickCar(t, 8);
  assert.equal(t.elements.follow.attributes['aria-pressed'], 'false');
  showCars(t);
  step(t);
  assert.equal(t.cars.drawnSelected.at(-1), 8);
  assert.match(panel(t), /Ada Moreno/);
});
