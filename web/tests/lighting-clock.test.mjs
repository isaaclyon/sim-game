import assert from 'node:assert/strict';
import test from 'node:test';
import { tools } from './main-harness.mjs';

test('lighting receives sim snapshot time and never advances from animation frames', async () => {
  const t = await tools();
  const snapshot = (time) => ({
    type: 'frame',
    agents: new Float32Array(),
    signals: new Float32Array(),
    stats: { secondOfDay: time },
    tickMs: 0,
  });
  t.reply(snapshot(64800));
  assert.deepEqual(t.lightingTimes, [64800]);
  t.key(' ');
  t.clock.now += 10000;
  // Supply the HUD fields before exercising animation frames.
  t.reply({
    ...snapshot(64800),
    stats: {
      secondOfDay: 64800,
      population: 0,
      unemployed: 0,
      unemploymentPercent: 0,
      driving: 0,
      walking: 0,
      parked: 0,
      tripsCompleted: 0,
      despawnedStuck: 0,
      pathsFailed: 0,
      despawnedByEdit: 0,
      demand: [0, 0, 0],
    },
  });
  t.frame();
  t.frame();
  assert.deepEqual(t.lightingTimes, [64800, 64800]);
  t.reply(snapshot(0));
  assert.equal(t.lightingTimes.at(-1), 0, 'midnight is taken directly from the sim');
});
