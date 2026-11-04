import assert from 'node:assert/strict';
import { test } from 'node:test';
import { worker } from './worker-harness.mjs';

function memory(record = null) {
  return {
    record,
    writes: [],
    failRead: false,
    async read() {
      if (this.failRead) throw new Error('Storage is unavailable.');
      return this.record;
    },
    async write(value, replace) {
      this.writes.push({ value, replace });
      this.record = value;
    },
  };
}
const latest = (t, type) => t.messages.filter((m) => m.type === type).at(-1);
async function start(store = memory()) {
  const t = await worker(12, 3, store);
  t.ready();
  await t.init;
  return t;
}
async function shared() {
  const a = await start();
  await a.send({ type: 'fillBlock', x: 50, y: 50, zone: 1 });
  a.advance(1500);
  await a.send({ type: 'exportCity' });
  return { a, file: latest(a, 'cityFile') };
}

test('a shared city opens as its own paused, saved copy that keeps building', async () => {
  const { a, file } = await shared();
  assert.equal(file.error, null);
  assert.ok(file.bytes.length > 0);
  const sent = latest(a, 'frame').stats;

  const store = memory();
  const b = await start(store);
  b.advance(300);
  await b.send({ type: 'openCity', bytes: file.bytes });
  assert.equal(latest(b, 'cityReset').speed, 0, 'opens paused');
  assert.deepEqual(structuredClone(latest(b, 'frame').stats), structuredClone(sent));
  assert.deepEqual(latest(b, 'zones').cells, latest(a, 'zones').cells);
  const status = latest(b, 'cityStatus');
  assert.equal(status.ready, true);
  assert.equal(status.error, null);
  assert.equal(status.dirty, false);
  assert.equal(store.writes.length, 1);
  assert.equal(store.writes[0].replace, true, 'a confirmed open replaces the local slot');

  // The friend keeps building their copy; the sender's city is untouched.
  await b.send({ type: 'paintZone', ax: -1000, ay: -1000, bx: 1000, by: 1000, zone: 5 });
  await b.send({ type: 'setSpeed', speed: 1 });
  b.advance(1000);
  assert.ok(latest(b, 'frame').stats.tick > sent.tick);
  assert.notDeepEqual(latest(b, 'zones').cells, latest(a, 'zones').cells);
  assert.equal(latest(b, 'cityStatus').dirty, true);
  // A reload resumes the friend's copy from their own storage.
  await b.send({ type: 'saveCity' });
  const reloaded = await start(store);
  assert.deepEqual(latest(reloaded, 'zones').cells, latest(b, 'zones').cells);
});

test('a file the sim rejects changes neither the city nor its save', async () => {
  const store = memory();
  const t = await start(store);
  await t.send({ type: 'saveCity' });
  const saved = store.record;
  t.advance(500);
  const before = latest(t, 'frame').stats.tick;
  for (const bytes of [new Uint8Array([1, 2, 3]), new TextEncoder().encode('CITYSAVE junk')]) {
    await t.send({ type: 'openCity', bytes });
    const status = latest(t, 'cityStatus');
    assert.match(status.error, /Could not open that city .*current city is unchanged/);
    assert.equal(status.ready, true);
  }
  assert.equal(store.record, saved);
  assert.equal(store.writes.length, 1);
  t.advance(500);
  assert.ok(latest(t, 'frame').stats.tick > before, 'the current city keeps running');
});

test('a shared city can replace an unreadable save only through an explicit open', async () => {
  const { file } = await shared();
  const store = memory({ bytes: new Uint8Array([9]), savedAt: 1 });
  const t = await start(store);
  assert.equal(latest(t, 'cityStatus').ready, false);
  await t.send({ type: 'openCity', bytes: file.bytes });
  assert.equal(latest(t, 'cityStatus').ready, true);
  assert.equal(store.writes.length, 1);
  assert.ok(store.record.bytes.length > 1, 'the shared city replaced the damaged slot');
});

test('exporting before a city is ready reports an error instead of bytes', async () => {
  const t = await worker(12, 3, memory({ bytes: new Uint8Array([9]), savedAt: 1 }));
  t.ready();
  await t.init;
  await t.send({ type: 'exportCity' });
  const file = latest(t, 'cityFile');
  assert.equal(file.bytes, null);
  assert.match(file.error, /still opening/);
});

test('fresh cities record a replayable command log; opened copies have none', async () => {
  const { a, file } = await shared();
  await a.send({ type: 'buildRoad', id: 1, ax: 10, ay: 10.25, bx: 200, by: 10.25, class: 1 });
  await a.send({ type: 'commandLog' });
  const lines = latest(a, 'commandLog').text.trim().split('\n');
  const tick = latest(a, 'frame').stats.tick;
  assert.deepEqual(lines.slice(0, 3), ['sim-game command log 1', 'seed 1', 'grid 3 3']);
  assert.equal(lines[3], '0 spawn 12');
  assert.equal(lines[4], '0 fill 50 50 residential-low');
  assert.equal(lines[5], `${tick} road 10 10.25 200 10.25 main`);
  assert.equal(lines.at(-1), `end ${tick}`);

  await a.send({ type: 'openCity', bytes: file.bytes });
  await a.send({ type: 'commandLog' });
  assert.equal(latest(a, 'commandLog').text, null);
});
