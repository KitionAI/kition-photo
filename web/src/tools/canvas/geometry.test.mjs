// The canvas gestures' arithmetic (plan 2026-11-24, stage 2): the brush keys, the number keys, Shift's
// constraints, the nudge and wheel steps, and the Free Transform and Crop boxes. Run with
// `pnpm --dir web test:parity` (Node loads the TypeScript module directly).
import test from 'node:test'
import assert from 'node:assert/strict'
import {
  BRUSH_SIZES,
  DIGIT_PAIR_MS,
  brushSizeStep,
  centredBox,
  constrainEight,
  cropParams,
  cropQuad,
  digitPercent,
  dragCrop,
  dragTransform,
  handleAt,
  hardnessStep,
  insideOutline,
  lockAxis,
  nudgeStep,
  quadFromFields,
  rectQuad,
  sameQuad,
  snap45,
  transformFields,
  wheelZoomFactor,
} from './geometry.ts'

const near = (a, b, tolerance = 1e-6) => Math.abs(a - b) <= tolerance
const nearQuad = (quad, points, tolerance = 1e-6) => quad.every((p, index) => near(p.x, points[index][0], tolerance) && near(p.y, points[index][1], tolerance))

test('[ and ] step the brush size through the table, from any size, within 1 to 5000', () => {
  assert.equal(brushSizeStep(1, 1), 2)
  assert.equal(brushSizeStep(10, 1), 15)
  assert.equal(brushSizeStep(50, 1), 60)
  assert.equal(brushSizeStep(100, 1), 125)
  assert.equal(brushSizeStep(300, 1), 400)
  assert.equal(brushSizeStep(4900, 1), 5000)
  assert.equal(brushSizeStep(5000, 1), 5000)
  assert.equal(brushSizeStep(1, -1), 1)
  assert.equal(brushSizeStep(15, -1), 10)
  assert.equal(brushSizeStep(400, -1), 300)
  // A size between two steps snaps in the key's direction.
  assert.equal(brushSizeStep(22, 1), 25)
  assert.equal(brushSizeStep(22, -1), 20)
  assert.equal(BRUSH_SIZES[0], 1)
  assert.equal(BRUSH_SIZES.at(-1), 5000)
  assert.deepEqual([...BRUSH_SIZES].sort((a, b) => a - b), [...BRUSH_SIZES])
})

test('Shift+[ and Shift+] step the hardness by quarters within 0 to 100', () => {
  assert.equal(hardnessStep(100, -1), 75)
  assert.equal(hardnessStep(100, 1), 100)
  assert.equal(hardnessStep(0, -1), 0)
  assert.equal(hardnessStep(40, 1), 75)
  assert.equal(hardnessStep(40, -1), 25)
})

test('number keys: 1 is 10 %, 0 is 100 %, two quick keys an exact value, a slow second key starts again', () => {
  let typed = digitPercent(null, 1, 0)
  assert.equal(typed.percent, 10)
  assert.equal(digitPercent(null, 0, 0).percent, 100)
  typed = digitPercent(digitPercent(null, 4, 1000).entry, 5, 1100)
  assert.equal(typed.percent, 45)
  assert.equal(typed.entry, null)
  assert.equal(digitPercent(digitPercent(null, 0, 0).entry, 0, 50).percent, 100)
  assert.equal(digitPercent(digitPercent(null, 4, 0).entry, 5, DIGIT_PAIR_MS + 1).percent, 50)
})

test('a Shift brush stroke waits 3 screen pixels, then keeps to the axis it ran further along', () => {
  const origin = { x: 10, y: 10 }
  assert.deepEqual(lockAxis(origin, { x: 11, y: 11 }, 1, null), { point: { x: 10, y: 10 }, locked: null })
  const horizontal = lockAxis(origin, { x: 30, y: 14 }, 1, null)
  assert.deepEqual(horizontal, { point: { x: 30, y: 10 }, locked: 'h' })
  // Once locked, the axis stays even where the pointer runs further the other way.
  assert.deepEqual(lockAxis(origin, { x: 12, y: 80 }, 1, 'h'), { point: { x: 12, y: 10 }, locked: 'h' })
  assert.equal(lockAxis(origin, { x: 11, y: 40 }, 1, null).locked, 'v')
})

test('a Shift move keeps to eight directions; Shift on the Polygonal Lasso to 45°', () => {
  assert.deepEqual(constrainEight(100, 12), { x: 100, y: 0 })
  assert.deepEqual(constrainEight(-5, 90), { x: 0, y: 90 })
  assert.deepEqual(constrainEight(50, 48), { x: 49, y: 49 })
  assert.deepEqual(constrainEight(0, 0), { x: 0, y: 0 })
  const snapped = snap45({ x: 0, y: 0 }, { x: 10, y: 1 })
  assert.ok(near(snapped.y, 0) && near(snapped.x, Math.hypot(10, 1)))
})

test('an arrow nudges one screen pixel (at least one document pixel); Shift nudges 10', () => {
  assert.equal(nudgeStep(1, false), 1)
  assert.equal(nudgeStep(4, false), 1)
  assert.equal(nudgeStep(0.5, false), 2)
  assert.equal(nudgeStep(0.25, false), 4)
  assert.equal(nudgeStep(0.25, true), 10)
})

test('a mouse notch zooms at most 1.2 times; a trackpad pinch as before', () => {
  assert.ok(near(wheelZoomFactor(-100), 1.2))
  assert.ok(near(wheelZoomFactor(100), 1 / 1.2))
  assert.ok(near(wheelZoomFactor(-3, 1), 1.2))
  assert.ok(near(wheelZoomFactor(-4), Math.exp(0.04)))
})

test('a marquee from its centre spans the drag both ways', () => {
  assert.deepEqual(centredBox({ x: 50, y: 50 }, { x: 60, y: 55 }, false), { x: 40, y: 45, width: 20, height: 10 })
  assert.deepEqual(centredBox({ x: 50, y: 50 }, { x: 60, y: 55 }, true), { x: 40, y: 40, width: 20, height: 20 })
})

test('inside a selection outline, by its segments', () => {
  // A 10 x 10 square at (5, 5), as pixel edges.
  const square = [5, 5, 15, 5, 15, 5, 15, 15, 15, 15, 5, 15, 5, 15, 5, 5]
  assert.equal(insideOutline(square, 10, 10), true)
  assert.equal(insideOutline(square, 2, 10), false)
  assert.equal(insideOutline(square, 10, 20), false)
})

test('transform handles: eight, the reference point, inside, and outside to rotate', () => {
  const screen = rectQuad(100, 100, 300, 200)
  assert.equal(handleAt(screen, { x: 200, y: 150 }, { x: 101, y: 99 }), 'nw')
  assert.equal(handleAt(screen, { x: 200, y: 150 }, { x: 200, y: 203 }), 's')
  assert.equal(handleAt(screen, { x: 200, y: 150 }, { x: 302, y: 150 }), 'e')
  assert.equal(handleAt(screen, { x: 200, y: 150 }, { x: 201, y: 151 }), 'centre')
  assert.equal(handleAt(screen, { x: 200, y: 150 }, { x: 150, y: 130 }), 'inside')
  assert.equal(handleAt(screen, { x: 200, y: 150 }, { x: 350, y: 250 }), 'rotate')
  assert.equal(handleAt(screen, null, { x: 350, y: 250 }, false), null)
})

test('transform drags: scale, Shift proportional, Alt about the centre, rotate (Shift 15°), move, distort and skew', () => {
  const box = rectQuad(0, 0, 100, 50)
  const centre = { x: 50, y: 25 }
  const none = { shift: false, alt: false, meta: false }
  // The bottom-right corner 20 right and 10 down: the box grows that way, the top-left stays.
  assert.ok(nearQuad(dragTransform(box, centre, 'se', { x: 100, y: 50 }, { x: 120, y: 60 }, none), [[0, 0], [120, 0], [120, 60], [0, 60]]))
  // The right edge moves only the width.
  assert.ok(nearQuad(dragTransform(box, centre, 'e', { x: 100, y: 25 }, { x: 150, y: 90 }, none), [[0, 0], [150, 0], [150, 50], [0, 50]]))
  // Shift keeps the proportions (the larger scale wins).
  assert.ok(nearQuad(dragTransform(box, centre, 'se', { x: 100, y: 50 }, { x: 200, y: 60 }, { ...none, shift: true }), [[0, 0], [200, 0], [200, 100], [0, 100]]))
  // Alt scales about the reference point.
  assert.ok(nearQuad(dragTransform(box, centre, 'e', { x: 100, y: 25 }, { x: 110, y: 25 }, { ...none, alt: true }), [[-10, 0], [110, 0], [110, 50], [-10, 50]]))
  // Outside turns about the reference point; Shift holds it to 15° steps.
  const turned = dragTransform(box, centre, 'rotate', { x: 150, y: 25 }, { x: 50, y: 125 }, none)
  assert.ok(near(Math.atan2(turned[1].y - turned[0].y, turned[1].x - turned[0].x), Math.PI / 2))
  const snapped = dragTransform(box, centre, 'rotate', { x: 150, y: 25 }, { x: 150, y: 37 }, { ...none, shift: true })
  const degrees = (Math.atan2(snapped[1].y - snapped[0].y, snapped[1].x - snapped[0].x) * 180) / Math.PI
  assert.ok(near(degrees % 15, 0, 1e-6) || near(Math.abs(degrees % 15), 15, 1e-6), `${degrees}`)
  // Inside moves; Shift keeps eight directions.
  assert.ok(nearQuad(dragTransform(box, centre, 'inside', { x: 10, y: 10 }, { x: 40, y: 13 }, { ...none, shift: true }), [[30, 0], [130, 0], [130, 50], [30, 50]]))
  // Cmd on a corner moves it alone; on an edge it slides the edge along itself.
  assert.ok(nearQuad(dragTransform(box, centre, 'ne', { x: 100, y: 0 }, { x: 110, y: -20 }, { ...none, meta: true }), [[0, 0], [110, -20], [100, 50], [0, 50]]))
  assert.ok(nearQuad(dragTransform(box, centre, 'n', { x: 50, y: 0 }, { x: 70, y: 30 }, { ...none, meta: true }), [[20, 0], [120, 0], [100, 50], [0, 50]]))
})

test("the options bar's numbers describe the box, and give it back", () => {
  const rect = [0, 0, 100, 50]
  const box = quadFromFields(rect, { x: 60, y: 40, w: 200, h: 50, angle: 30 })
  const fields = transformFields(box, rect, { x: 60, y: 40 })
  assert.ok(near(fields.w, 200) && near(fields.h, 50) && near(fields.angle, 30) && fields.x === 60 && fields.y === 40, JSON.stringify(fields))
  assert.ok(sameQuad(quadFromFields(rect, { x: 50, y: 25, w: 100, h: 100, angle: 0 }), rectQuad(0, 0, 100, 50)))
})

test('the crop box: edges and corners in its turned axes, a held ratio, move, and Shift turns by 15°', () => {
  const box = { x: 0, y: 0, width: 200, height: 100, angle: 0 }
  assert.deepEqual(dragCrop(box, 'e', { x: 200, y: 50 }, { x: 150, y: 70 }, { shift: false }, null), { x: 0, y: 0, width: 150, height: 100, angle: 0 })
  assert.deepEqual(dragCrop(box, 'nw', { x: 0, y: 0 }, { x: 20, y: 10 }, { shift: false }, null), { x: 20, y: 10, width: 180, height: 90, angle: 0 })
  const square = dragCrop(box, 'se', { x: 200, y: 100 }, { x: 180, y: 100 }, { shift: false }, [1, 1])
  assert.ok(near(square.width, square.height) && square.x === 0 && square.y === 0, JSON.stringify(square))
  const kept = dragCrop(box, 'se', { x: 200, y: 100 }, { x: 100, y: 100 }, { shift: true }, null)
  assert.ok(near(kept.width / kept.height, 2), JSON.stringify(kept))
  assert.deepEqual(dragCrop(box, 'inside', { x: 50, y: 50 }, { x: 60, y: 45 }, { shift: false }, null), { x: 10, y: -5, width: 200, height: 100, angle: 0 })
  const turned = dragCrop(box, 'rotate', { x: 250, y: 50 }, { x: 250, y: 60 }, { shift: true }, null)
  assert.equal(turned.angle % 15, 0)
  // A turned box keeps its centre when an opposite pair of edges moves symmetrically… and its corners turn with it.
  const quad = cropQuad({ x: 0, y: 0, width: 100, height: 100, angle: 90 })
  assert.ok(nearQuad(quad, [[100, 0], [100, 100], [0, 100], [0, 0]]))
  // A side never shrinks below a pixel.
  assert.ok(dragCrop(box, 'e', { x: 200, y: 50 }, { x: -100, y: 50 }, { shift: false }, null).width >= 1)
  assert.deepEqual(cropParams({ x: 10.2, y: 5.2, width: 99.6, height: 50.2, angle: 0 }), { x: 10, y: 5, width: 100, height: 50 })
  assert.equal(cropParams({ x: 0, y: 0, width: 10, height: 10, angle: 12.345 }).angle, 12.35)
})
