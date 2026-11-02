import assert from 'node:assert/strict';
import test from 'node:test';
import { buildRoadMarkings } from '../src/road-markings.ts';

const STREET = 0;
const MAIN = 1;
const ALLEY = 2;

/** Paint quads as `{ x0, x1, z0, z1 }` bounds, for roads along +x. */
function quads(roads) {
  const p = buildRoadMarkings(new Float32Array(roads.flat())).getAttribute('position');
  const out = [];
  for (let i = 0; i < (p?.count ?? 0); i += 6) {
    const xs = [];
    const zs = [];
    for (let j = 0; j < 6; j++) {
      assert.equal(p.getY(i + j), 0);
      xs.push(p.getX(i + j));
      zs.push(p.getZ(i + j));
    }
    out.push({
      x0: Math.min(...xs),
      x1: Math.max(...xs),
      z0: Math.min(...zs),
      z1: Math.max(...zs),
    });
  }
  return out;
}
const lanesAt = (q) =>
  [...new Set(q.map((m) => Math.round(((m.z0 + m.z1) / 2) * 10) / 10))].sort((a, b) => a - b);

test('main roads get a solid double centre line and edge lines at the shoulder', () => {
  const q = quads([[0, 0, 100, 0, MAIN]]);
  assert.deepEqual(lanesAt(q), [-4.1, -0.2, 0.2, 4.1]);
  for (const m of q) assert.ok(m.x0 === 0 && m.x1 === 100, 'solid the whole way');
});

test('streets get one dashed centre line and alleys get no paint', () => {
  const q = quads([[0, 0, 100, 0, STREET]]);
  assert.deepEqual(lanesAt(q), [0]);
  assert.ok(q.length > 8 && q.length < 14, `${q.length} dashes`);
  const painted = q.reduce((s, m) => s + m.x1 - m.x0, 0);
  assert.ok(painted > 25 && painted < 45, 'about a third is paint');
  assert.equal(quads([[0, 0, 100, 0, ALLEY]]).length, 0);
});

test('dashes keep their rhythm across the samples of one curve', () => {
  const whole = quads([[0, 0, 100, 0, STREET]]);
  const split = quads([
    [0, 0, 40, 0, STREET],
    [40, 0, 100, 0, STREET],
  ]);
  const painted = (q) => q.reduce((s, m) => s + m.x1 - m.x0, 0);
  assert.ok(Math.abs(painted(whole) - painted(split)) < 1e-4);
});

test('paint stops short of junctions where three or more roads meet', () => {
  const q = quads([
    [-100, 0, 0, 0, MAIN],
    [0, 0, 100, 0, MAIN],
    [0, 0, 0, 100, STREET],
  ]);
  for (const m of q) {
    const touches = m.x0 < 6 && m.x1 > -6 && m.z0 < 6 && m.z1 > -6;
    assert.ok(!touches, `paint inside the junction: ${JSON.stringify(m)}`);
  }
  // Two roads meeting end to end are one road continuing, so the paint carries on.
  const bend = quads([
    [-100, 0, 0, 0, MAIN],
    [0, 0, 100, 0, MAIN],
  ]);
  assert.ok(bend.some((m) => m.x1 === 0) && bend.some((m) => m.x0 === 0));
});
