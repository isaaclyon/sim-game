import assert from 'node:assert/strict';
import test from 'node:test';
import { tools } from './main-harness.mjs';
import { worker } from './worker-harness.mjs';

const frames = (w) => w.messages.filter((m) => m.type === 'frame');

test('fast speeds send at most one frame per screen frame, and the newest state follows', async () => {
  const w = await worker(200);
  w.ready();
  await w.init;
  await w.send({ type: 'setSpeed', speed: 10 });
  const before = frames(w).length;
  // One real second in 4 ms worker steps: 100 ticks at 10x.
  for (let i = 0; i < 250; i++) w.advance(4);
  const sent = frames(w).length - before;
  assert.ok(sent >= 50 && sent <= 61, `${sent} frames in one second`);
  // Ticks too soon after a frame are held back, then sent even if the city pauses meanwhile.
  w.advance(100);
  const shown = frames(w).length;
  const at = frames(w).at(-1).stats.secondOfDay;
  w.advance(15);
  assert.equal(frames(w).length, shown, 'a tick 15 ms after the last frame');
  await w.send({ type: 'setSpeed', speed: 0 });
  w.advance(4);
  assert.equal(frames(w).length, shown + 1);
  assert.ok(frames(w).at(-1).stats.secondOfDay > at, 'it shows the held-back tick');
  for (let i = 0; i < 5; i++) w.advance(4);
  assert.equal(frames(w).length, shown + 1, 'nothing more to send while paused');
});

test('the worker refills an agent buffer the page hands back instead of allocating', async () => {
  const w = await worker(200);
  w.ready();
  await w.init;
  w.advance(101);
  const first = frames(w).at(-1).agents;
  assert.ok(first.length > 0);
  await w.send({ type: 'spareAgents', agents: first });
  w.advance(101);
  const next = frames(w).at(-1).agents;
  assert.equal(next.buffer, first.buffer, 'same memory, new contents');
  assert.equal(next.length, first.length);
  // A buffer too small for the snapshot is replaced by a roomier one.
  await w.send({ type: 'spareAgents', agents: new Float32Array(5) });
  w.advance(101);
  const grown = frames(w).at(-1).agents;
  assert.equal(grown.length, first.length);
  assert.ok(grown.buffer.byteLength > grown.byteLength, 'headroom for more agents');
});

test('the page returns each frame it stops drawing to the worker', async () => {
  const t = await tools();
  const frame = () => ({
    type: 'frame',
    agents: new Float32Array(10),
    signals: new Float32Array(),
    stats: { secondOfDay: 0 },
    tickMs: 0,
  });
  const sent = [frame(), frame(), frame(), frame()];
  for (const f of sent) t.reply(f);
  const spares = t.messages.filter((m) => m.type === 'spareAgents').map((m) => m.agents);
  // The last two are still drawn: one interpolates from the other.
  assert.deepEqual(spares, [sent[0].agents, sent[1].agents]);
  assert.equal(spares[0], sent[0].agents);
});
