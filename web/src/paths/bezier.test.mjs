// Path geometry for the Pen, Direct Selection, Path Selection and Convert Point (plan 2026-11-24,
// stage 7): hit tests, a segment split, anchors deleted and moved, direction points dragged with
// Photoshop's smooth-point rule, points converted. Run with `pnpm --dir web test:parity`.
import test from 'node:test'
import assert from 'node:assert/strict'
import { anchorsIn, convertPoint, cubicAt, deleteAnchor, deleteAnchors, hitAnchor, hitHandle, hitSegment, hitSubpath, moveAnchors, moveHandle, movePath, nearestOnSegment, segmentCount, splitSegment } from './bezier.ts'
import { partAt, pathTargets, prunedSelection, replaceCommand, selectedOn, targetParams } from './store.ts'

const corner = (x, y) => ({ anchor: [x, y], in: [x, y], out: [x, y], smooth: false })
/** A straight open path (0,0) → (100,0) → (100,100), and a closed curve. */
const line = () => ({ subpaths: [{ closed: false, knots: [corner(0, 0), corner(100, 0), corner(100, 100)] }] })
const curve = () => ({
  subpaths: [
    {
      closed: true,
      knots: [
        { anchor: [0, 50], in: [0, 80], out: [0, 20], smooth: true },
        { anchor: [100, 50], in: [100, 20], out: [100, 80], smooth: true },
      ],
    },
  ],
})
const near = (a, b, tolerance = 1e-6) => Math.abs(a - b) <= tolerance

test('a cubic runs from its first to its last point', () => {
  assert.deepEqual(cubicAt([0, 0], [0, 10], [10, 10], [10, 0], 0), [0, 0])
  assert.deepEqual(cubicAt([0, 0], [0, 10], [10, 10], [10, 0], 1), [10, 0])
  assert.deepEqual(cubicAt([0, 0], [0, 10], [10, 10], [10, 0], 0.5), [5, 7.5])
})

test('an open subpath has one segment fewer than knots, a closed one as many', () => {
  assert.equal(segmentCount(line().subpaths[0]), 2)
  assert.equal(segmentCount(curve().subpaths[0]), 2)
  assert.equal(segmentCount({ closed: false, knots: [corner(0, 0)] }), 0)
})

test('the nearest point on a segment is found to a fraction of a pixel', () => {
  const found = nearestOnSegment([[0, 0], [0, 0], [100, 0], [100, 0]], [30, 7])
  assert.ok(near(found.distance, 7, 1e-3), String(found.distance))
  assert.ok(near(cubicAt([0, 0], [0, 0], [100, 0], [100, 0], found.t)[0], 30, 0.05))
})

test('hits: an anchor before a segment, only within reach, the nearer of two', () => {
  assert.deepEqual(hitAnchor(line(), [102, 3], 6), [0, 1])
  assert.equal(hitAnchor(line(), [50, 3], 6), null)
  assert.deepEqual(hitSegment(line(), [50, 3], 6), { subpath: 0, knot: 0, t: hitSegment(line(), [50, 3], 6).t })
  assert.ok(near(hitSegment(line(), [50, 3], 6).t, 0.5, 0.01))
  assert.equal(hitSegment(line(), [50, 30], 6), null)
  // The closing segment of a closed path counts.
  assert.equal(hitSegment(curve(), [50, 72], 6)?.knot, 1)
})

test('a direction point is hit only on a selected anchor, and a retracted one never', () => {
  assert.deepEqual(hitHandle(curve(), [[0, 0]], [1, 21], 6), { subpath: 0, knot: 0, handle: 'out' })
  assert.equal(hitHandle(curve(), [[0, 1]], [1, 21], 6), null)
  assert.equal(hitHandle(line(), [[0, 1]], [100, 0], 6), null)
})

test('Path Selection picks the subpath under the press, or whose bounds hold it', () => {
  assert.equal(hitSubpath(line(), [50, 2], 6), 0)
  assert.equal(hitSubpath(line(), [60, 40], 6), 0)
  assert.equal(hitSubpath(line(), [300, 300], 6), null)
})

test('splitting a segment adds an anchor on it and keeps the curve where it was', () => {
  const before = curve()
  const t = 0.3
  const at = cubicAt(...[before.subpaths[0].knots[0].anchor, before.subpaths[0].knots[0].out, before.subpaths[0].knots[1].in, before.subpaths[0].knots[1].anchor], t)
  const after = splitSegment(before, 0, 0, t)
  assert.equal(after.subpaths[0].knots.length, 3)
  assert.ok(near(after.subpaths[0].knots[1].anchor[0], at[0]) && near(after.subpaths[0].knots[1].anchor[1], at[1]))
  assert.equal(after.subpaths[0].knots[1].smooth, true)
  // The two halves meet the old curve at their own middles.
  const k = after.subpaths[0].knots
  const half = cubicAt(k[0].anchor, k[0].out, k[1].in, k[1].anchor, 0.5)
  const old = cubicAt(before.subpaths[0].knots[0].anchor, before.subpaths[0].knots[0].out, before.subpaths[0].knots[1].in, before.subpaths[0].knots[1].anchor, t / 2)
  assert.ok(near(half[0], old[0], 1e-6) && near(half[1], old[1], 1e-6))
  // A straight segment's new anchor is a corner; the original is untouched.
  const straight = splitSegment(line(), 0, 0, 0.5)
  assert.deepEqual(straight.subpaths[0].knots[1], corner(50, 0))
  assert.equal(line().subpaths[0].knots.length, 3)
})

test('deleting anchors: the rest stay in order; a closed path of one anchor opens; an empty subpath goes', () => {
  assert.deepEqual(deleteAnchor(line(), 0, 1).subpaths[0].knots.map((k) => k.anchor), [[0, 0], [100, 100]])
  const one = deleteAnchor(curve(), 0, 0)
  assert.equal(one.subpaths[0].closed, false)
  assert.deepEqual(deleteAnchors(line(), [[0, 0], [0, 1], [0, 2]]).subpaths, [])
  assert.deepEqual(deleteAnchors(line(), [[0, 2], [0, 0]]).subpaths[0].knots.map((k) => k.anchor), [[100, 0]])
})

test('moving anchors moves their direction points with them; moving the path moves everything', () => {
  const moved = moveAnchors(curve(), [[0, 1]], 10, -5)
  assert.deepEqual(moved.subpaths[0].knots[1], { anchor: [110, 45], in: [110, 15], out: [110, 75], smooth: true })
  assert.deepEqual(moved.subpaths[0].knots[0], curve().subpaths[0].knots[0])
  assert.deepEqual(movePath(line(), 1, 2).subpaths[0].knots.map((k) => k.anchor), [[1, 2], [101, 2], [101, 102]])
})

test("a smooth point's other direction point turns to stay in line, keeping its length; Alt breaks it", () => {
  const turned = moveHandle(curve(), 0, 0, 'out', [30, 50], false)
  assert.deepEqual(turned.subpaths[0].knots[0].out, [30, 50])
  assert.ok(near(turned.subpaths[0].knots[0].in[0], -30) && near(turned.subpaths[0].knots[0].in[1], 50))
  const broken = moveHandle(curve(), 0, 0, 'out', [30, 50], true)
  assert.deepEqual(broken.subpaths[0].knots[0].in, [0, 80])
  assert.equal(broken.subpaths[0].knots[0].smooth, false)
})

test('converting: no direction gives a corner with retracted points; one gives a smooth point and its mirror', () => {
  assert.deepEqual(convertPoint(curve(), 0, 0, null).subpaths[0].knots[0], corner(0, 50))
  assert.deepEqual(convertPoint(line(), 0, 1, [130, 10]).subpaths[0].knots[1], { anchor: [100, 0], in: [70, -10], out: [130, 10], smooth: true })
})

test('a marquee takes the anchors inside it, whichever way it was dragged', () => {
  assert.deepEqual(anchorsIn(line(), [110, -10, 90, 110]), [[0, 1], [0, 2]])
})

test('the store: the shape layer before the work path, names for the commands, the selection kept to anchors that exist', () => {
  const view = { work: line(), shape: { layer: 7, path: curve() }, selected: { kind: 'work', layer: null, anchors: [[0, 1], [0, 9]] }, editing: null }
  assert.deepEqual(pathTargets(view).map((t) => t.kind), ['shape', 'work'])
  assert.deepEqual(targetParams({ kind: 'work', layer: null }), { name: 'work' })
  assert.deepEqual(targetParams({ kind: 'shape', layer: 7 }), { name: 'layer', layer: 7 })
  assert.deepEqual(replaceCommand({ kind: 'shape', layer: 7 }, line())[0], 'shape.edit')
  assert.deepEqual(replaceCommand({ kind: 'work', layer: null }, line()), ['path.set', { name: 'work', path: line() }])
  assert.deepEqual(prunedSelection(view).anchors, [[0, 1]])
  assert.deepEqual(selectedOn(view, { kind: 'shape', layer: 7 }), [])
  assert.equal(partAt(view, [100, 1], 6).anchor[1], 1)
  assert.equal(partAt(view, [50, 1], 6).segment.knot, 0)
  assert.equal(partAt(view, [500, 500], 6), null)
})
