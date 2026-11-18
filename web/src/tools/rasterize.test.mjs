// The "Rasterize?" prompt: which tools ask, on which layers, and that a press goes to the prompt
// (once) instead of the tool, as upstream's `rasterize_prompt` tests.
import test from 'node:test'
import assert from 'node:assert/strict'
import { RASTERIZE_KINDS, paintsPixels, rasterizeNeeded, rasterizeThenPaint, withRasterizePrompt } from './rasterize.ts'

const NO_LOCKS = { transparency: false, pixels: false, position: false, artboard: false, all: false }
const layer = (id, kind, extra = {}) => ({ id, kind, depth: 0, locks: NO_LOCKS, ...extra })
const info = (layers, activeLayer, channels = {}) => ({ layers, activeLayer, channels: { target: { kind: 'composite' }, quickMask: null, ...channels } })

test('only the tools that paint pixels ask: brush-like tools, the Paint Bucket, the Magic Eraser, and the classic Gradient', () => {
  for (const tool of ['Brush', 'Pencil', 'Eraser', 'CloneStamp', 'Smudge', 'Sponge', 'HistoryBrush', 'PaintBucket', 'MagicEraser']) assert.equal(paintsPixels(tool, false), true, tool)
  for (const tool of ['Move', 'RectMarquee', 'Eyedropper', 'Hand', 'Type', 'Patch', 'Gradient', 'Pen']) assert.equal(paintsPixels(tool, false), false, tool)
  assert.equal(paintsPixels('Gradient', true), true)
})

test('asks for a type, shape, object or fill layer whose pixels the paint reaches', () => {
  for (const [kind, expected] of Object.entries(RASTERIZE_KINDS)) {
    assert.deepEqual(rasterizeNeeded(info([layer(7, kind)], 7), false), { ...expected, layer: 7 }, kind)
  }
  for (const kind of ['Pixel', 'Adjustment', 'Group']) assert.equal(rasterizeNeeded(info([layer(7, kind)], 7), false), null, kind)
  assert.equal(RASTERIZE_KINDS.Type.command, 'layer.rasterize.type')
  assert.equal(RASTERIZE_KINDS.Fill.command, 'layer.rasterize.fillContent')
})

test('does not ask when the paint goes to a mask, an alpha channel or the Quick Mask, or the pixels are locked', () => {
  const type = [layer(7, 'Type', { hasMask: true })]
  assert.equal(rasterizeNeeded(info(type, 7), true), null, 'the mask target')
  assert.equal(rasterizeNeeded(info(type, 7, { target: { kind: 'alpha', index: 0 } }), false), null, 'an alpha channel')
  assert.equal(rasterizeNeeded(info(type, 7, { quickMask: { visible: true } }), false), null, 'Quick Mask')
  assert.equal(rasterizeNeeded(info([layer(7, 'Type', { locks: { ...NO_LOCKS, pixels: true } })], 7), false), null, 'locked pixels')
  assert.equal(rasterizeNeeded(info([layer(7, 'Type', { locks: { ...NO_LOCKS, all: true } })], 7), false), null, 'locked all')
  // A group's lock reaches the layers in it, not the layers beside it.
  const grouped = [layer(1, 'Group', { locks: { ...NO_LOCKS, all: true } }), layer(7, 'Type', { depth: 1 }), layer(8, 'Shape')]
  assert.equal(rasterizeNeeded(info(grouped, 7), false), null, 'locked through its group')
  assert.equal(rasterizeNeeded(info(grouped, 8), false)?.layer, 8)
  assert.equal(rasterizeNeeded(null, false), null)
  assert.equal(rasterizeNeeded(info(type, null), false), null)
})

test('a press goes to the prompt instead of the tool, except Alt-click setting a clone source', () => {
  const begun = []
  const tool = { id: 'CloneStamp', cursor: 'crosshair', begin: (pointer) => void begun.push(pointer) }
  const opened = []
  const needed = { ...RASTERIZE_KINDS.Type, layer: 7 }
  const wrapped = withRasterizePrompt(tool, 'CloneStamp', () => needed, (ask) => opened.push(ask))
  const press = { x: 3, y: 4, pressure: 1, alt: false }
  wrapped.begin(press, {})
  assert.equal(begun.length, 0)
  assert.equal(opened.length, 1)
  assert.equal(opened[0].layer, 7)
  assert.equal(opened[0].pointer, press)
  wrapped.begin({ ...press, alt: true }, {})
  assert.equal(begun.length, 1, 'Alt-click sets the source')
  const plain = withRasterizePrompt(tool, 'CloneStamp', () => null, () => assert.fail('no prompt'))
  plain.begin(press, {})
  assert.equal(begun.length, 2)
})

test('OK rasterizes, then the same tool paints at the press; a failed rasterize paints nothing', async () => {
  const calls = []
  const tool = { id: 'Brush', cursor: 'crosshair', begin: (pointer) => ({ end: (at) => calls.push(['paint', pointer.x, at.x]) }) }
  const ask = { ...RASTERIZE_KINDS.Shape, layer: 4, tool, pointer: { x: 9, y: 2 }, context: {} }
  await rasterizeThenPaint(ask, async (command, params) => {
    calls.push([command, params.layer])
    return true
  })
  assert.deepEqual(calls, [['layer.rasterize.shape', 4], ['paint', 9, 9]])
  calls.length = 0
  await rasterizeThenPaint(ask, async () => false)
  assert.deepEqual(calls, [])
})
