import assert from 'node:assert/strict';
import { test } from 'node:test';
import { cityFromLink, cityLink, MAX_LINK_LENGTH } from '../src/city-share.ts';
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
const settle = async () => {
  for (let i = 0; i < 20; i++) await new Promise(setImmediate);
};
// Arrays from the page's realm differ only in prototype.
const same = (a, b) => assert.deepEqual(Array.from(a), Array.from(b));
const city = Uint8Array.from({ length: 5000 }, (_, i) => (i * 7) % 13);

test('links carry the save bytes exactly and refuse cities too big to share', async () => {
  const link = await cityLink(city, 'https://example.test/game/#old');
  assert.match(link, /^https:\/\/example\.test\/game\/#city=[A-Za-z0-9_-]+$/);
  same(await cityFromLink(new URL(link).hash), city);
  assert.equal(await cityFromLink('#other'), null);
  await assert.rejects(cityFromLink('#city=not*base64'), /damaged or incomplete/);
  await assert.rejects(cityFromLink(new URL(link).hash.slice(0, 20)), /damaged/);
  const noise = new Uint8Array(MAX_LINK_LENGTH);
  for (let i = 0; i < noise.length; i += 65536)
    crypto.getRandomValues(noise.subarray(i, i + 65536));
  assert.equal(await cityLink(noise, 'https://example.test/'), null);
});

test('share buttons wait for a ready city; download and copy link use exported bytes', async () => {
  const t = await tools();
  const download = t.elements['download-city'];
  const copy = t.elements['copy-link'];
  assert.equal(download.disabled, true);
  assert.equal(t.elements['command-log'].hidden, true, 'the command log is a developer tool');
  t.reply(status({ busy: true }));
  assert.equal(copy.disabled, true);
  t.reply(status());
  download.onclick();
  assert.deepEqual({ ...t.messages.at(-1) }, { type: 'exportCity' });
  await t.reply({ type: 'cityFile', bytes: city, error: null });
  await settle();
  assert.match(t.page.downloads.at(-1).name, /^city-\d+\.city$/);
  assert.match(t.elements['share-status'].textContent, /Downloaded/);

  copy.onclick();
  t.reply({ type: 'cityFile', bytes: city, error: null });
  await settle();
  const link = t.page.copied.at(-1);
  same(await cityFromLink(new URL(link).hash), city);
  assert.equal(t.elements['share-url'].value, link);
  assert.match(t.elements['share-status'].textContent, /Link copied.*own copy/);

  copy.onclick();
  t.reply({ type: 'cityFile', bytes: null, error: 'Could not export the city: busy' });
  await settle();
  assert.equal(t.elements['share-status'].dataset.error, 'true');
  assert.match(t.elements['share-status'].textContent, /Could not export/);
});

test('a refused clipboard leaves the link visible to copy by hand', async () => {
  const t = await tools({ clipboard: false });
  t.reply(status());
  t.elements['copy-link'].onclick();
  t.reply({ type: 'cityFile', bytes: city, error: null });
  await settle();
  assert.equal(t.elements['share-url'].hidden, false);
  assert.match(t.elements['share-url'].value, /#city=/);
  assert.match(t.elements['share-status'].textContent, /Copy the link below/);
});

test('opening a file asks first; cancel sends nothing', async () => {
  const t = await tools();
  t.reply(status({ ready: false, error: 'Saved city is damaged.' }));
  const input = t.elements['open-city-file'];
  assert.equal(t.elements['open-city'].disabled, false, 'a damaged save can be replaced');
  input.files = [{ arrayBuffer: async () => city.buffer }];
  const count = t.messages.length;
  await input.onchange();
  assert.equal(t.messages.length, count);
  assert.match(t.confirm.prompts.at(-1), /replaces your current city/);
  t.confirm.answer = true;
  input.files = [{ arrayBuffer: async () => city.buffer }];
  await input.onchange();
  assert.equal(t.messages.at(-1).type, 'openCity');
  same(t.messages.at(-1).bytes, city);
  t.reply(status({ busy: true }));
  t.reply(status({ dirty: true }));
  assert.match(t.elements['share-status'].textContent, /Opened, paused.*yours/);
});

test('a city link opens once on first visit, then leaves the address', async () => {
  const link = await cityLink(city, 'https://example.test/game/');
  const t = await tools({ hash: new URL(link).hash });
  assert.deepEqual(t.page.replaced, ['/game/']);
  t.reply(status({ busy: true, ready: false }));
  t.reply(status({ savedAt: null, dirty: true }));
  await settle();
  const opens = t.messages.filter((m) => m.type === 'openCity');
  assert.equal(opens.length, 1);
  same(opens[0].bytes, city);
  assert.equal(t.confirm.prompts.length, 0, 'nothing to lose, so no question');
  t.reply(status());
  await settle();
  assert.equal(t.messages.filter((m) => m.type === 'openCity').length, 1);
});

test('a city link asks before replacing a saved city and reports damage', async () => {
  const link = await cityLink(city, 'https://example.test/game/');
  const kept = await tools({ hash: new URL(link).hash });
  kept.reply(status());
  await settle();
  assert.match(kept.confirm.prompts.at(-1), /Open this city/);
  assert.equal(kept.messages.filter((m) => m.type === 'openCity').length, 0);
  assert.match(kept.elements['share-status'].textContent, /Kept your city/);

  const damaged = await tools({ hash: '#city=AAAA' });
  damaged.reply(status({ savedAt: null }));
  await settle();
  assert.match(damaged.elements['share-status'].textContent, /damaged or incomplete/);
  assert.equal(damaged.elements['share-status'].dataset.error, 'true');
});

test('a city link pasted into a running game asks first, even before its first save', async () => {
  const link = await cityLink(city, 'https://example.test/game/');
  const t = await tools();
  t.reply(status({ savedAt: null, dirty: true }));
  t.confirm.answer = true;
  t.page.hash = new URL(link).hash;
  t.event('hashchange');
  await settle();
  assert.deepEqual(t.page.replaced, ['/game/']);
  assert.match(t.confirm.prompts.at(-1), /Open this city/);
  same(t.messages.filter((m) => m.type === 'openCity')[0].bytes, city);
});

test('the developer command log downloads as text, or explains why there is none', async () => {
  const t = await tools();
  t.elements['command-log'].onclick();
  assert.deepEqual({ ...t.messages.at(-1) }, { type: 'commandLog' });
  await t.reply({ type: 'commandLog', text: 'sim-game command log 1\n' });
  assert.match(t.page.downloads.at(-1).name, /\.cmdlog$/);
  await t.reply({ type: 'commandLog', text: null });
  assert.match(t.elements['share-status'].textContent, /No command log/);
});
