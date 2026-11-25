// Imports carry `.ts` so `node --test` loads this module as it is.
import { BLEND_MODES } from './params.ts'

/**
 * The Layer Style dialog's model (plan 2026-11-24, stage 5; the reference's `layer_style.rs`): the
 * layer's effects as the engine reads them back (`layer.layerStyle.get`), each kind's fields in
 * Photoshop's order, the list on the left in Photoshop's order, and what OK sends: the whole list
 * through `layer.layerStyle.replace` (each effect with its snapshot, so what no field names
 * survives) and the blending options through `layer.layerStyle.blendingOptions`, in one history
 * step. Pure, so `node --test` checks it.
 */

/** Photoshop's list order, top to bottom. */
export const EFFECT_KINDS = ['bevelEmboss', 'stroke', 'innerShadow', 'innerGlow', 'satin', 'colorOverlay', 'gradientOverlay', 'patternOverlay', 'outerGlow', 'dropShadow'] as const
export type EffectKind = (typeof EFFECT_KINDS)[number]

/** Each kind's English name (upstream's phrase). */
export const EFFECT_LABELS: Readonly<Record<EffectKind, string>> = {
  bevelEmboss: 'Bevel & Emboss',
  stroke: 'Stroke',
  innerShadow: 'Inner Shadow',
  innerGlow: 'Inner Glow',
  satin: 'Satin',
  colorOverlay: 'Color Overlay',
  gradientOverlay: 'Gradient Overlay',
  patternOverlay: 'Pattern Overlay',
  outerGlow: 'Outer Glow',
  dropShadow: 'Drop Shadow',
}

export const isEffectKind = (value: unknown): value is EffectKind => typeof value === 'string' && (EFFECT_KINDS as readonly string[]).includes(value)

/** The engine's built-in contours. */
export const CONTOURS = ['Linear', 'Cone', 'Cone (Inverted)', 'Domed', 'Domed (Inverted)', 'Diagonal (Descending)'] as const

/** A field of an effect: a slider (with its unit), a choice, a check box, a colour, a pattern, or the gradient. */
export type FxField =
  | { key: string; kind: 'slider'; min: number; max: number; softMax?: number; unit: string }
  | { key: string; kind: 'choice'; options: readonly string[] }
  | { key: string; kind: 'check' }
  | { key: string; kind: 'color' }
  | { key: string; kind: 'pattern'; optional: boolean }
  | { key: string; kind: 'gradient' }

const slider = (key: string, min: number, max: number, unit: string, softMax?: number): FxField => ({ key, kind: 'slider', min, max, unit, softMax })
const choice = (key: string, options: readonly string[]): FxField => ({ key, kind: 'choice', options })
const check = (key: string): FxField => ({ key, kind: 'check' })
const color = (key: string): FxField => ({ key, kind: 'color' })
const BLEND = choice('blend', BLEND_MODES)
const OPACITY = slider('opacity', 0, 100, '%')
const ANGLE = slider('angle', -180, 180, '°')
const DISTANCE = slider('distance', 0, 30000, 'px', 250)
const SIZE = slider('size', 0, 250, 'px')
const NOISE = slider('noise', 0, 100, '%')
const CONTOUR = choice('contour', CONTOURS)

/** Each kind's fields in Photoshop's order (Structure, then Quality). */
export const EFFECT_FIELDS: Readonly<Record<EffectKind, readonly FxField[]>> = {
  dropShadow: [BLEND, color('color'), OPACITY, ANGLE, check('useGlobalLight'), DISTANCE, slider('spread', 0, 100, '%'), SIZE, CONTOUR, NOISE, check('knocksOut')],
  innerShadow: [BLEND, color('color'), OPACITY, ANGLE, check('useGlobalLight'), DISTANCE, slider('choke', 0, 100, '%'), SIZE, CONTOUR, NOISE],
  outerGlow: [BLEND, OPACITY, NOISE, color('color'), choice('technique', ['softer', 'precise']), slider('spread', 0, 100, '%'), SIZE, CONTOUR, slider('range', 1, 100, '%')],
  innerGlow: [BLEND, OPACITY, NOISE, color('color'), choice('technique', ['softer', 'precise']), choice('source', ['center', 'edge']), slider('choke', 0, 100, '%'), SIZE, CONTOUR, slider('range', 1, 100, '%')],
  bevelEmboss: [
    choice('style', ['outer', 'inner', 'emboss', 'pillow', 'stroke']),
    choice('technique', ['smooth', 'chiselHard', 'chiselSoft']),
    slider('depth', 1, 1000, '%', 500),
    choice('direction', ['up', 'down']),
    SIZE,
    slider('soften', 0, 16, 'px'),
    ANGLE,
    slider('altitude', 0, 90, '°'),
    check('useGlobalLight'),
    check('contour'),
    slider('contourRange', 1, 100, '%'),
    { key: 'texture', kind: 'pattern', optional: true },
    slider('textureScale', 1, 1000, '%', 200),
    slider('textureDepth', -1000, 1000, '%'),
  ],
  satin: [BLEND, color('color'), OPACITY, ANGLE, DISTANCE, SIZE, check('invert')],
  colorOverlay: [BLEND, color('color'), OPACITY],
  gradientOverlay: [BLEND, OPACITY, { key: 'gradient', kind: 'gradient' }, check('reverse'), choice('style', ['linear', 'radial', 'angle', 'reflected', 'diamond']), ANGLE, slider('scale', 10, 150, '%'), check('dither')],
  patternOverlay: [BLEND, OPACITY, { key: 'pattern', kind: 'pattern', optional: false }, slider('scale', 1, 1000, '%', 200), ANGLE, check('link')],
  stroke: [SIZE, choice('position', ['outside', 'inside', 'center']), BLEND, OPACITY, color('color')],
}

/** A gradient stop: where (0 to 1) and what colour (`#rrggbb`); an opacity stop: where and how opaque (0 to 100). */
export type ColorStop = { at: number; color: string }
export type OpacityStop = { at: number; opacity: number }

/** One effect in the dialog: its kind, whether it is on, its fields' values, its snapshot from the layer (null for one added here), and whether the layer had it. */
export type StyleEntry = { kind: EffectKind; enabled: boolean; params: Record<string, unknown>; fx: unknown; original: boolean }

/** The blending options the dialog shows (Photoshop's General Blending): mode, opacity and fill opacity, in percent. */
export type Blending = { blend: string; opacity: number; fill: number }

export type StyleState = { entries: StyleEntry[]; blending: Blending }

/** What `layer.layerStyle.get` answers. */
export type StyleAnswer = { enabled?: boolean; items?: { kind?: unknown; enabled?: unknown; params?: unknown; fx?: unknown }[] } | null

/** The engine's blend mode label ("Linear Dodge (Add)", "Normal") as the commands name it ("linearDodge"). */
export function blendKey(label: string | undefined): string {
  if (!label) return 'normal'
  const compact = label.replace(/\s*\(Add\)\s*/i, '').replace(/[^a-z]/gi, '').toLowerCase()
  return BLEND_MODES.find((mode) => mode.toLowerCase() === compact) ?? 'normal'
}

/** The dialog's starting state from the layer's style and its blending (`opacity`, `fill` 0 to 1, `blend` the engine's label). */
export function styleFromLayer(answer: StyleAnswer, layer: { opacity?: number; fill?: number; blend?: string }): StyleState {
  const entries: StyleEntry[] = (answer?.items ?? []).flatMap((item) => {
    if (!isEffectKind(item.kind)) return []
    const params = item.params && typeof item.params === 'object' ? { ...(item.params as Record<string, unknown>) } : {}
    delete params.enabled
    return [{ kind: item.kind, enabled: item.enabled !== false, params, fx: item.fx ?? null, original: true }]
  })
  return {
    entries,
    blending: { blend: blendKey(layer.blend), opacity: Math.round((layer.opacity ?? 1) * 100), fill: Math.round((layer.fill ?? 1) * 100) },
  }
}

/** A row of the list: one per effect the dialog has, else a placeholder for a kind it has none of (`index` null). */
export type StyleRow = { kind: EffectKind; index: number | null; enabled: boolean }

/** The list's rows in Photoshop's order: every effect of a kind under its name (two strokes, two rows), or the kind's empty row. */
export function styleRows(state: StyleState): StyleRow[] {
  return EFFECT_KINDS.flatMap((kind): StyleRow[] => {
    const own = state.entries.flatMap((entry, index): StyleRow[] => (entry.kind === kind ? [{ kind, index, enabled: entry.enabled }] : []))
    return own.length ? own : [{ kind, index: null, enabled: false }]
  })
}

/**
 * The state with effect `index` switched on or off; a kind's empty row (`index` null) adds a new
 * effect of it, on, from `defaults`. Answers the state and the entry's index.
 */
export function toggleEffect(state: StyleState, kind: EffectKind, index: number | null, on: boolean, defaults: Record<string, unknown>): { state: StyleState; index: number } {
  if (index == null) {
    const params = { ...defaults }
    delete params.enabled
    const entries = [...state.entries, { kind, enabled: on, params, fx: null, original: false }]
    return { state: { ...state, entries }, index: entries.length - 1 }
  }
  return { state: { ...state, entries: state.entries.map((entry, at) => (at === index ? { ...entry, enabled: on } : entry)) }, index }
}

/** The state with `patch` in effect `index`'s values (a field the person changed). */
export function setEffectParams(state: StyleState, index: number, patch: Record<string, unknown>): StyleState {
  return { ...state, entries: state.entries.map((entry, at) => (at === index ? { ...entry, params: { ...entry.params, ...patch } } : entry)) }
}

/** The state with effect `index`'s gradient (a gradient overlay's) as the gradient editor left it. */
export function setEffectGradient(state: StyleState, index: number, stops: readonly ColorStop[], opacity: readonly OpacityStop[]): StyleState {
  return { ...state, entries: state.entries.map((entry, at) => (at === index ? { ...entry, params: { ...entry.params, stops, opacityStops: opacity, from: stops[0]?.color, to: stops.at(-1)?.color } } : entry)) }
}

/** A colour as the engine's snapshot keeps one (RGB, 0 to 1). */
const snapshotColor = (hex: string) => {
  const value = /^#?([0-9a-f]{6})$/i.exec(hex.trim())?.[1] ?? '000000'
  return { mode: 'Rgb', c: [0, 2, 4].map((index) => parseInt(value.slice(index, index + 2), 16) / 255).concat(0), alpha: 1 }
}

/** The engine's names for a gradient style and a blend mode in a snapshot ("Linear", "Multiply"). */
const pascal = (word: string) => word.charAt(0).toUpperCase() + word.slice(1)

/**
 * A gradient overlay's snapshot with the gradient editor's stops: the layer's own snapshot with
 * its stops replaced, or a new one built from the fields (the engine fills in what it lacks).
 */
export function gradientSnapshot(entry: StyleEntry): unknown {
  const stops = entry.params.stops as ColorStop[] | undefined
  const opacity = (entry.params.opacityStops as OpacityStop[] | undefined) ?? []
  if (!stops?.length) return entry.fx
  const gradientStops = stops.map((stop) => [stop.at, snapshotColor(stop.color)])
  const opacityStops = opacity.map((stop) => [stop.at, stop.opacity / 100])
  const own = (entry.fx as { GradientOverlay?: Record<string, unknown> } | null)?.GradientOverlay
  if (own) {
    const gradient = { ...(own.gradient as Record<string, unknown>), stops: gradientStops, opacity_stops: opacityStops }
    return { GradientOverlay: { ...own, gradient } }
  }
  return {
    GradientOverlay: {
      common: { enabled: entry.enabled, blend: pascal(String(entry.params.blend ?? 'normal')), opacity: Number(entry.params.opacity ?? 100) / 100 },
      gradient: { stops: gradientStops, opacity_stops: opacityStops, style: pascal(String(entry.params.style ?? 'linear')), angle: Number(entry.params.angle ?? 90), scale: Number(entry.params.scale ?? 100) / 100, reverse: entry.params.reverse === true, align: true, offset: [0, 0] },
      dither: entry.params.dither === true,
    },
  }
}

/**
 * `layer.layerStyle.replace`'s effects: every effect the layer had (on or off) and every one added
 * and left on, each with its values (and `enabled`) over its snapshot. An added effect switched
 * off again is dropped, as it was never there. A gradient overlay edited in the gradient editor
 * carries its stops in its snapshot (its ends' colours stay out, so they do not undo the stops).
 */
export function replaceEffects(state: StyleState): { kind: EffectKind; params: Record<string, unknown>; fx?: unknown }[] {
  return state.entries
    .filter((entry) => entry.original || entry.enabled)
    .map((entry) => {
      const params: Record<string, unknown> = { ...entry.params, enabled: entry.enabled }
      let fx = entry.fx
      if (entry.kind === 'gradientOverlay' && Array.isArray(params.stops)) {
        fx = gradientSnapshot(entry)
        delete params.from
        delete params.to
      }
      delete params.stops
      delete params.opacityStops
      // A pattern left empty (no library pattern) leaves the engine's default.
      if ((entry.kind === 'patternOverlay' && !params.pattern) || (entry.kind === 'bevelEmboss' && !params.texture)) {
        delete params.pattern
        delete params.texture
      }
      return fx ? { kind: entry.kind, params, fx } : { kind: entry.kind, params }
    })
}

/** `layer.layerStyle.blendingOptions`' values when they differ from where the dialog started, else null. */
export function blendingChange(state: StyleState, start: StyleState): Record<string, unknown> | null {
  const now = state.blending
  const was = start.blending
  if (now.blend === was.blend && now.opacity === was.opacity && now.fill === was.fill) return null
  return { blend: now.blend, opacity: now.opacity, fillOpacity: now.fill }
}

/** The gradient a gradient overlay shows: the editor's stops, else its two end colours. */
export function effectGradient(entry: StyleEntry): { stops: ColorStop[]; opacity: OpacityStop[] } {
  const stops = entry.params.stops as ColorStop[] | undefined
  if (stops?.length) return { stops: [...stops], opacity: [...((entry.params.opacityStops as OpacityStop[] | undefined) ?? [])] }
  const own = (entry.fx as { GradientOverlay?: { gradient?: { stops?: [number, { c?: number[] }][]; opacity_stops?: [number, number][] } } } | null)?.GradientOverlay?.gradient
  if (own?.stops?.length) {
    const hex = (c: number[] | undefined) => `#${(c ?? [0, 0, 0]).slice(0, 3).map((value) => Math.round(Math.min(1, Math.max(0, value)) * 255).toString(16).padStart(2, '0')).join('')}`
    return { stops: own.stops.map(([at, value]) => ({ at, color: hex(value?.c) })), opacity: (own.opacity_stops ?? []).map(([at, value]) => ({ at, opacity: Math.round(value * 100) })) }
  }
  return {
    stops: [
      { at: 0, color: typeof entry.params.from === 'string' ? entry.params.from : '#000000' },
      { at: 1, color: typeof entry.params.to === 'string' ? entry.params.to : '#ffffff' },
    ],
    opacity: [],
  }
}
