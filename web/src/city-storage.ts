/** One local city. Transaction completion, not request success, acknowledges a save. */
export interface CityRecord {
  bytes: Uint8Array;
  savedAt: number;
}
export interface CityStorage {
  read(): Promise<CityRecord | null>;
  /** Only a confirmed New city may replace a save changed in another tab. */
  write(record: CityRecord, replace?: boolean): Promise<void>;
}

type StoredCity = CityRecord & { revision: number };
let expectedRevision: number | null = null;
let replacementPending = false;
const validRevision = (value: unknown): value is number =>
  typeof value === 'number' && Number.isSafeInteger(value) && value > 0;

let database: Promise<IDBDatabase> | undefined;
function open(): Promise<IDBDatabase> {
  if (database) return database;
  database = new Promise<IDBDatabase>((resolve, reject) => {
    const request = indexedDB.open('sim-game-city', 1);
    let finished = false;
    request.onupgradeneeded = () => request.result.createObjectStore('cities');
    request.onblocked = () => {
      finished = true;
      reject(new Error('City storage is blocked. Close other game tabs and reload.'));
    };
    request.onerror = () => reject(request.error);
    request.onsuccess = () => {
      const db = request.result;
      if (finished) return db.close();
      db.onclose = () => {
        database = undefined;
      };
      db.onversionchange = () => {
        db.close();
        database = undefined;
      };
      resolve(db);
    };
  }).catch((error) => {
    database = undefined;
    throw error;
  });
  return database;
}

function failure(error: DOMException | null): Error {
  return new Error(
    error?.name === 'QuotaExceededError'
      ? 'Browser storage is full. Free some storage and try Save city again.'
      : 'Browser storage is unavailable. Check browser permissions and try again.',
  );
}

export const cityStorage: CityStorage = {
  async read() {
    const db = await open();
    return new Promise((resolve, reject) => {
      const tx = db.transaction('cities', 'readonly');
      const request = tx.objectStore('cities').get('current');
      tx.onabort = () => reject(failure(tx.error));
      tx.onerror = () => reject(failure(tx.error));
      tx.oncomplete = () => {
        const record = request.result;
        if (record === undefined) {
          expectedRevision = null;
          return resolve(null);
        }
        if (
          !(record?.bytes instanceof Uint8Array) ||
          !Number.isFinite(record.savedAt) ||
          record.savedAt < 0 ||
          !validRevision(record.revision)
        ) {
          reject(new Error('The saved city is damaged. It has not been overwritten.'));
        } else {
          expectedRevision = record.revision;
          resolve(record);
        }
      };
    });
  },
  async write(record, replace = false) {
    if (replace) replacementPending = true;
    const db = await open();
    return new Promise((resolve, reject) => {
      const tx = db.transaction('cities', 'readwrite');
      const store = tx.objectStore('cities');
      let revision: number;
      tx.onabort = () => reject(failure(tx.error));
      tx.onerror = () => reject(failure(tx.error));
      tx.oncomplete = () => {
        expectedRevision = revision;
        resolve();
      };
      const request = store.get('current');
      request.onsuccess = () => {
        const current = request.result;
        const observed = validRevision(current?.revision) ? current.revision : null;
        if (!replacementPending && observed !== expectedRevision) {
          reject(new Error('Your city was saved in another tab. Reload before saving here.'));
          tx.abort();
          return;
        }
        // Remember the acknowledged replacement's base even if its write fails, so a
        // retry is safe only while no other tab has changed that base in the meantime.
        if (replacementPending) {
          expectedRevision = observed;
          replacementPending = false;
        }
        revision = validRevision(observed) && observed < Number.MAX_SAFE_INTEGER ? observed + 1 : 1;
        const value: StoredCity = { ...record, revision };
        store.put(value, 'current');
      };
    });
  },
};
