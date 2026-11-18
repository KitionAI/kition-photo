// The mask target: when it applies, what it adds to a command, and the engine's list of the
// commands that follow it (read from the engine, so the editor's list cannot drift from it).
import { existsSync, readFileSync } from 'node:fs'
import test from 'node:test'
import assert from 'node:assert/strict'
import { disabledWithMaskTarget, maskTargetApplies, withMaskTarget } from './maskTarget.ts'

const info = (layer, channels) => ({ activeLayer: 1, layers: [{ id: 1, hasMask: true, ...layer }], channels: { target: { kind: 'composite' }, quickMask: null, ...channels } })

test('applies only to a layer with a mask, on the composite, outside Quick Mask', () => {
  assert.equal(maskTargetApplies(true, info()), true)
  assert.equal(maskTargetApplies(false, info()), false)
  assert.equal(maskTargetApplies(true, info({ hasMask: false })), false)
  assert.equal(maskTargetApplies(true, info({}, { target: { kind: 'alpha', index: 0 } })), false, 'an alpha channel keeps its target')
  assert.equal(maskTargetApplies(true, info({}, { quickMask: { visible: true } })), false, 'Quick Mask keeps its target')
  assert.equal(maskTargetApplies(true, null), false)
})

test('aims only the commands that follow the target, and never over a named target', () => {
  const follows = new Set(['paint.stroke', 'image.adjustments.invert'])
  assert.deepEqual(withMaskTarget('image.adjustments.invert', {}, follows, true), { target: 'mask' })
  assert.deepEqual(withMaskTarget('layer.new.layer', {}, follows, true), {})
  assert.deepEqual(withMaskTarget('paint.stroke', { target: 'pixels' }, follows, true), { target: 'pixels' })
  assert.deepEqual(withMaskTarget('paint.stroke', { size: 3 }, follows, false), { size: 3 })
})

test('swaps the engine\'s own states of the aimed commands for their states aimed at the mask', () => {
  assert.deepEqual(disabledWithMaskTarget(['image.adjustments.invert', 'layer.delete'], { follows: ['image.adjustments.invert', 'edit.fill'], disabled: ['edit.fill'] }).sort(), ['edit.fill', 'layer.delete'])
})

const engineDir = new URL('../../public/engine/', import.meta.url)
const built = existsSync(new URL('kition_photo_kit_bg.wasm', engineDir))

test("the engine names the commands that follow the target, pastes and filters included", { skip: built ? false : 'the web engine is not built' }, async () => {
  const sdk = await import(new URL('kition_photo_kit.js', engineDir).href)
  sdk.initSync({ module: readFileSync(new URL('kition_photo_kit_bg.wasm', engineDir)) })
  const doc = sdk.PhotoDocument.create('Untitled', 16, 16)
  const { follows } = JSON.parse(doc.maskTargetCommands())
  for (const id of ['paint.stroke', 'paint.bucket', 'edit.fill', 'edit.paste', 'edit.pasteSpecial.pasteInPlace', 'filter.blur.gaussianBlur', 'image.adjustments.invert']) assert.ok(follows.includes(id), id)
  assert.ok(!follows.includes('layer.new.layer'))
})
