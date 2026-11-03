import assert from 'node:assert/strict';
import { test } from 'node:test';
import { worker } from './worker-harness.mjs';

function memory() {
  return {
    record: null,
    writes: 0,
    failRead: false,
    failWrite: false,
    async read() {
      if (this.failRead) throw new Error('Storage is unavailable.');
      return this.record;
    },
    async write(record) {
      this.writes++;
      if (this.failWrite) throw new Error('Storage is full.');
      this.record = record;
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

test('manual save commits a city and reload restores it paused with complete snapshots', async () => {
  const store = memory();
  const a = await start(store);
  a.advance(800);
  await a.send({ type: 'saveCity' });
  const frame = latest(a, 'frame');
  assert.ok(store.record.bytes.length > 0);
  assert.equal(latest(a, 'cityStatus').dirty, false);
  const b = await start(store);
  assert.deepEqual(latest(b, 'frame').agents, frame.agents);
  assert.deepEqual(structuredClone(latest(b, 'frame').stats), structuredClone(frame.stats));
  for (const type of ['roads', 'zones', 'lots', 'buildings']) assert.ok(latest(b, type));
  assert.equal(latest(b, 'cityReset').speed, 0);
  b.advance(2000);
  assert.equal(latest(b, 'frame').stats.tick, frame.stats.tick);
});

test('autosave includes paused edits, skips unchanged worlds, and ignores previews', async () => {
  const store = memory();
  const t = await start(store);
  await t.send({ type: 'setSpeed', speed: 0 });
  await t.send({ type: 'saveCity' });
  t.advance(60000);
  await t.settled();
  assert.equal(store.writes, 1);
  await t.send({ type: 'previewBlock', x: 20, y: 20, revision: 1 });
  t.advance(60000);
  await t.settled();
  assert.equal(store.writes, 1);
  await t.send({ type: 'fillBlock', x: 50, y: 50, zone: 1 });
  assert.equal(latest(t, 'cityStatus').dirty, true);
  t.advance(60000);
  await t.settled();
  assert.equal(store.writes, 2);
  assert.equal(latest(t, 'cityStatus').dirty, false);
});

test('failed saves retain the old city and timestamp and allow a manual retry', async () => {
  const store = memory();
  const t = await start(store);
  await t.send({ type: 'saveCity' });
  const old = store.record;
  t.advance(500);
  store.failWrite = true;
  await t.send({ type: 'saveCity' });
  assert.equal(store.record, old);
  assert.equal(latest(t, 'cityStatus').savedAt, old.savedAt);
  assert.equal(latest(t, 'cityStatus').dirty, true);
  assert.match(latest(t, 'cityStatus').error, /full/);
  store.failWrite = false;
  await t.send({ type: 'saveCity' });
  assert.notEqual(store.record, old);
  assert.equal(latest(t, 'cityStatus').error, null);
});

test('edits made during an in-flight save remain unsaved after its commit', async () => {
  const store = memory();
  const t = await start(store);
  let finish;
  store.write = (record) =>
    new Promise((resolve) => {
      finish = () => {
        store.record = record;
        resolve();
      };
    });
  const saving = t.send({ type: 'saveCity' });
  await Promise.resolve();
  await Promise.resolve();
  assert.equal(latest(t, 'cityStatus').busy, true);
  t.advance(500);
  finish();
  await saving;
  assert.equal(latest(t, 'cityStatus').dirty, true);
});

for (const bad of ['corrupt', 'unreadable']) {
  test(`${bad} storage blocks automatic overwrite until an explicit new-city request`, async () => {
    const store = memory();
    store.record = { bytes: new Uint8Array([1, 2, 3]), savedAt: 1 };
    store.failRead = bad === 'unreadable';
    const old = store.record;
    const t = await start(store);
    assert.equal(latest(t, 'cityStatus').ready, false);
    assert.ok(latest(t, 'cityStatus').error);
    t.advance(60000);
    await t.send({ type: 'saveCity' });
    assert.equal(store.writes, 0);
    assert.equal(store.record, old);
    assert.equal(latest(t, 'frame'), undefined);
    store.failRead = false;
    await t.send({ type: 'newCity' });
    assert.equal(latest(t, 'cityStatus').ready, true);
    assert.equal(latest(t, 'frame').stats.tick, 0);
    assert.equal(store.writes, 1);
  });
}

test('new city waits for old writes and resets snapshots, selections and the clock once', async () => {
  const store = memory();
  const t = await start(store);
  t.advance(900);
  let finish;
  const write = store.write.bind(store);
  store.write = (record) =>
    new Promise((resolve) => {
      finish = async () => {
        await write(record);
        resolve();
      };
    });
  const saving = t.send({ type: 'saveCity' });
  await Promise.resolve();
  await Promise.resolve();
  const replacing = t.send({ type: 'newCity' });
  store.write = write;
  await finish();
  await saving;
  await replacing;
  assert.equal(store.writes, 2);
  assert.equal(latest(t, 'frame').stats.tick, 0);
  assert.equal(latest(t, 'frame').stats.agents, 12);
  assert.equal(t.messages.filter((m) => m.type === 'roads').length, 2);
  const restored = await start(store);
  assert.equal(latest(restored, 'frame').stats.tick, 0);
  t.advance(101);
  assert.equal(latest(t, 'frame').stats.tick, 1);
  await t.send({ type: 'newCity' });
  t.advance(101);
  assert.equal(latest(t, 'frame').stats.tick, 1);
});

test('a failed new-city write keeps the old save, reports unsaved progress and allows retry', async () => {
  const store = memory();
  const t = await start(store);
  t.advance(900);
  await t.send({ type: 'saveCity' });
  const old = store.record;
  store.failWrite = true;
  await t.send({ type: 'newCity' });
  assert.equal(store.record, old);
  assert.equal(latest(t, 'frame').stats.tick, 0);
  const status = latest(t, 'cityStatus');
  assert.equal(status.ready, true, 'the unsaved new city is playable');
  assert.equal(status.dirty, true);
  assert.equal(status.savedAt, null, 'the old timestamp does not acknowledge the new city');
  assert.match(status.error, /previous stored city is unchanged/);
  store.failWrite = false;
  await t.send({ type: 'saveCity' });
  assert.equal(latest(t, 'cityStatus').dirty, false);
  const restored = await start(store);
  assert.equal(latest(restored, 'frame').stats.tick, 0);
});
