import type { AddedToolId, PhotoToolId, UpstreamToolId } from './catalog'

/**
 * The options Photo's options bars hold, shared as upstream shares them: one brush for every
 * brush tool (the engine's brush: size, hardness, mode, opacity, flow, pressure, the Mixer Brush's
 * settings), one `ToolOptions` record for every other tool (tolerance, anti-alias, feather, the
 * selection mode…), and Smoothing kept per tool for the Brush, Pencil, and Eraser
 * (`sync_tool_smoothing`). Values are as the bar shows them: percentages (0 to 100) where upstream
 * shows percentages; `toolParams` turns them into each command's own units. Defaults are
 * upstream's (`ToolOptions::default`, the engine's brush: 20 px, hard, 10 % Smoothing).
 */
export const DEFAULT_OPTIONS = {
  // The engine's brush, shared by every brush tool.
  size: 20,
  hardness: 100,
  mode: 'normal',
  opacity: 100,
  flow: 100,
  pressureSize: true,
  pressureOpacity: false,
  wet: 50,
  load: 50,
  mix: 50,
  mixerFlow: 100,
  mixerSampleAll: false,
  /** Per tool (Brush, Pencil, Eraser); see `SMOOTHING_TOOLS`. */
  smoothing: 10,
  // Upstream's `ToolOptions` and `selection_mode`, shared by every tool that shows them.
  selectionMode: 'replace' as SelectionMode,
  tolerance: 32,
  contiguous: true,
  antiAlias: true,
  sampleAllLayers: false,
  feather: 0,
  marqueeStyle: 'normal',
  marqueeWidth: 1,
  marqueeHeight: 1,
  magneticWidth: 10,
  contrast: 10,
  frequency: 57,
  enhanceEdge: false,
  gradientStyle: 'linear',
  reverse: false,
  dither: true,
  gradientClassic: false,
  gradientMode: 'normal',
  fillOpacity: 100,
  fillSource: 'foreground',
  font: 'Inter',
  fontStyle: 'Regular',
  fontSize: 48,
  textAntiAlias: 'sharp',
  textAlign: 'left',
  aligned: true,
  cloneSample: 'current',
  spotType: 'smart',
  patchMode: 'source',
  camMode: 'move',
  structure: 4,
  colorAdapt: 0,
  range: 'midtones',
  exposure: 50,
  protectTones: true,
  spongeMode: 'desaturate',
  vibrance: true,
  strength: 50,
  protectDetail: true,
  fingerPainting: false,
  penMode: 'path',
  shapeFill: true,
  strokeWidth: 0,
  radius: 0,
  sides: 5,
  weight: 3,
  customShape: 'Heart',
  autoSelect: true,
  autoSelectTarget: 'layer',
  showTransform: false,
  cropRatio: '',
  deleteCropped: true,
  cropOverlay: true,
  eraserOpacity: 100,
  bgSampling: 'continuous',
  bgLimits: 'contiguous',
  bgTolerance: 50,
  protectForeground: false,
  scrubbyZoom: true,
  autoErase: false,
  sampleSize: 'point',
  eyedropperSample: 'all',
  // Upstream's Note tool (`AnalysisUi`): the author and colour new notes get.
  noteAuthor: '',
  noteColor: '#ffff82',
}

export type SelectionMode = 'replace' | 'add' | 'subtract' | 'intersect'
export type ToolOptions = typeof DEFAULT_OPTIONS
export type OptionKey = keyof ToolOptions
type NumberKey = { [K in OptionKey]: ToolOptions[K] extends number ? K : never }[OptionKey]

/** The tools whose Smoothing upstream keeps apart (`sync_tool_smoothing`). */
export const SMOOTHING_TOOLS = ['Brush', 'Pencil', 'Eraser'] as const
type SmoothingTool = (typeof SMOOTHING_TOOLS)[number]
const ownSmoothing = (id: PhotoToolId): id is SmoothingTool => (SMOOTHING_TOOLS as readonly string[]).includes(id)

/** Every option, with each smoothing tool's own Smoothing. */
export type OptionsState = { values: ToolOptions; smoothing: Record<SmoothingTool, number> }

/**
 * Each number's range, as upstream's fields hold it: the bars draw these limits and stored values
 * are clamped to them. A bar may narrow one (Fixed Size takes whole pixels, Fixed Ratio fractions).
 */
export const RANGES: Record<NumberKey, readonly [number, number]> = {
  size: [1, 5000],
  hardness: [0, 100],
  opacity: [0, 100],
  flow: [1, 100],
  wet: [0, 100],
  load: [0, 100],
  mix: [0, 100],
  mixerFlow: [0, 100],
  smoothing: [0, 100],
  tolerance: [0, 255],
  feather: [0, 1000],
  marqueeWidth: [0.001, 300000],
  marqueeHeight: [0.001, 300000],
  magneticWidth: [1, 256],
  contrast: [1, 100],
  frequency: [0, 100],
  fillOpacity: [1, 100],
  fontSize: [1, 1296],
  structure: [1, 7],
  colorAdapt: [0, 10],
  exposure: [1, 100],
  strength: [1, 100],
  strokeWidth: [0, 288],
  radius: [0, 10000],
  sides: [3, 100],
  weight: [1, 1000],
  eraserOpacity: [1, 100],
  bgTolerance: [0, 100],
}

/** Upstream's layer blend modes (`BlendMode::LAYER_MODES`): the engine's name and upstream's label. */
export const BLEND_MODES = [
  ['normal', 'Normal'],
  ['dissolve', 'Dissolve'],
  ['darken', 'Darken'],
  ['multiply', 'Multiply'],
  ['colorBurn', 'Color Burn'],
  ['linearBurn', 'Linear Burn'],
  ['darkerColor', 'Darker Color'],
  ['lighten', 'Lighten'],
  ['screen', 'Screen'],
  ['colorDodge', 'Color Dodge'],
  ['linearDodge', 'Linear Dodge (Add)'],
  ['lighterColor', 'Lighter Color'],
  ['overlay', 'Overlay'],
  ['softLight', 'Soft Light'],
  ['hardLight', 'Hard Light'],
  ['vividLight', 'Vivid Light'],
  ['linearLight', 'Linear Light'],
  ['pinLight', 'Pin Light'],
  ['hardMix', 'Hard Mix'],
  ['difference', 'Difference'],
  ['exclusion', 'Exclusion'],
  ['subtract', 'Subtract'],
  ['divide', 'Divide'],
  ['hue', 'Hue'],
  ['saturation', 'Saturation'],
  ['color', 'Color'],
  ['luminosity', 'Luminosity'],
] as const

/** Upstream's crop ratio presets (`CROP_RATIOS`): the key and upstream's label. */
export const CROP_RATIOS = [
  ['', 'Ratio'],
  ['original', 'Original Ratio'],
  ['1:1', '1 : 1 (Square)'],
  ['4:5', '4 : 5 (8 : 10)'],
  ['5:7', '5 : 7'],
  ['2:3', '2 : 3 (4 : 6)'],
  ['16:9', '16 : 9'],
] as const

const BLEND = BLEND_MODES.map(([value]) => value)

/** The values a choice option may take; a stored value outside them falls back to the default. */
const CHOICES: Partial<Record<OptionKey, readonly string[]>> = {
  mode: BLEND,
  gradientMode: BLEND,
  selectionMode: ['replace', 'add', 'subtract', 'intersect'],
  marqueeStyle: ['normal', 'fixedRatio', 'fixedSize'],
  fillSource: ['foreground', 'pattern'],
  gradientStyle: ['linear', 'radial', 'angle', 'reflected', 'diamond'],
  autoSelectTarget: ['layer', 'group'],
  sampleSize: ['point', '3', '5', '11', '31', '51', '101'],
  eyedropperSample: ['current', 'currentAndBelow', 'all', 'allNoAdjustments', 'currentAndBelowNoAdjustments'],
  textAntiAlias: ['none', 'sharp', 'crisp', 'strong', 'smooth'],
  textAlign: ['left', 'center', 'right'],
  penMode: ['path', 'shape'],
  range: ['shadows', 'midtones', 'highlights'],
  spongeMode: ['desaturate', 'saturate'],
  cloneSample: ['current', 'currentAndBelow', 'all'],
  spotType: ['smart', 'createTexture', 'proximityMatch'],
  patchMode: ['source', 'destination'],
  camMode: ['move', 'extend'],
  bgSampling: ['continuous', 'once', 'backgroundSwatch'],
  bgLimits: ['discontiguous', 'contiguous', 'findEdges'],
}

/** Free-form strings: their form, and a length cap. */
const PATTERNS: Partial<Record<OptionKey, RegExp>> = {
  cropRatio: /^(|original|\d+(\.\d+)?:\d+(\.\d+)?)$/,
  noteColor: /^#[0-9a-f]{6}$/,
}
const MAX_TEXT = 200

const BRUSH = ['size', 'hardness', 'mode', 'opacity', 'pressureOpacity', 'flow', 'smoothing', 'pressureSize'] as const
const TIP = ['size', 'hardness'] as const
const FILL_SAMPLING = ['tolerance', 'antiAlias', 'contiguous', 'sampleAllLayers'] as const
const MARQUEE = ['selectionMode', 'feather', 'antiAlias', 'marqueeStyle', 'marqueeWidth', 'marqueeHeight'] as const
const LASSO = ['selectionMode', 'feather', 'antiAlias'] as const
const SHAPE = ['shapeFill', 'strokeWidth'] as const
const TYPE = ['font', 'fontStyle', 'fontSize', 'textAntiAlias', 'textAlign'] as const
const SMUDGE_LIKE = [...TIP, 'strength', 'sampleAllLayers'] as const
const TONE = [...TIP, 'range', 'exposure', 'protectTones'] as const
const CLONE = [...TIP, 'aligned', 'cloneSample'] as const

/**
 * The options each tool's bar draws, in its order (upstream's `options_bar` arms). A test holds
 * the bars to this list, and every option to some tool, so nothing is drawn without being kept.
 */
export const TOOL_FIELDS: Record<UpstreamToolId, readonly OptionKey[]> = {
  Move: ['autoSelect', 'autoSelectTarget', 'showTransform'],
  RectMarquee: MARQUEE,
  EllipseMarquee: MARQUEE,
  Lasso: LASSO,
  PolygonLasso: LASSO,
  MagneticLasso: [...LASSO, 'magneticWidth', 'contrast', 'frequency'],
  ObjectSelection: ['sampleAllLayers'],
  QuickSelection: ['size', 'sampleAllLayers', 'enhanceEdge'],
  MagicWand: ['selectionMode', ...FILL_SAMPLING],
  Crop: ['cropRatio', 'cropOverlay', 'deleteCropped'],
  Slice: [],
  SliceSelect: [],
  Eyedropper: ['sampleSize', 'eyedropperSample'],
  Ruler: [],
  Note: ['noteAuthor', 'noteColor'],
  Count: [],
  SpotHealing: [...TIP, 'spotType', 'sampleAllLayers'],
  Healing: CLONE,
  Patch: ['patchMode'],
  SmartMove: ['camMode', 'structure', 'colorAdapt', 'sampleAllLayers'],
  Brush: BRUSH,
  Pencil: ['size', 'mode', 'opacity', 'smoothing', 'autoErase'],
  MixerBrush: [...TIP, 'wet', 'load', 'mix', 'mixerFlow', 'mixerSampleAll'],
  CloneStamp: CLONE,
  HistoryBrush: TIP,
  Eraser: BRUSH,
  BackgroundEraser: [...TIP, 'bgSampling', 'bgLimits', 'bgTolerance', 'protectForeground'],
  MagicEraser: [...FILL_SAMPLING, 'eraserOpacity'],
  Gradient: ['gradientClassic', 'gradientStyle', 'gradientMode', 'fillOpacity', 'reverse', 'dither'],
  PaintBucket: ['fillSource', 'fillOpacity', ...FILL_SAMPLING],
  Blur: SMUDGE_LIKE,
  Sharpen: [...SMUDGE_LIKE, 'protectDetail'],
  Smudge: [...SMUDGE_LIKE, 'fingerPainting'],
  Dodge: TONE,
  Burn: TONE,
  Sponge: [...TIP, 'spongeMode', 'vibrance'],
  Pen: ['penMode'],
  Type: TYPE,
  VerticalType: TYPE,
  PathSelection: [],
  DirectSelection: [],
  Rectangle: [...SHAPE, 'radius'],
  EllipseShape: SHAPE,
  Triangle: SHAPE,
  Polygon: [...SHAPE, 'sides'],
  Line: [...SHAPE, 'weight'],
  CustomShape: [...SHAPE, 'customShape'],
  Hand: [],
  Zoom: ['scrubbyZoom'],
}

/**
 * The options of the tools Kition Photo adds (plan 2026-11-24, stage 7): only options upstream's
 * tools keep, so nothing new is stored.
 */
export const ADDED_TOOL_FIELDS: Record<AddedToolId, readonly OptionKey[]> = {
  SingleRowMarquee: LASSO,
  SingleColumnMarquee: LASSO,
  RedEye: [],
  ColorReplacement: [...TIP, 'bgSampling', 'bgLimits', 'bgTolerance', 'antiAlias'],
  PatternStamp: [...TIP, 'aligned'],
  ConvertPoint: [],
}

/** The options `tool`'s bar draws. */
export function toolFields(tool: PhotoToolId): readonly OptionKey[] {
  return (TOOL_FIELDS as Partial<Record<PhotoToolId, readonly OptionKey[]>>)[tool] ?? ADDED_TOOL_FIELDS[tool as AddedToolId] ?? []
}

/** `value` for option `key`, if it may stand for it: its default's type, clamped to its range, one of its choices, its form. */
function valid(key: OptionKey, value: unknown): unknown {
  const fallback = DEFAULT_OPTIONS[key]
  if (typeof value !== typeof fallback) return undefined
  if (typeof value === 'number') {
    if (!Number.isFinite(value)) return undefined
    const range = (RANGES as Partial<Record<OptionKey, readonly [number, number]>>)[key]
    return range ? Math.min(range[1], Math.max(range[0], value)) : value
  }
  if (typeof value !== 'string') return value
  const choices = CHOICES[key]
  if (choices) return choices.includes(value) ? value : undefined
  const pattern = PATTERNS[key]
  if (pattern) return pattern.test(value) ? value : undefined
  return value.slice(0, MAX_TEXT)
}

/** Every option at upstream's defaults. */
export function defaultOptions(): OptionsState {
  return { values: { ...DEFAULT_OPTIONS }, smoothing: { Brush: DEFAULT_OPTIONS.smoothing, Pencil: DEFAULT_OPTIONS.smoothing, Eraser: DEFAULT_OPTIONS.smoothing } }
}

/**
 * The options from stored data of any earlier shape: each known option where it is valid
 * (numbers clamped to their range), its default for the rest; unknown keys are dropped.
 */
export function mergeOptions(stored: unknown): OptionsState {
  const state = defaultOptions()
  if (!stored || typeof stored !== 'object') return state
  const { values, smoothing } = stored as { values?: unknown; smoothing?: unknown }
  if (values && typeof values === 'object')
    for (const key of Object.keys(DEFAULT_OPTIONS) as OptionKey[]) {
      const value = valid(key, (values as Record<string, unknown>)[key])
      if (value !== undefined) (state.values as Record<string, unknown>)[key] = value
    }
  if (smoothing && typeof smoothing === 'object')
    for (const tool of SMOOTHING_TOOLS) {
      const value = valid('smoothing', (smoothing as Record<string, unknown>)[tool])
      if (typeof value === 'number') state.smoothing[tool] = value
    }
  return state
}

/** A tool's options: the shared ones, with its own Smoothing where it keeps one. */
export function optionsFor(state: OptionsState, id: PhotoToolId): ToolOptions {
  return ownSmoothing(id) ? { ...state.values, smoothing: state.smoothing[id] } : state.values
}

/** The options after a tool's bar changed some: Smoothing to the tool's own, the rest shared. */
export function withOptions(state: OptionsState, id: PhotoToolId, patch: Partial<ToolOptions>): OptionsState {
  const { smoothing, ...shared } = patch
  return {
    values: { ...state.values, ...shared },
    smoothing: smoothing !== undefined && ownSmoothing(id) ? { ...state.smoothing, [id]: smoothing } : state.smoothing,
  }
}

const fraction = (percent: number) => percent / 100
const tip = (o: ToolOptions) => ({ size: o.size, hardness: o.hardness })
const smudgeLike = (o: ToolOptions) => ({ ...tip(o), strength: o.strength, sampleAllLayers: o.sampleAllLayers })
const tone = (o: ToolOptions) => ({ ...tip(o), range: o.range, exposure: o.exposure, protectTones: o.protectTones })
const clone = (o: ToolOptions) => ({ ...tip(o), aligned: o.aligned, sampleLayer: o.cloneSample })
const fillSampling = (o: ToolOptions) => ({ tolerance: o.tolerance, antiAlias: o.antiAlias, contiguous: o.contiguous, sampleAllLayers: o.sampleAllLayers })
const brush = (o: ToolOptions) => ({
  size: o.size,
  hardness: fraction(o.hardness),
  opacity: fraction(o.opacity),
  flow: fraction(o.flow),
  mode: o.mode,
  smoothing: fraction(o.smoothing),
  brush: { pressureSize: o.pressureSize, pressureOpacity: o.pressureOpacity },
})
const lasso = (o: ToolOptions) => ({ feather: o.feather, antiAlias: o.antiAlias })
/**
 * New type's options, as upstream's Type tool sends them (`type_tool::pointer_up`: font, style,
 * size, and alignment, with the foreground colour the editor adds): every other character and
 * paragraph setting comes from the engine's default type styles (Type › Save Default Type
 * Styles). The anti-aliasing is the layer's own, not a type style, so sending it overrides no
 * saved default.
 */
const type = (o: ToolOptions) => ({ font: o.font, fontStyle: o.fontStyle, size: o.fontSize, align: o.textAlign, antialias: o.textAntiAlias })

/**
 * The parameters a tool's command takes from its options, in the command's own units: Brush,
 * Eraser, Pencil, and Background Eraser take fractions (0 to 1), the retouching tools percentages
 * (0 to 100). The Pencil and Mixer Brush take what their command lacks a parameter for through
 * the `brush` patch (`resolve_brush`). Shapes and the Pen add what depends on the colours.
 */
const PARAMS: Partial<Record<PhotoToolId, (o: ToolOptions) => Record<string, unknown>>> = {
  Brush: brush,
  Eraser: brush,
  Pencil: (o) => ({ size: o.size, opacity: fraction(o.opacity), mode: o.mode, autoErase: o.autoErase, brush: { smoothing: { amount: fraction(o.smoothing) } } }),
  MixerBrush: (o) => ({ size: o.size, brush: { hardness: fraction(o.hardness) }, wet: o.wet, load: o.load, mix: o.mix, flow: o.mixerFlow, sampleAllLayers: o.mixerSampleAll }),
  BackgroundEraser: (o) => ({ size: o.size, hardness: fraction(o.hardness), sampling: o.bgSampling, limits: o.bgLimits, tolerance: o.bgTolerance, protectForegroundColor: o.protectForeground }),
  CloneStamp: clone,
  // The tools added in plan 2026-11-24, stage 7.
  SingleRowMarquee: (o) => ({ feather: o.feather }),
  SingleColumnMarquee: (o) => ({ feather: o.feather }),
  ColorReplacement: (o) => ({ size: o.size, brush: { hardness: fraction(o.hardness) }, sampling: o.bgSampling, limits: o.bgLimits, tolerance: o.bgTolerance, antiAlias: o.antiAlias }),
  PatternStamp: (o) => ({ size: o.size, hardness: o.hardness, aligned: o.aligned }),
  Healing: clone,
  SpotHealing: (o) => ({ ...tip(o), type: o.spotType, sampleAllLayers: o.sampleAllLayers }),
  HistoryBrush: tip,
  Blur: smudgeLike,
  Sharpen: (o) => ({ ...smudgeLike(o), protectDetail: o.protectDetail }),
  Smudge: (o) => ({ ...smudgeLike(o), fingerPainting: o.fingerPainting }),
  Dodge: tone,
  Burn: tone,
  Sponge: (o) => ({ ...tip(o), mode: o.spongeMode, vibrance: o.vibrance }),
  QuickSelection: (o) => ({ size: o.size, enhanceEdge: o.enhanceEdge, sampleAllLayers: o.sampleAllLayers }),
  ObjectSelection: (o) => ({ sampleAllLayers: o.sampleAllLayers }),
  RectMarquee: (o) => ({ feather: o.feather }),
  EllipseMarquee: lasso,
  Lasso: lasso,
  PolygonLasso: lasso,
  MagneticLasso: (o) => ({ ...lasso(o), width: o.magneticWidth, contrast: o.contrast }),
  MagicWand: fillSampling,
  MagicEraser: (o) => ({ ...fillSampling(o), opacity: o.eraserOpacity }),
  PaintBucket: (o) => ({ contents: o.fillSource, opacity: o.fillOpacity, ...fillSampling(o) }),
  Gradient: (o) => ({ style: o.gradientStyle, mode: o.gradientMode, opacity: o.fillOpacity, reverse: o.reverse, dither: o.dither }),
  Patch: (o) => ({ mode: o.patchMode }),
  SmartMove: (o) => ({ mode: o.camMode, structure: o.structure, color: o.colorAdapt, sampleAllLayers: o.sampleAllLayers }),
  Crop: (o) => ({ deleteCroppedPixels: o.deleteCropped }),
  Eyedropper: (o) => ({ size: o.sampleSize === 'point' ? 'point' : Number(o.sampleSize), sampleLayer: o.eyedropperSample }),
  Note: (o) => ({ author: o.noteAuthor, color: o.noteColor }),
  Type: type,
  VerticalType: type,
}

/** A tool's command parameters from its options (nothing for a tool whose options take none). */
export function toolParams(id: PhotoToolId, options: ToolOptions): Record<string, unknown> {
  return PARAMS[id]?.(options) ?? {}
}

/** A shape's fill (the foreground colour, or none) and stroke (the background colour at the stroke width, or none), as upstream. */
export function shapeStyle(options: ToolOptions, foreground: string, background: string) {
  return { fill: options.shapeFill ? foreground : null, stroke: options.strokeWidth > 0 ? { width: options.strokeWidth, color: background } : null }
}

/**
 * How a selection combines (upstream's `tool_feedback::selection_mode`): Quick Selection adds
 * (Alt subtracts); Object Selection replaces (Shift adds, Alt subtracts); the other tools follow
 * Shift and Alt, else the options bar's mode.
 */
export function selectionModeFor(id: PhotoToolId, bar: SelectionMode, keys: { shift: boolean; alt: boolean }): SelectionMode {
  if (id === 'QuickSelection') return keys.alt ? 'subtract' : 'add'
  if (id === 'ObjectSelection') return keys.alt ? 'subtract' : keys.shift ? 'add' : 'replace'
  if (keys.shift && keys.alt) return 'intersect'
  if (keys.shift) return 'add'
  if (keys.alt) return 'subtract'
  return bar
}

type Point = { x: number; y: number }

/**
 * Where a marquee dragged from `start` to `end` ends (upstream's `marquee_end`): a fixed size, a
 * fixed ratio (the larger drag extent wins), a square with Shift, or the pointer.
 */
export function marqueeEnd(style: string, width: number, height: number, shift: boolean, start: Point, end: Point): Point {
  const dx = end.x - start.x
  const dy = end.y - start.y
  const sx = dx < 0 ? -1 : 1
  const sy = dy < 0 ? -1 : 1
  if (style === 'fixedSize') return { x: start.x + sx * Math.max(1, width), y: start.y + sy * Math.max(1, height) }
  if (style === 'fixedRatio' && width > 0 && height > 0) {
    const k = Math.max(Math.abs(dx) / width, Math.abs(dy) / height)
    return { x: start.x + sx * k * width, y: start.y + sy * k * height }
  }
  if (shift) {
    const side = Math.max(Math.abs(dx), Math.abs(dy))
    return { x: start.x + sx * side, y: start.y + sy * side }
  }
  return end
}

/** The whole pixels between two corners (upstream's `marquee_px`), as an area. */
export function pixelArea(a: Point, b: Point) {
  const x0 = Math.floor(Math.min(a.x, b.x))
  const y0 = Math.floor(Math.min(a.y, b.y))
  const x1 = Math.ceil(Math.max(a.x, b.x))
  const y1 = Math.ceil(Math.max(a.y, b.y))
  return { x: x0, y: y0, width: x1 - x0, height: y1 - y0 }
}

/** A crop ratio key's width and height (`original` is the document's), or null for none. */
export function cropRatioSize(key: string, documentWidth: number, documentHeight: number): [number, number] | null {
  if (key === 'original') return [documentWidth, documentHeight]
  const [a, b] = key.split(':').map(Number)
  return a > 0 && b > 0 ? [a, b] : null
}

/** A crop ratio key from typed width and height (upstream writes `w:h`), or none when either is not positive. */
export function cropRatioKey(width: number, height: number) {
  return width > 0 && height > 0 ? `${+width.toFixed(3)}:${+height.toFixed(3)}` : ''
}

/** The screen distance the Magnetic Lasso's border runs before it fastens a point by itself (upstream: 8 at Frequency 100, 98 at 0). */
export function fasteningSpacing(frequency: number) {
  const f = Number.isFinite(frequency) ? Math.min(100, Math.max(0, frequency)) : 57
  return 8 + (100 - f) * 0.9
}
