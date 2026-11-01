import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import test from 'node:test';
import { initSync, Sim } from '../src/pkg/sim_wasm.js';

initSync({ module: readFileSync(new URL('../src/pkg/sim_wasm_bg.wasm', import.meta.url)) });

test('population counts residents through WASM, independently of travelling cars', () => {
  const sim = new Sim(1, 3, 3);
  const initial = sim.stats();
  assert.equal(initial.population, 0);
  assert.equal(initial.agents, 0);
  initial.free();
  sim.fill_block(50, 50, 1);
  sim.tick(1200);
  const residents = sim.stats();
  assert.ok(residents.population > 0);
  assert.equal(residents.driving, 0);
  residents.free();
  sim.free();
});
