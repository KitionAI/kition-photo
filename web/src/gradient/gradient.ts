/**
 * The gradient editor's model (plan 2026-11-24, stage 5; Photoshop's Gradient Editor): colour
 * stops under the bar, opacity stops over it, and a midpoint between each pair of colour stops
 * (where the two colours mix half and half). A colour stop's colour is `#rrggbb` or the engine's
 * `foreground` / `background`, which follow the tool colours. Pure, so `node --test` checks it.
 */

export type ColorStop = { at: number; color: string }
export type OpacityStop = { at: number; opacity: number }
/** `midpoints[i]` sits between the i-th and the next colour stop (in location order), as a fraction of that segment (0.05 to 0.95; 0.5 when absent). */
export type GradientDraft = { stops: ColorStop[]; opacity: OpacityStop[]; midpoints: number[] }

/** The engine's form: `[[location, colour], …]`, `[[location, opacity 0..100], …]`, one midpoint per segment. */
export type EngineGradient = { stops: [number, string][]; transparency: [number, number][]; midpoints?: number[] }

const clamp = (value: number, low: number, high: number) => Math.min(high, Math.max(low, Number.isFinite(value) ? value : low))
const round = (value: number, places = 4) => Math.round(value * 10 ** places) / 10 ** places

/** The smallest a midpoint may sit from a stop (Photoshop's 5 %). */
export const MIDPOINT_MIN = 0.05

/** A colour stop's colour as `#rrggbb`: the foreground and background resolved. */
export function resolveColor(color: string, foreground: string, background: string) {
  if (color === 'foreground') return foreground
  if (color === 'background') return background
  return /^#[0-9a-f]{6}$/i.test(color) ? color.toLowerCase() : '#000000'
}

const toRgb = (hex: string) => [0, 2, 4].map((index) => parseInt(hex.slice(1 + index, 3 + index), 16))
const toHex = (rgb: number[]) => `#${rgb.map((value) => Math.round(clamp(value, 0, 255)).toString(16).padStart(2, '0')).join('')}`

/** A draft from the engine's form (stops sorted by location; missing midpoints at 0.5). */
export function fromEngine(gradient: Partial<EngineGradient> | null | undefined): GradientDraft {
  const stops = (gradient?.stops ?? [])
    .filter((stop): stop is [number, string] => Array.isArray(stop) && typeof stop[0] === 'number' && typeof stop[1] === 'string')
    .map(([at, color]) => ({ at: clamp(at, 0, 1), color }))
    .sort((a, b) => a.at - b.at)
  const opacity = (gradient?.transparency ?? [])
    .filter((stop): stop is [number, number] => Array.isArray(stop) && typeof stop[0] === 'number' && typeof stop[1] === 'number')
    .map(([at, value]) => ({ at: clamp(at, 0, 1), opacity: clamp(value, 0, 100) }))
    .sort((a, b) => a.at - b.at)
  const draft: GradientDraft = {
    stops: stops.length >= 2 ? stops : [{ at: 0, color: 'foreground' }, { at: 1, color: 'background' }],
    opacity,
    midpoints: [],
  }
  const given = gradient?.midpoints ?? []
  draft.midpoints = draft.stops.slice(1).map((_, index) => clamp(given[index] ?? 0.5, MIDPOINT_MIN, 1 - MIDPOINT_MIN))
  return draft
}

/** The engine's form of a draft: sorted, rounded, the midpoints only when one is not 0.5. */
export function toEngine(draft: GradientDraft): EngineGradient {
  const order = sortedOrder(draft)
  const stops = order.map((index) => [round(draft.stops[index].at), draft.stops[index].color] as [number, string])
  const transparency = [...draft.opacity].sort((a, b) => a.at - b.at).map((stop) => [round(stop.at), round(stop.opacity, 2)] as [number, number])
  const midpoints = order.slice(1).map((_, index) => round(draft.midpoints[index] ?? 0.5))
  return midpoints.some((mid) => Math.abs(mid - 0.5) > 1e-6) ? { stops, transparency, midpoints } : { stops, transparency }
}

/** The colour stops' indices in location order (a dragged stop may pass another). */
function sortedOrder(draft: GradientDraft) {
  return draft.stops.map((_, index) => index).sort((a, b) => draft.stops[a].at - draft.stops[b].at || a - b)
}

/** Where `t` falls in a segment once its midpoint bends it (the midpoint maps to half). */
export function bend(t: number, midpoint: number) {
  const mid = clamp(midpoint, MIDPOINT_MIN, 1 - MIDPOINT_MIN)
  return t <= mid ? (t / mid) * 0.5 : 0.5 + ((t - mid) / (1 - mid)) * 0.5
}

/** The colour at `t` (0 to 1), midpoints included. */
export function colorAt(draft: GradientDraft, t: number, foreground: string, background: string): string {
  const order = sortedOrder(draft)
  const stops = order.map((index) => ({ at: draft.stops[index].at, rgb: toRgb(resolveColor(draft.stops[index].color, foreground, background)) }))
  if (t <= stops[0].at) return toHex(stops[0].rgb)
  for (let index = 1; index < stops.length; index += 1) {
    const a = stops[index - 1]
    const b = stops[index]
    if (t <= b.at) {
      const span = b.at - a.at
      const local = span <= 0 ? 1 : bend((t - a.at) / span, draft.midpoints[index - 1] ?? 0.5)
      return toHex(a.rgb.map((value, channel) => value + (b.rgb[channel] - value) * local))
    }
  }
  return toHex(stops[stops.length - 1].rgb)
}

/** The opacity (0 to 100) at `t`: 100 without opacity stops. */
export function opacityAt(draft: GradientDraft, t: number): number {
  const stops = [...draft.opacity].sort((a, b) => a.at - b.at)
  if (!stops.length) return 100
  if (t <= stops[0].at) return stops[0].opacity
  for (let index = 1; index < stops.length; index += 1) {
    const a = stops[index - 1]
    const b = stops[index]
    if (t <= b.at) return b.at - a.at <= 0 ? b.opacity : a.opacity + ((b.opacity - a.opacity) * (t - a.at)) / (b.at - a.at)
  }
  return stops[stops.length - 1].opacity
}

/** A new colour stop at `at`, of the colour the gradient has there; answers the draft and its index. */
export function addColorStop(draft: GradientDraft, at: number, foreground: string, background: string): { draft: GradientDraft; index: number } {
  const where = clamp(at, 0, 1)
  const color = colorAt(draft, where, foreground, background)
  const stops = [...draft.stops, { at: where, color }]
  // The segment it splits gets two midpoints at 0.5 (Photoshop resets them).
  const next = { ...draft, stops }
  next.midpoints = sortedOrder(next).slice(1).map(() => 0.5)
  return { draft: keepMidpoints(draft, next), index: stops.length - 1 }
}

/** The midpoints of `before` kept for the segments `after` still has between the same two stops. */
function keepMidpoints(before: GradientDraft, after: GradientDraft): GradientDraft {
  const pairs = new Map<string, number>()
  const old = sortedOrder(before)
  old.slice(1).forEach((index, segment) => pairs.set(`${before.stops[old[segment]].at}|${before.stops[index].at}`, before.midpoints[segment] ?? 0.5))
  const now = sortedOrder(after)
  return { ...after, midpoints: now.slice(1).map((index, segment) => pairs.get(`${after.stops[now[segment]].at}|${after.stops[index].at}`) ?? 0.5) }
}

/** A new opacity stop at `at`, of the opacity there; answers the draft and its index. */
export function addOpacityStop(draft: GradientDraft, at: number): { draft: GradientDraft; index: number } {
  const where = clamp(at, 0, 1)
  const opacity = [...draft.opacity, { at: where, opacity: Math.round(opacityAt(draft, where)) }]
  return { draft: { ...draft, opacity }, index: opacity.length - 1 }
}

/** The draft with a stop moved to `at`. */
export function moveStop(draft: GradientDraft, which: 'color' | 'opacity', index: number, at: number): GradientDraft {
  const where = clamp(at, 0, 1)
  if (which === 'opacity') return { ...draft, opacity: draft.opacity.map((stop, at2) => (at2 === index ? { ...stop, at: where } : stop)) }
  return keepMidpoints(draft, { ...draft, stops: draft.stops.map((stop, at2) => (at2 === index ? { ...stop, at: where } : stop)) })
}

/** The draft without a stop, or null when it would leave fewer than two colour stops. */
export function removeStop(draft: GradientDraft, which: 'color' | 'opacity', index: number): GradientDraft | null {
  if (which === 'opacity') return { ...draft, opacity: draft.opacity.filter((_, at) => at !== index) }
  if (draft.stops.length <= 2) return null
  const after = { ...draft, stops: draft.stops.filter((_, at) => at !== index) }
  return keepMidpoints(draft, { ...after, midpoints: sortedOrder(after).slice(1).map(() => 0.5) })
}

/** The draft with a colour stop's colour, or an opacity stop's opacity. */
export function setStop(draft: GradientDraft, which: 'color' | 'opacity', index: number, value: string | number): GradientDraft {
  if (which === 'opacity') return { ...draft, opacity: draft.opacity.map((stop, at) => (at === index ? { ...stop, opacity: clamp(Number(value), 0, 100) } : stop)) }
  return { ...draft, stops: draft.stops.map((stop, at) => (at === index ? { ...stop, color: String(value) } : stop)) }
}

/** The draft with the midpoint of segment `segment` (in location order) at `fraction` of it. */
export function setMidpoint(draft: GradientDraft, segment: number, fraction: number): GradientDraft {
  return { ...draft, midpoints: draft.midpoints.map((mid, at) => (at === segment ? clamp(fraction, MIDPOINT_MIN, 1 - MIDPOINT_MIN) : mid)) }
}

/** Where each midpoint sits on the bar (0 to 1), in segment order. */
export function midpointPlaces(draft: GradientDraft): number[] {
  const order = sortedOrder(draft)
  return order.slice(1).map((index, segment) => {
    const a = draft.stops[order[segment]].at
    return a + (draft.stops[index].at - a) * (draft.midpoints[segment] ?? 0.5)
  })
}

/** The gradient as CSS (`linear-gradient(to right, …)`), sampled finely enough to show midpoints and opacity. */
export function cssGradient(draft: GradientDraft, foreground: string, background: string, reverse = false): string {
  const samples = 24
  const parts: string[] = []
  for (let step = 0; step <= samples; step += 1) {
    const t = step / samples
    const at = reverse ? 1 - t : t
    const [r, g, b] = toRgb(colorAt(draft, at, foreground, background))
    parts.push(`rgba(${r}, ${g}, ${b}, ${round(opacityAt(draft, at) / 100, 3)}) ${round(t * 100, 2)}%`)
  }
  return `linear-gradient(to right, ${parts.join(', ')})`
}

/**
 * The engine form for targets that keep no midpoints (the Gradient tool's current gradient, a
 * layer style's gradient): each bent segment gets a stop at its midpoint, of the half-and-half
 * colour, so the gradient looks as the editor showed it.
 */
export function bakeMidpoints(draft: GradientDraft, foreground: string, background: string): EngineGradient {
  const engine = toEngine(draft)
  const order = sortedOrder(draft)
  const stops: [number, string][] = []
  order.forEach((index, at) => {
    stops.push([round(draft.stops[index].at), draft.stops[index].color])
    const next = order[at + 1]
    const mid = draft.midpoints[at] ?? 0.5
    if (next === undefined || Math.abs(mid - 0.5) < 1e-6) return
    const a = draft.stops[index]
    const b = draft.stops[next]
    const place = a.at + (b.at - a.at) * mid
    const half = toHex(toRgb(resolveColor(a.color, foreground, background)).map((value, channel) => (value + toRgb(resolveColor(b.color, foreground, background))[channel]) / 2))
    stops.push([round(place), half])
  })
  return { stops, transparency: engine.transparency }
}
