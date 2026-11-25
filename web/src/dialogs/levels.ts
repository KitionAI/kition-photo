/**
 * Levels' numbers (Photoshop's Levels dialog, as the engine's `image.adjustments.levels` takes
 * them): per channel the input black, gamma and white points and the output range. The midtone
 * handle sits where the input that maps to middle grey is: a fraction `0.5 ^ gamma` of the way from
 * black to white, so gamma 1 is halfway and a brightening gamma (2) moves it towards black. Pure.
 */

export type Levels = { inBlack: number; gamma: number; inWhite: number; outBlack: number; outWhite: number }

export const LEVELS_IDENTITY: Levels = { inBlack: 0, gamma: 1, inWhite: 255, outBlack: 0, outWhite: 255 }

/** The channels Levels and Curves edit in an RGB image: the composite and each colour. */
export const RGB_CHANNELS = ['rgb', 'red', 'green', 'blue'] as const
export type RgbChannel = (typeof RGB_CHANNELS)[number]

const clamp = (value: number, min: number, max: number) => Math.min(max, Math.max(min, value))
const round2 = (value: number) => Math.round(value * 100) / 100

export const GAMMA_MIN = 0.01
export const GAMMA_MAX = 9.99

/** Where the midtone handle sits between black and white (0..1) for `gamma`. */
export function gammaFraction(gamma: number) {
  return 0.5 ** clamp(gamma, GAMMA_MIN, GAMMA_MAX)
}

/** The gamma the midtone handle means at `fraction` of the way from black to white. */
export function gammaAt(fraction: number) {
  const f = clamp(fraction, 1e-6, 1 - 1e-6)
  return round2(clamp(Math.log(f) / Math.log(0.5), GAMMA_MIN, GAMMA_MAX))
}

/** The midtone handle's input value (0..255) for these levels. */
export function midtoneInput(levels: Levels) {
  return levels.inBlack + (levels.inWhite - levels.inBlack) * gammaFraction(levels.gamma)
}

/**
 * `levels` with one value changed, kept in the engine's ranges: black stays at least 2 below white
 * (inBlack 0..253, inWhite 2..255), gamma in 0.01..9.99, outputs 0..255.
 */
export function setLevel(levels: Levels, key: keyof Levels, value: number): Levels {
  const next = { ...levels }
  if (key === 'inBlack') next.inBlack = Math.round(clamp(value, 0, Math.min(253, levels.inWhite - 2)))
  else if (key === 'inWhite') next.inWhite = Math.round(clamp(value, Math.max(2, levels.inBlack + 2), 255))
  else if (key === 'gamma') next.gamma = round2(clamp(value, GAMMA_MIN, GAMMA_MAX))
  else next[key] = Math.round(clamp(value, 0, 255))
  return next
}

/** The gamma that puts the midtone handle at input `value` (dragging it). */
export function gammaForMidtone(levels: Levels, value: number) {
  const span = levels.inWhite - levels.inBlack
  return gammaAt(span > 0 ? (value - levels.inBlack) / span : 0.5)
}

const same = (a: Levels, b: Levels) => a.inBlack === b.inBlack && a.gamma === b.gamma && a.inWhite === b.inWhite && a.outBlack === b.outBlack && a.outWhite === b.outWhite

/** The command's parameters: the composite at the top level, each colour channel that differs from the identity under its name. */
export function levelsParams(channels: Readonly<Record<RgbChannel, Levels>>): Record<string, unknown> {
  const params: Record<string, unknown> = { ...channels.rgb }
  for (const channel of RGB_CHANNELS) {
    if (channel !== 'rgb' && !same(channels[channel], LEVELS_IDENTITY)) params[channel] = { ...channels[channel] }
  }
  return params
}

/** Levels read back from an adjustment layer's settings (missing values are the identity's). */
export function levelsFrom(params: Readonly<Record<string, unknown>>): Record<RgbChannel, Levels> {
  const read = (source: unknown): Levels => {
    const values = source && typeof source === 'object' && !Array.isArray(source) ? (source as Record<string, unknown>) : {}
    const num = (key: keyof Levels) => (typeof values[key] === 'number' && Number.isFinite(values[key]) ? (values[key] as number) : LEVELS_IDENTITY[key])
    return { inBlack: num('inBlack'), gamma: num('gamma'), inWhite: num('inWhite'), outBlack: num('outBlack'), outWhite: num('outWhite') }
  }
  return { rgb: read(params), red: read(params.red), green: read(params.green), blue: read(params.blue) }
}

/** A curve as the engine takes it: `[in, out]` points in 0..255. */
export type EngineCurve = [number, number][]

/** Curve points in 0..1 (the curve editor's) as the engine's 0..255 integers. */
export function toEngineCurve(points: readonly (readonly [number, number])[]): EngineCurve {
  return [...points].sort((a, b) => a[0] - b[0]).map(([x, y]) => [Math.round(clamp(x, 0, 1) * 255), Math.round(clamp(y, 0, 1) * 255)])
}

/** The engine's 0..255 curve as the editor's 0..1 points; a missing or broken one is the identity. */
export function fromEngineCurve(curve: unknown): [number, number][] {
  if (!Array.isArray(curve) || curve.length < 2) return [[0, 0], [1, 1]]
  const points = curve.filter((point): point is [number, number] => Array.isArray(point) && typeof point[0] === 'number' && typeof point[1] === 'number')
  return points.length >= 2 ? points.map(([x, y]) => [clamp(x, 0, 255) / 255, clamp(y, 0, 255) / 255]) : [[0, 0], [1, 1]]
}

const identityCurve = (curve: EngineCurve) => curve.length === 2 && curve[0][0] === 0 && curve[0][1] === 0 && curve[1][0] === 255 && curve[1][1] === 255

/** Curves' parameters: the composite as `points`, each colour channel that is not the identity under its name. */
export function curvesParams(curves: Readonly<Record<RgbChannel, readonly (readonly [number, number])[]>>): Record<string, unknown> {
  const params: Record<string, unknown> = { points: toEngineCurve(curves.rgb) }
  for (const channel of RGB_CHANNELS) {
    if (channel === 'rgb') continue
    const curve = toEngineCurve(curves[channel])
    if (!identityCurve(curve)) params[channel] = curve
  }
  return params
}
