// The brush picker's and Brush Settings' arithmetic (plan 2026-11-24, stage 5): node --test (part of `pnpm test:parity`).
import test from 'node:test'
import assert from 'node:assert/strict'
import { THUMB_HEIGHT, THUMB_WIDTH, presetOptions, presetsOf, settingsOf, settingsPatch, thumbnailSize, thumbnailStroke } from './brushes.ts'

test("Brush Settings shows the engine's brush in degrees and percent", () => {
  const brush = { angle: 29.6, roundness: 0.5, spacing: 0.1, spacingEnabled: true, shapeDynamics: { enabled: true, size: { jitter: 0.4 } }, transfer: { enabled: false, opacity: { jitter: 0.3 } } }
  assert.deepEqual(settingsOf(brush), { angle: 30, roundness: 50, spacing: 10, spacingEnabled: true, shapeDynamics: true, sizeJitter: 40, transfer: false, opacityJitter: 30 })
  assert.deepEqual(settingsOf(null), { angle: 0, roundness: 100, spacing: 25, spacingEnabled: true, shapeDynamics: false, sizeJitter: 0, transfer: false, opacityJitter: 0 })
})

test('a field becomes the patch tools.setBrush takes; a jitter switches its section on', () => {
  assert.deepEqual(settingsPatch('angle', 200), { angle: 180 })
  assert.deepEqual(settingsPatch('roundness', 25), { roundness: 0.25 })
  assert.deepEqual(settingsPatch('spacing', 50), { spacing: 0.5, spacingEnabled: true })
  assert.deepEqual(settingsPatch('spacing', 0), { spacing: 0.01, spacingEnabled: true })
  assert.deepEqual(settingsPatch('spacingEnabled', false), { spacingEnabled: false })
  assert.deepEqual(settingsPatch('sizeJitter', 40), { shapeDynamics: { enabled: true, size: { jitter: 0.4 } } })
  assert.deepEqual(settingsPatch('shapeDynamics', false), { shapeDynamics: { enabled: false } })
  assert.deepEqual(settingsPatch('opacityJitter', 120), { transfer: { enabled: true, opacity: { jitter: 1 } } })
  assert.deepEqual(settingsPatch('transfer', true), { transfer: { enabled: true } })
})

test("a picked preset gives the options bar its size, hardness and pressure", () => {
  assert.deepEqual(presetOptions({ size: 45, hardness: 0, pressureSize: false, pressureOpacity: true, angle: 3 }), { size: 45, hardness: 0, pressureSize: false, pressureOpacity: true })
  assert.deepEqual(presetOptions({ size: 9000, hardness: 0.899 }), { size: 5000, hardness: 90 })
  assert.deepEqual(presetOptions(null), {})
})

test('the preset list reads what the engine lists, and the thumbnails stay in their row', () => {
  assert.deepEqual(presetsOf({ presets: [{ name: 'Hard Round', size: 30 }, { size: 3 }, null] }), [{ name: 'Hard Round', size: 30 }])
  assert.deepEqual(presetsOf(null), [])
  const stroke = thumbnailStroke(2)
  for (const [x, y, pressure] of stroke) {
    assert.ok(x >= 0 && x <= THUMB_WIDTH)
    assert.ok(y >= 2 * THUMB_HEIGHT && y <= 3 * THUMB_HEIGHT)
    assert.ok(pressure > 0 && pressure <= 1)
  }
  assert.ok(stroke[24][2] > stroke[0][2], 'full in the middle')
  assert.equal(thumbnailSize({ name: 'x', size: 300 }), 12)
  assert.equal(thumbnailSize({ name: 'x', size: 1 }), 2)
})
