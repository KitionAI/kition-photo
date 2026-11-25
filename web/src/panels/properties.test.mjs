// The Properties panel's arithmetic, against upstream's own cases (`doc_props_ui.rs`,
// `props_layout.rs` tests). Run with `pnpm --dir web test:parity`.
import test from 'node:test'
import assert from 'node:assert/strict'
import { QUICK_ACTIONS, boundsEdit, canvasResizeParams, kindLabel } from './properties.ts'

test('canvas size keeps the aspect ratio when linked, and changes nothing below half a pixel', () => {
  const size = { width: 2400, height: 1500 }
  assert.deepEqual(canvasResizeParams(size, 1200, null, true), { width: 1200, height: 750, anchor: 'center' })
  assert.deepEqual(canvasResizeParams(size, null, 3000, true), { width: 4800, height: 3000, anchor: 'center' })
  assert.deepEqual(canvasResizeParams(size, 1000, null, false), { width: 1000, height: 1500, anchor: 'center' })
  assert.equal(canvasResizeParams(size, 2400.2, null, false), null)
})

test("the header names an adjustment by its kind, an artboard as such, and other layers as '<Kind> Layer'", () => {
  assert.equal(kindLabel('Adjustment', 'Levels', false), 'Levels')
  assert.equal(kindLabel('Group', null, true), 'Artboard')
  assert.equal(kindLabel('Object', null, false), 'Object Layer')
  assert.deepEqual(Object.keys(QUICK_ACTIONS).sort(), ['Fill', 'Group', 'Object', 'Pixel', 'Shape', 'Type'])
})

test("typed X, Y, W and H run the Move tool's and Free Transform's commands (plan 2026-11-24, stage 5)", () => {
  const box = [100, 50, 300, 250]
  assert.deepEqual(boundsEdit(7, box, 'x', 140), { command: 'layer.translate', params: { layer: 7, dx: 40, dy: 0 } })
  assert.deepEqual(boundsEdit(7, box, 'y', 20.4), { command: 'layer.translate', params: { layer: 7, dx: 0, dy: -30 } })
  assert.deepEqual(boundsEdit(7, box, 'w', 100), {
    command: 'edit.transform',
    params: { layer: 7, rect: [100, 50, 300, 250], quad: [[100, 50], [200, 50], [200, 250], [100, 250]], interpolation: 'bicubic' },
  })
  assert.deepEqual(boundsEdit(7, box, 'h', 400).params.quad, [[100, 50], [300, 50], [300, 450], [100, 450]])
  // The same value, no size, or not a number: nothing to run.
  assert.equal(boundsEdit(7, box, 'x', 100), null)
  assert.equal(boundsEdit(7, box, 'w', 200), null)
  assert.equal(boundsEdit(7, box, 'w', 0), null)
  assert.equal(boundsEdit(7, box, 'h', Number.NaN), null)
})
