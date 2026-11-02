import assert from 'node:assert/strict';
import test from 'node:test';
import { AgentKind, AGENT_STRIDE as STRIDE } from '../src/protocol.ts';
import { worker } from './worker-harness.mjs';

const frame = (w) => w.messages.findLast((msg) => msg.type === 'frame');
const replies = (w) => w.messages.filter((msg) => msg.type === 'citizen');
/** Index of an agent of `kind` drawn in the latest frame, which is its agent id. */
function visible(w, kind) {
  const { agents } = frame(w);
  for (let i = 0; i < agents.length / STRIDE; i++)
    if (agents[i * STRIDE + 3] >= 0 && agents[i * STRIDE + 4] === kind) return i;
  return -1;
}
const visibleCar = (w) => visible(w, AgentKind.Car);

/** Homes, shops and industry in every block, run until a resident's car is on the road. */
async function town() {
  const w = await worker(0, 5);
  w.ready();
  await w.init;
  const blocks = new Map();
  for (let x = 10; x < 500; x += 40)
    for (let y = 10; y < 500; y += 40) {
      await w.send({ type: 'previewBlock', x, y, revision: 0 });
      const { outline } = w.messages.at(-1);
      if (outline.length) blocks.set(`${outline[0]},${outline[1]}`, [x, y]);
    }
  let i = 0;
  for (const [x, y] of blocks.values())
    await w.send({ type: 'fillBlock', x, y, zone: [1, 3, 5][i++ % 3] });
  for (let i = 0; i < 20000 && visibleCar(w) < 0; i++) w.advance(101);
  assert.ok(visibleCar(w) >= 0, 'a resident drives');
  return w;
}

test('selecting a car answers at once with its resident, and keeps answering while selected', async () => {
  const w = await town();
  const agent = visibleCar(w);
  await w.send({ type: 'inspect', agent, selection: 7 });
  const first = w.messages.at(-1);
  assert.equal(first.type, 'citizen');
  assert.equal(first.selection, 7);
  const c = first.citizen;
  assert.equal(c.agent, agent, 'the car belongs to the resident shown');
  assert.equal(c.resident, true);
  assert.equal(c.driving, true);
  assert.match(c.name, /^\S+ \S+$/);
  assert.ok(c.home && c.home.zone === 1, 'lives in a house');
  assert.ok(c.destination, 'driving somewhere');
  assert.ok([0, 1, 2].includes(c.activity));
  const before = replies(w).length;
  w.advance(101);
  assert.equal(w.messages.at(-1).type, 'citizen', 'details follow each frame');
  assert.equal(replies(w).length, before + 1);
  assert.equal(w.messages.at(-1).citizen.id, c.id, 'the same person, not whoever has the slot');
  assert.equal(w.messages.at(-1).citizen.name, c.name);
  await w.send({ type: 'inspect', agent: null, selection: 8 });
  const count = replies(w).length;
  for (let i = 0; i < 50; i++) w.advance(101);
  assert.equal(replies(w).length, count, 'nothing is streamed without a selection');
});

test('selecting a walker shows a resident on foot (ADR 0026)', async () => {
  const w = await town();
  for (let i = 0; i < 20000 && visible(w, AgentKind.Walker) < 0; i++) w.advance(101);
  const agent = visible(w, AgentKind.Walker);
  assert.ok(agent >= 0, 'a resident walks');
  await w.send({ type: 'inspect', agent, selection: 4 });
  const c = w.messages.at(-1).citizen;
  assert.equal(c.agent, agent);
  assert.equal(c.onFoot, true);
  assert.equal(c.driving, false, 'walkers are not cars');
  assert.ok(c.destination, 'walking somewhere');
  const { stats } = frame(w);
  assert.ok(stats.walking > 0);
});

test('a paused sim still answers selections and edits', async () => {
  const w = await town();
  await w.send({ type: 'setSpeed', speed: 0 });
  const agent = visibleCar(w);
  await w.send({ type: 'inspect', agent, selection: 1 });
  assert.equal(w.messages.at(-1).citizen.agent, agent);
  const roads = w.messages.findLast((msg) => msg.type === 'roads').segments;
  await w.send({
    type: 'deleteRoad',
    x: (roads[0] + roads[2]) / 2,
    y: (roads[1] + roads[3]) / 2,
  });
  assert.equal(w.messages.at(-2).type, 'frame');
  assert.equal(w.messages.at(-1).type, 'citizen', 'details refresh after a paused edit');
  assert.equal(w.messages.at(-1).selection, 1);
});

test('a newer selection replaces the old one; unknown and synthetic cars select nobody', async () => {
  const w = await worker(100);
  w.ready();
  await w.init;
  for (let i = 0; i < 300 && visibleCar(w) < 0; i++) w.advance(101);
  await w.send({ type: 'inspect', agent: visibleCar(w), selection: 2 });
  assert.equal(w.messages.at(-1).selection, 2);
  assert.equal(w.messages.at(-1).citizen, null, 'benchmark cars have no resident');
  await w.send({ type: 'inspect', agent: 1e6, selection: 3 });
  assert.equal(w.messages.at(-1).selection, 3);
  assert.equal(w.messages.at(-1).citizen, null, 'unknown car');
  w.advance(101);
  assert.ok(replies(w).every((msg) => msg.selection <= 3));
});
