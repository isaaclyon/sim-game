import assert from 'node:assert/strict';
import test from 'node:test';

import { fitLevel } from '../src/ui/fit.ts';

/** Runs fitLevel over a bar whose content needs `needs[level]` pixels of `room`. */
function fit(needs, room) {
  const tried = [];
  const level = fitLevel(needs.length - 1, (l) => {
    tried.push(l);
    return needs[l] > room;
  });
  return { level, tried };
}

test('a bar that fits stays whole', () => {
  assert.deepEqual(fit([900, 800, 700], 1000), { level: 0, tried: [0] });
});

test('a narrow bar takes the first level that fits', () => {
  assert.deepEqual(fit([900, 800, 700], 850), { level: 1, tried: [0, 1] });
});

test('a bar nothing fits is left at the last level, applied', () => {
  assert.deepEqual(fit([900, 800, 700], 100), { level: 2, tried: [0, 1, 2] });
});
