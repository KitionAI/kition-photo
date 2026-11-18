// The options bars' state: upstream's defaults and sharing, the mapping to command parameters
// per tool family, and the merge of stored options. Run with `pnpm --dir web test:parity` (Node
// loads the TypeScript modules directly).
import test from 'node:test'
import assert from 'node:assert/strict'
import { PHOTO_TOOLS } from './catalog.ts'
import {
  DEFAULT_OPTIONS,
  RANGES,
  SMOOTHING_TOOLS,
  TOOL_FIELDS,
  cropRatioKey,
  cropRatioSize,
  defaultOptions,
  fasteningSpacing,
  marqueeEnd,
  mergeOptions,
  optionsFor,
  pixelArea,
  selectionModeFor,
  shapeStyle,
  toolParams,
  withOptions,
} from './options.ts'

const defaults = { ...DEFAULT_OPTIONS }
const withPatch = (patch) => ({ ...DEFAULT_OPTIONS, ...patch })

test('every tool lists the options its bar draws; every option is some tool’s', () => {
  assert.deepEqual(Object.keys(TOOL_FIELDS).sort(), PHOTO_TOOLS.map((tool) => tool.id).sort())
  const listed = new Set(Object.values(TOOL_FIELDS).flat())
  for (const field of listed) assert.ok(field in DEFAULT_OPTIONS, field)
  assert.deepEqual(
    Object.keys(DEFAULT_OPTIONS).filter((key) => !listed.has(key)),
    [],
  )
})

test('every number option has a range its default lies in', () => {
  for (const [key, value] of Object.entries(DEFAULT_OPTIONS)) {
    if (typeof value !== 'number') continue
    const range = RANGES[key]
    assert.ok(range, key)
    assert.ok(value >= range[0] && value <= range[1], `${key} ${value} in ${range}`)
  }
})

test("upstream's defaults: the engine's brush and ToolOptions::default", () => {
  assert.equal(defaults.size, 20)
  assert.equal(defaults.hardness, 100)
  assert.equal(defaults.smoothing, 10)
  assert.equal(defaults.tolerance, 32)
  assert.equal(defaults.bgTolerance, 50)
  assert.equal(defaults.frequency, 57)
  assert.equal(defaults.autoSelect, true)
  assert.equal(defaults.deleteCropped, true)
  assert.equal(defaults.gradientClassic, false)
  assert.equal(defaults.textAntiAlias, 'sharp')
  assert.equal(defaults.noteColor, '#ffff82')
})

test('options are shared as upstream shares them; Smoothing is per tool for the Brush, Pencil, and Eraser', () => {
  let state = defaultOptions()
  state = withOptions(state, 'MagicWand', { tolerance: 5, selectionMode: 'add', sampleAllLayers: true })
  assert.equal(optionsFor(state, 'PaintBucket').tolerance, 5, 'tolerance is shared')
  assert.equal(optionsFor(state, 'Lasso').selectionMode, 'add', 'one selection mode')
  assert.equal(optionsFor(state, 'Blur').sampleAllLayers, true, 'Sample All Layers is shared')
  state = withOptions(state, 'Gradient', { fillOpacity: 40 })
  assert.equal(optionsFor(state, 'PaintBucket').fillOpacity, 40, 'the fills share their opacity')
  state = withOptions(state, 'Rectangle', { shapeFill: false, strokeWidth: 4 })
  assert.deepEqual([optionsFor(state, 'Pen').shapeFill, optionsFor(state, 'Pen').strokeWidth], [false, 4], 'the Pen in Shape mode takes the shape options')
  state = withOptions(state, 'Dodge', { size: 77 })
  assert.equal(optionsFor(state, 'Brush').size, 77, 'one brush')
  state = withOptions(state, 'Pencil', { smoothing: 0 })
  assert.equal(optionsFor(state, 'Pencil').smoothing, 0)
  assert.equal(optionsFor(state, 'Brush').smoothing, 10)
  assert.deepEqual([...SMOOTHING_TOOLS], ['Brush', 'Pencil', 'Eraser'])
})

test('Brush and Eraser send fractions, and the pressure switches through the brush patch', () => {
  const params = toolParams('Brush', withPatch({ size: 40, hardness: 50, opacity: 50, flow: 25, smoothing: 0, mode: 'multiply' }))
  assert.deepEqual(params, { size: 40, hardness: 0.5, opacity: 0.5, flow: 0.25, mode: 'multiply', smoothing: 0, brush: { pressureSize: true, pressureOpacity: false } })
  assert.deepEqual(toolParams('Eraser', defaults), toolParams('Brush', defaults))
})

test('the Pencil sends its opacity as a fraction, Auto Erase, and Smoothing through the brush patch', () => {
  assert.deepEqual(toolParams('Pencil', withPatch({ opacity: 40, autoErase: true, smoothing: 30 })), { size: 20, opacity: 0.4, mode: 'normal', autoErase: true, brush: { smoothing: { amount: 0.3 } } })
})

test("the Mixer Brush sends percentages and the brush's own Flow and Sample All Layers", () => {
  assert.deepEqual(toolParams('MixerBrush', withPatch({ wet: 10, load: 20, mix: 30, mixerFlow: 40, mixerSampleAll: true, hardness: 80 })), { size: 20, brush: { hardness: 0.8 }, wet: 10, load: 20, mix: 30, flow: 40, sampleAllLayers: true })
})

test('the Background Eraser sends its sampling, limits, tolerance, and protection', () => {
  assert.deepEqual(toolParams('BackgroundEraser', withPatch({ bgSampling: 'once', bgLimits: 'findEdges', bgTolerance: 20, protectForeground: true })), {
    size: 20,
    hardness: 1,
    sampling: 'once',
    limits: 'findEdges',
    tolerance: 20,
    protectForegroundColor: true,
  })
})

test('retouching tools send percentages and their own options', () => {
  assert.deepEqual(toolParams('Sponge', withPatch({ spongeMode: 'saturate', vibrance: false })), { size: 20, hardness: 100, mode: 'saturate', vibrance: false })
  assert.deepEqual(toolParams('Sharpen', withPatch({ strength: 70, protectDetail: false })), { size: 20, hardness: 100, strength: 70, sampleAllLayers: false, protectDetail: false })
  assert.equal(toolParams('Smudge', withPatch({ fingerPainting: true })).fingerPainting, true)
  assert.deepEqual(toolParams('Dodge', withPatch({ range: 'shadows', exposure: 30, protectTones: false })), { size: 20, hardness: 100, range: 'shadows', exposure: 30, protectTones: false })
  assert.deepEqual(toolParams('CloneStamp', withPatch({ aligned: false, cloneSample: 'all' })), { size: 20, hardness: 100, aligned: false, sampleLayer: 'all' })
  assert.deepEqual(toolParams('SpotHealing', withPatch({ spotType: 'proximityMatch' })), { size: 20, hardness: 100, type: 'proximityMatch', sampleAllLayers: false })
})

test('selection tools send feather, anti-alias, and the Magnetic Lasso its edge detection', () => {
  assert.deepEqual(toolParams('RectMarquee', withPatch({ feather: 4 })), { feather: 4 })
  assert.deepEqual(toolParams('EllipseMarquee', withPatch({ antiAlias: false })), { feather: 0, antiAlias: false })
  assert.deepEqual(toolParams('MagneticLasso', withPatch({ magneticWidth: 20, contrast: 40 })), { feather: 0, antiAlias: true, width: 20, contrast: 40 })
  assert.deepEqual(toolParams('MagicWand', withPatch({ tolerance: 0, contiguous: false })), { tolerance: 0, antiAlias: true, contiguous: false, sampleAllLayers: false })
})

test('fills, crop, eyedropper, notes, and type send their options', () => {
  assert.deepEqual(toolParams('PaintBucket', withPatch({ fillSource: 'pattern', fillOpacity: 60 })), { contents: 'pattern', opacity: 60, tolerance: 32, antiAlias: true, contiguous: true, sampleAllLayers: false })
  assert.deepEqual(toolParams('Gradient', withPatch({ gradientStyle: 'radial', reverse: true, gradientMode: 'screen' })), { style: 'radial', mode: 'screen', opacity: 100, reverse: true, dither: true })
  assert.deepEqual(toolParams('MagicEraser', withPatch({ eraserOpacity: 30 })).opacity, 30)
  assert.deepEqual(toolParams('Crop', withPatch({ deleteCropped: false })), { deleteCroppedPixels: false })
  assert.deepEqual(toolParams('Eyedropper', defaults), { size: 'point', sampleLayer: 'all' })
  assert.deepEqual(toolParams('Eyedropper', withPatch({ sampleSize: '5' })).size, 5)
  assert.deepEqual(toolParams('Note', withPatch({ noteAuthor: 'QA' })), { author: 'QA', color: '#ffff82' })
  // Upstream's rule: font, style, size, and alignment (and the colour) only; the rest of new type's
  // style comes from Type › Save Default Type Styles.
  assert.deepEqual(toolParams('Type', withPatch({ fontSize: 24, textAlign: 'center' })), { font: 'Inter', fontStyle: 'Regular', size: 24, align: 'center', antialias: 'sharp' })
  assert.deepEqual(toolParams('SmartMove', withPatch({ camMode: 'extend', structure: 7, colorAdapt: 3 })), { mode: 'extend', structure: 7, color: 3, sampleAllLayers: false })
  assert.deepEqual(toolParams('Hand', defaults), {})
})

test('shapes fill with the foreground and stroke with the background, as upstream', () => {
  assert.deepEqual(shapeStyle(defaults, '#000000', '#ffffff'), { fill: '#000000', stroke: null })
  assert.deepEqual(shapeStyle(withPatch({ shapeFill: false, strokeWidth: 3 }), '#000000', '#ffffff'), { fill: null, stroke: { width: 3, color: '#ffffff' } })
})

test("selection modes: modifiers win over the bar's mode", () => {
  const none = { shift: false, alt: false }
  assert.equal(selectionModeFor('Lasso', 'add', none), 'add')
  assert.equal(selectionModeFor('Lasso', 'replace', { shift: true, alt: false }), 'add')
  assert.equal(selectionModeFor('MagicWand', 'add', { shift: false, alt: true }), 'subtract')
  assert.equal(selectionModeFor('RectMarquee', 'replace', { shift: true, alt: true }), 'intersect')
  assert.equal(selectionModeFor('QuickSelection', 'replace', none), 'add')
  assert.equal(selectionModeFor('ObjectSelection', 'add', { shift: true, alt: true }), 'subtract')
})

test('marquee styles: fixed size, fixed ratio, and Shift squares', () => {
  const start = { x: 10, y: 10 }
  assert.deepEqual(marqueeEnd('fixedSize', 64, 32, false, start, { x: 0, y: 50 }), { x: -54, y: 42 })
  assert.deepEqual(marqueeEnd('fixedRatio', 2, 1, false, start, { x: 30, y: 50 }), { x: 90, y: 50 })
  assert.deepEqual(marqueeEnd('normal', 1, 1, true, start, { x: 30, y: 15 }), { x: 30, y: 30 })
  assert.deepEqual(marqueeEnd('normal', 1, 1, false, start, { x: 30, y: 15 }), { x: 30, y: 15 })
  assert.deepEqual(pixelArea({ x: 10.5, y: 3 }, { x: 2.2, y: 8.1 }), { x: 2, y: 3, width: 9, height: 6 })
})

test('crop ratios: presets, the original, typed numbers', () => {
  assert.deepEqual(cropRatioSize('16:9', 100, 50), [16, 9])
  assert.deepEqual(cropRatioSize('original', 100, 50), [100, 50])
  assert.equal(cropRatioSize('', 100, 50), null)
  assert.equal(cropRatioKey(4, 3), '4:3')
  assert.equal(cropRatioKey(0, 3), '')
  assert.equal(fasteningSpacing(57), 8 + 43 * 0.9)
})

test('stored options merge: unknown keys dropped, numbers clamped, invalid and missing values at their defaults', () => {
  const merged = mergeOptions({
    values: { size: 80000, opacity: 'half', hardness: Number.NaN, legacy: true, mode: 'multiply', gradientMode: 'no-such-mode', cropRatio: '3:2', tolerance: -4, noteColor: 'red', noteAuthor: 'x'.repeat(500) },
    smoothing: { Brush: 30, Pencil: 'none', Dodge: 50 },
    Brush: { size: 12 },
  })
  assert.equal(merged.values.size, 5000, 'clamped to the range')
  assert.equal(merged.values.tolerance, 0, 'clamped to the range')
  assert.equal(merged.values.mode, 'multiply')
  assert.equal(merged.values.opacity, 100)
  assert.equal(merged.values.hardness, 100)
  assert.equal(merged.values.gradientMode, 'normal')
  assert.equal(merged.values.cropRatio, '3:2')
  assert.equal(merged.values.noteColor, '#ffff82')
  assert.equal(merged.values.noteAuthor.length, 200)
  assert.equal('legacy' in merged.values, false)
  assert.deepEqual(merged.smoothing, { Brush: 30, Pencil: 10, Eraser: 10 })
  assert.deepEqual(Object.keys(merged), ['values', 'smoothing'])
  assert.deepEqual(mergeOptions('garbage'), defaultOptions())
  assert.deepEqual(mergeOptions(null), defaultOptions())
  assert.deepEqual(mergeOptions({ Brush: { size: 99 } }), defaultOptions(), "an older shape (one record per tool) starts at upstream's defaults")
  assert.equal(mergeOptions({ values: { cropRatio: '../../x' } }).values.cropRatio, '')
})

test('what is stored reads back the same', () => {
  let state = defaultOptions()
  state = withOptions(state, 'Zoom', { scrubbyZoom: false })
  state = withOptions(state, 'Eraser', { smoothing: 55, size: 33 })
  assert.deepEqual(mergeOptions(JSON.parse(JSON.stringify(state))), state)
})
