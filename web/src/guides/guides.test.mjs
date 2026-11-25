// Rulers, guides and snapping (plan 2026-11-24, stage 7): ruler ticks at every zoom, what a drag
// snaps to and the lines it shows, guide hits and positions, and View's rows and check marks.
// Run with `pnpm --dir web test:parity`.
import test from 'node:test'
import assert from 'node:assert/strict'
import { guideAt, guidePosition, rulerStep, rulerTicks, snapAxis, snapMove, snapPoint, snapSource } from './snap.ts'
import { DEFAULT_VIEW, VIEW_SETTING_ROWS, gridSpacing, shown, validView, viewChecked, viewRow } from './view.ts'

const ALL = { guides: true, grid: true, layers: true, documentBounds: true }
const NONE = { guides: false, grid: false, layers: false, documentBounds: false }

test('the ruler numbers 1, 2 or 5 times a power of ten, at least 50 screen pixels apart, with small ticks between', () => {
  assert.deepEqual(rulerStep(1), { major: 50, minor: 10 })
  assert.deepEqual(rulerStep(0.82), { major: 100, minor: 10 })
  assert.deepEqual(rulerStep(0.1), { major: 500, minor: 100 })
  assert.deepEqual(rulerStep(16), { major: 5, minor: 1 })
  assert.deepEqual(rulerStep(64), { major: 1, minor: 1 })
  for (const zoom of [0.01, 0.05, 0.3, 1, 3, 12, 32]) {
    const { major, minor } = rulerStep(zoom)
    assert.ok(major * zoom >= 50, `${zoom}`)
    assert.ok(minor >= 1 && Number.isInteger(major / minor), `${zoom}`)
  }
})

test('ticks follow the view: numbered where document coordinates are multiples of the step', () => {
  const ticks = rulerTicks(-20, 1, 200)
  assert.deepEqual(ticks.major.map((tick) => [tick.at, tick.label]), [[20, '0'], [70, '50'], [120, '100'], [170, '150']])
  assert.equal(ticks.minor.length, 17)
  // Scrolled and zoomed: a tick at document 200 sits where the view puts it.
  const zoomed = rulerTicks(150, 2, 400)
  assert.ok(zoomed.major.some((tick) => tick.label === '200' && tick.at === 100))
})

test('a guide is dropped on a whole pixel, or with Shift on the ruler’s nearest tick', () => {
  assert.equal(guidePosition(120.4, 1, false), 120)
  assert.equal(guidePosition(123, 1, true), 120)
  assert.equal(guidePosition(126, 1, true), 130)
})

test('a guide is hit within reach, the nearer of two', () => {
  const guides = { horizontal: [50], vertical: [100, 104] }
  assert.deepEqual(guideAt(guides, { x: 103.5, y: 0 }, 4), { orientation: 'vertical', index: 1 })
  assert.deepEqual(guideAt(guides, { x: 10, y: 52 }, 4), { orientation: 'horizontal', index: 0 })
  assert.equal(guideAt(guides, { x: 10, y: 60 }, 4), null)
})

test('snapping: to a guide within reach, ties to a guide before the grid, nothing out of reach or turned off', () => {
  const source = snapSource({ guides: { horizontal: [], vertical: [100] }, layers: [], width: 400, height: 300, grid: 25, to: ALL })
  assert.equal(snapPoint({ x: 96, y: 0 }, source, 8).x, 100)
  assert.equal(snapPoint({ x: 80, y: 0 }, source, 8).x, 75)
  assert.equal(snapPoint({ x: 87, y: 0 }, source, 3).x, 87)
  assert.equal(snapAxis([100], source.x, 25, 8).edge.kind, 'guide')
  const off = snapSource({ guides: { horizontal: [], vertical: [100] }, layers: [], width: 400, height: 300, grid: 25, to: NONE })
  assert.deepEqual(snapPoint({ x: 96, y: 7 }, off, 8), { x: 96, y: 7, lines: [] })
})

test("a moved box snaps by its edges or centre to other layers' and the document's, with a line across both", () => {
  const source = snapSource({ guides: { horizontal: [], vertical: [] }, layers: [{ x0: 200, y0: 50, x1: 300, y1: 150 }], width: 400, height: 300, grid: null, to: { ...NONE, layers: true, documentBounds: true } })
  // The box's left edge, 4 px short of the other layer's right edge.
  const moved = snapMove({ x0: 0, y0: 160, x1: 50, y1: 200 }, 296, 0, source, 8)
  assert.equal(moved.dx, 300)
  assert.deepEqual(moved.lines, [{ axis: 'x', at: 300, from: 50, to: 200, kind: 'layer' }])
  // Its centre to the document's centre.
  const centred = snapMove({ x0: 0, y0: 0, x1: 50, y1: 50 }, 172, 128, source, 8)
  assert.deepEqual([centred.dx, centred.dy], [175, 125])
})

test("View's rows: each toggles its setting and shows its check mark; Show items need the extras", () => {
  for (const command of VIEW_SETTING_ROWS) assert.notEqual(viewRow(DEFAULT_VIEW, command), DEFAULT_VIEW, command)
  assert.equal(viewRow(DEFAULT_VIEW, 'view.rulers').rulers, true)
  assert.equal(viewChecked(viewRow(DEFAULT_VIEW, 'view.show.grid'), 'view.show.grid'), true)
  const hidden = viewRow(DEFAULT_VIEW, 'view.extras')
  assert.equal(shown(hidden, 'guides'), false)
  assert.equal(viewChecked(hidden, 'view.show.guides'), false)
  // Showing an item while the extras are hidden shows them again.
  assert.equal(shown(viewRow(hidden, 'view.show.guides'), 'guides'), true)
  assert.deepEqual(viewRow(DEFAULT_VIEW, 'view.snapTo.all').snapTo, ALL)
  assert.equal(viewRow(DEFAULT_VIEW, 'view.lockGuides').lockGuides, true)
  assert.equal(viewChecked(DEFAULT_VIEW, 'view.clearGuides'), undefined)
  assert.equal(viewRow(DEFAULT_VIEW, 'view.clearGuides'), DEFAULT_VIEW)
})

test('stored View settings read back; unknown or wrong values fall back to the defaults', () => {
  assert.deepEqual(validView(DEFAULT_VIEW), DEFAULT_VIEW)
  assert.deepEqual(validView({ rulers: 'yes', snapTo: { layers: true } }), { ...DEFAULT_VIEW, snapTo: { ...DEFAULT_VIEW.snapTo, layers: true } })
  assert.equal(validView(null), undefined)
})

test('the grid has a line every inch of the resolution, in four', () => {
  assert.deepEqual(gridSpacing(72), { major: 72, minor: 18 })
  assert.deepEqual(gridSpacing(0), { major: 72, minor: 18 })
})
