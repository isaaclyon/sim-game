import assert from 'node:assert/strict';
import test from 'node:test';
import { PRICES, tools } from './main-harness.mjs';

const roads = () => ({ type: 'roads', segments: new Float32Array(), bounds: [0, 0, 100, 100] });
const answer = (request) => ({
  type: 'blockPreview',
  revision: request.revision,
  outline: new Float32Array([0, 0, 100, 0, 100, 100, 0, 0]),
});

test('city replacement cancels a pending zone preview and allows fresh requests', async () => {
  const t = await tools();
  t.key('z');
  t.move(10, 20);
  const old = t.messages.at(-1);
  t.reply({ type: 'cityReset', speed: 0, prices: PRICES });
  t.key('z');
  t.move(30, 40);
  const fresh = t.messages.at(-1);
  assert.equal(fresh.type, 'previewBlock');
  assert.notEqual(fresh.revision, old.revision, 'does not wait for the old city to answer');
  const shown = t.previews.length;
  t.reply(answer(old));
  assert.equal(t.previews.length, shown, 'the old city outline stays hidden');
  t.reply(answer(fresh));
  assert.equal(t.previews.at(-1).type, 'block');
});

test('road changes refresh the hovered block and coalesce while a preview is pending', async () => {
  const t = await tools();
  t.key('z');
  t.move(10, 20);
  const first = t.messages.at(-1);
  t.reply(answer(first));
  t.reply(roads());
  const refresh = t.messages.at(-1);
  assert.notEqual(refresh, first);
  assert.deepEqual([refresh.type, refresh.x, refresh.y], ['previewBlock', 10, 20]);
  t.reply(roads());
  t.reply(roads());
  assert.equal(t.messages.at(-1), refresh, 'waits for the pending answer');
  t.reply(answer(refresh));
  const queued = t.messages.at(-1);
  assert.notEqual(queued, refresh);
  assert.deepEqual([queued.type, queued.x, queued.y], ['previewBlock', 10, 20]);
  t.reply(answer(queued));
  const count = t.messages.length;
  t.key('Escape');
  t.reply(roads());
  assert.equal(t.messages.length, count, 'inactive zoning does not request previews');
});
