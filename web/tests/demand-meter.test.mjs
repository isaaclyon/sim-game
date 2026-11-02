import assert from 'node:assert/strict';
import test from 'node:test';

import { tools } from './main-harness.mjs';

test('the HUD meter raises bars for demand and drops them for a surplus', async () => {
  const t = await tools();
  t.reply({
    type: 'frame',
    agents: new Float32Array(),
    signals: new Float32Array(),
    tickMs: 0,
    stats: {
      day: 1,
      secondOfDay: 0,
      population: 0,
      unemployed: 0,
      unemploymentPercent: 0,
      demand: { residential: 60, commercial: 0, industrial: -60 },
      driving: 0,
      parked: 0,
      tripsCompleted: 0,
      despawnedStuck: 0,
      pathsFailed: 0,
      despawnedByEdit: 0,
    },
  });
  t.frame();
  const style = (use) => t.elements[`demand-${use}`].style;
  assert.deepEqual(style('residential'), { height: '25.0%', bottom: '50%' });
  assert.deepEqual(style('commercial'), { height: '0.0%', bottom: '50%' });
  assert.deepEqual(style('industrial'), { height: '25.0%', bottom: '25.0%' });
  assert.equal(
    t.elements.demand.attributes['aria-label'],
    'demand: residential +60, commercial 0, industrial -60',
  );
});
