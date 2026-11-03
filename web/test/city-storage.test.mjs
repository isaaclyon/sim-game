import assert from 'node:assert/strict';
import { test } from 'node:test';

let identity = 0;
async function storage() {
  const transactions = [];
  let opens = 0;
  const db = {
    close() {},
    transaction(_name, mode) {
      const tx = {
        mode,
        error: null,
        request: {},
        value: null,
        abort() {
          tx.onabort();
        },
        objectStore() {
          return {
            get: () => tx.request,
            put: (value) => {
              tx.value = value;
              return tx.request;
            },
          };
        },
      };
      transactions.push(tx);
      return tx;
    },
  };
  const original = globalThis.indexedDB;
  globalThis.indexedDB = {
    open: () => {
      opens++;
      const request = { result: db };
      queueMicrotask(() => request.onsuccess());
      return request;
    },
  };
  const { cityStorage } = await import(`../src/city-storage.ts?test=${identity++}`);
  return {
    cityStorage,
    transactions,
    db,
    opens: () => opens,
    fork: async () => (await import(`../src/city-storage.ts?test=${identity++}`)).cityStorage,
    restore: () => {
      globalThis.indexedDB = original;
    },
  };
}
const flush = () => new Promise(setImmediate);

test('saving waits for transaction commit, not the individual write request', async () => {
  const t = await storage();
  try {
    let done = false;
    const value = { bytes: new Uint8Array([1]), savedAt: 123 };
    const saved = t.cityStorage.write(value).then(() => {
      done = true;
    });
    await flush();
    const tx = t.transactions[0];
    assert.equal(tx.mode, 'readwrite');
    tx.request.onsuccess();
    assert.deepEqual(tx.value, { ...value, revision: 1 });
    await flush();
    assert.equal(done, false);
    tx.oncomplete();
    await saved;
    assert.equal(done, true);
  } finally {
    t.restore();
  }
});

test('an aborted write rejects even after request success', async () => {
  const t = await storage();
  try {
    const saved = t.cityStorage.write({ bytes: new Uint8Array([1]), savedAt: 123 });
    const rejection = assert.rejects(saved, /full/);
    await flush();
    const tx = t.transactions[0];
    tx.request.onsuccess();
    tx.error = new DOMException('quota', 'QuotaExceededError');
    tx.onabort();
    await rejection;
  } finally {
    t.restore();
  }
});

test('missing, malformed and valid stored records are distinct', async () => {
  const t = await storage();
  try {
    for (const [record, valid] of [
      [undefined, true],
      [{ bytes: 'bad', savedAt: 123 }, false],
      [{ bytes: new Uint8Array([1]), savedAt: NaN }, false],
      [{ bytes: new Uint8Array([1]), savedAt: 123, revision: 1 }, true],
    ]) {
      const reading = t.cityStorage.read();
      const result = valid ? reading : assert.rejects(reading, /damaged/);
      await flush();
      const tx = t.transactions.at(-1);
      tx.request.result = record;
      tx.oncomplete();
      const value = await result;
      if (valid) assert.equal(value, record ?? null);
    }
  } finally {
    t.restore();
  }
});

test('an older tab cannot overwrite another tab’s new city or newer save', async () => {
  const t = await storage();
  try {
    const a = t.cityStorage;
    const b = await t.fork();
    const old = { bytes: new Uint8Array([1]), savedAt: 1, revision: 1 };
    for (const tab of [a, b]) {
      const reading = tab.read();
      await flush();
      const tx = t.transactions.at(-1);
      tx.request.result = old;
      tx.oncomplete();
      await reading;
    }
    const replacement = { bytes: new Uint8Array([2]), savedAt: 2 };
    const saving = b.write(replacement, true);
    await flush();
    let tx = t.transactions.at(-1);
    tx.request.result = old;
    tx.request.onsuccess();
    const current = tx.value;
    tx.oncomplete();
    await saving;
    assert.equal(current.revision, 2);

    const stale = a.write({ bytes: new Uint8Array([3]), savedAt: 3 });
    const rejected = assert.rejects(stale, /another tab.*Reload/);
    await flush();
    tx = t.transactions.at(-1);
    tx.request.result = current;
    tx.request.onsuccess();
    await rejected;
    assert.equal(tx.value, null, 'the outdated transaction never replaces the record');
  } finally {
    t.restore();
  }
});

for (const anotherTabSaved of [false, true]) {
  test(`retrying a failed new-city save ${anotherTabSaved ? 'rejects a changed slot' : 'replaces the unchanged slot'}`, async () => {
    const t = await storage();
    try {
      const old = { bytes: new Uint8Array([1]), savedAt: 1, revision: 7 };
      const replacement = { bytes: new Uint8Array([2]), savedAt: 2 };
      // Startup could not read the slot; only confirmed New city authorizes replacement.
      const saving = t.cityStorage.write(replacement, true);
      const failed = assert.rejects(saving, /full/);
      await flush();
      let tx = t.transactions.at(-1);
      tx.request.result = old;
      tx.request.onsuccess();
      tx.error = new DOMException('quota', 'QuotaExceededError');
      tx.onabort();
      await failed;

      const retry = t.cityStorage.write(replacement);
      const result = anotherTabSaved ? assert.rejects(retry, /another tab/) : retry;
      await flush();
      tx = t.transactions.at(-1);
      tx.request.result = anotherTabSaved ? { ...old, revision: 8 } : old;
      tx.request.onsuccess();
      if (!anotherTabSaved) tx.oncomplete();
      await result;
      assert.equal(tx.value?.revision ?? null, anotherTabSaved ? null : 8);
    } finally {
      t.restore();
    }
  });
}

test('an unexpected database close lets the next save reopen storage', async () => {
  const t = await storage();
  try {
    const value = { bytes: new Uint8Array([1]), savedAt: 1 };
    for (let attempt = 0; attempt < 2; attempt++) {
      const saving = t.cityStorage.write(value);
      await flush();
      const tx = t.transactions.at(-1);
      tx.request.result = attempt ? { ...value, revision: 1 } : undefined;
      tx.request.onsuccess();
      tx.oncomplete();
      await saving;
      if (!attempt) t.db.onclose?.();
    }
    assert.equal(t.opens(), 2, 'retry uses a new database connection');
  } finally {
    t.restore();
  }
});
