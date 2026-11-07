import assert from 'node:assert/strict';
import test from 'node:test';
import { tools } from './main-harness.mjs';

const home = { building: 9, zone: 2, capacity: 84, occupied: 53, closing: false, watered: true };
const asks = (t) => t.messages.filter((m) => m.type === 'inspectBuilding');
const text = (t) => t.card();
function select(t, building = home) {
  t.buildings.at = building.building;
  t.click(5, 5);
  const ask = asks(t).at(-1);
  assert.ok(ask, 'a building is selected');
  assert.equal(ask.building, building.building);
  t.reply({ type: 'building', selection: ask.selection, building });
  return ask;
}

test('homes show live resident counts; workplaces show total, filled and unfilled jobs', async () => {
  const t = await tools();
  const ask = select(t);
  assert.equal(t.elements.inspector.hidden, false);
  assert.match(text(t), /^Apartments #9$/m);
  assert.match(text(t), /^High density$/m);
  assert.match(text(t), /^Resident capacity: 84$/m);
  assert.match(text(t), /^Residents: 53$/m);
  const lines = t.elements['inspector-lines'].children;
  assert.ok(
    lines.every((li) => li.querySelector('svg')?.getAttribute('aria-hidden') === 'true'),
    'each line has an icon that screen readers skip',
  );
  assert.equal(t.elements.follow.hidden, true);
  t.key('f');
  assert.notEqual(t.elements.follow.attributes['aria-pressed'], 'true');
  t.reply({ type: 'building', selection: ask.selection, building: { ...home, occupied: 60 } });
  assert.match(text(t), /^Residents: 60$/m);
  assert.doesNotMatch(text(t), /water/);
  t.reply({ type: 'building', selection: ask.selection, building: { ...home, watered: false } });
  assert.match(text(t), /^No water: no one moves in, and people leave after a day$/m);
  for (const zone of [3, 4, 5, 6]) {
    const ask = select(t, { ...home, building: zone + 20, zone, capacity: 120, occupied: 71 });
    assert.match(text(t), /^Total jobs: 120$/m);
    assert.match(text(t), /^Filled jobs: 71$/m);
    assert.match(text(t), /^Unfilled jobs: 49$/m);
    t.reply({
      type: 'building',
      selection: ask.selection,
      building: { ...home, zone, closing: true },
    });
    assert.match(text(t), /^Closing/m);
  }
});

test('building and citizen selections replace each other; stale replies and removal cannot reopen panels', async () => {
  const t = await tools();
  const first = select(t);
  t.cars.at = 3;
  t.click(5, 5);
  const citizenAsk = t.messages.at(-1);
  assert.equal(citizenAsk.type, 'inspect', 'citizens take priority over buildings');
  t.reply({ type: 'building', selection: first.selection, building: home });
  assert.equal(t.elements.inspector.hidden, true);
  t.reply({
    type: 'citizen',
    selection: citizenAsk.selection,
    citizen: {
      id: 1,
      name: 'Ada Moreno',
      resident: true,
      agent: 3,
      activity: 0,
      home: null,
      job: null,
      destination: null,
      location: null,
      driving: false,
      onFoot: false,
    },
  });
  assert.equal(t.elements.follow.hidden, false);
  t.key('f');
  t.cars.at = null;
  const second = select(t);
  assert.equal(t.elements.follow.attributes['aria-pressed'], 'false');
  t.reply({ type: 'citizen', selection: citizenAsk.selection, citizen: null });
  assert.equal(t.elements.inspector.hidden, false);
  t.reply({ type: 'building', selection: second.selection, building: null });
  assert.equal(t.elements.inspector.hidden, true, 'demolition closes the panel');
  const third = select(t);
  t.key('Escape');
  t.reply({ type: 'building', selection: third.selection, building: home });
  assert.equal(t.elements.inspector.hidden, true);
  select(t);
  t.pressId('inspector-close');
  assert.equal(t.elements.inspector.hidden, true);
  select(t);
  t.buildings.at = null;
  t.click(5, 5);
  assert.equal(t.elements.inspector.hidden, true, 'empty ground clears');
});

test('tools and pan drags never select buildings', async () => {
  const t = await tools();
  t.buildings.at = 9;
  for (const key of ['r', 'v', 'b', 'z']) {
    t.key(key);
    t.click(5, 5);
    assert.equal(asks(t).length, 0);
    t.key('Escape');
  }
  t.down(5, 5);
  t.up(50, 50);
  assert.equal(asks(t).length, 0);
});
