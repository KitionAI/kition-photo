/**
 * Snapping, smart guides, ruler ticks and guide hits (plan 2026-11-24, stage 7), in document
 * pixels. Pure, so `guides.test.mjs` tests them without a browser.
 */
export type Guides = { horizontal: readonly number[]; vertical: readonly number[] }
export const NO_GUIDES: Guides = { horizontal: [], vertical: [] }
export type Box = { x0: number; y0: number; x1: number; y1: number }

/** Something to snap to on one axis: the line x = `at` (on the x axis; y = `at` on the y axis) running `from`–`to` across. */
export type SnapEdge = { at: number; from: number; to: number; kind: 'guide' | 'grid' | 'layer' | 'document' }
/** What a drag snaps to: the edges on each axis, and the grid's spacing (null: no grid). */
export type SnapSource = { x: readonly SnapEdge[]; y: readonly SnapEdge[]; grid: number | null }
/** A line a snap shows (smart guides): on the x axis a vertical line at x = `at` from y = `from` to `to`. */
export type SnapLine = { axis: 'x' | 'y'; at: number; from: number; to: number; kind: SnapEdge['kind'] }

export const NO_SNAP: SnapSource = { x: [], y: [], grid: null }

/** How near (screen pixels) a point or an edge comes before it snaps. */
export const SNAP_DISTANCE = 8

/**
 * What to snap to: the guides, the grid, other layers' edges and centres, and the document's
 * bounds and centre, as View › Snap To asks (`layers` are the boxes of the layers not being moved).
 */
export function snapSource(options: { guides: Guides; layers: readonly Box[]; width: number; height: number; grid: number | null; to: { guides: boolean; grid: boolean; layers: boolean; documentBounds: boolean } }): SnapSource {
  const x: SnapEdge[] = []
  const y: SnapEdge[] = []
  const { width, height, to } = options
  if (to.guides) {
    for (const at of options.guides.vertical) x.push({ at, from: 0, to: height, kind: 'guide' })
    for (const at of options.guides.horizontal) y.push({ at, from: 0, to: width, kind: 'guide' })
  }
  if (to.layers)
    for (const box of options.layers) {
      for (const at of [box.x0, (box.x0 + box.x1) / 2, box.x1]) x.push({ at, from: box.y0, to: box.y1, kind: 'layer' })
      for (const at of [box.y0, (box.y0 + box.y1) / 2, box.y1]) y.push({ at, from: box.x0, to: box.x1, kind: 'layer' })
    }
  if (to.documentBounds) {
    for (const at of [0, width / 2, width]) x.push({ at, from: 0, to: height, kind: 'document' })
    for (const at of [0, height / 2, height]) y.push({ at, from: 0, to: width, kind: 'document' })
  }
  return { x, y, grid: to.grid && options.grid && options.grid > 0 ? options.grid : null }
}

/** The nearest snap for any of `values` on one axis within `reach`: how far to move, and to what (null: none near). */
export function snapAxis(values: readonly number[], edges: readonly SnapEdge[], grid: number | null, reach: number): { delta: number; edge: SnapEdge | null; value: number } | null {
  let best: { delta: number; edge: SnapEdge | null; value: number } | null = null
  const consider = (delta: number, edge: SnapEdge | null, value: number) => {
    // Ties go to a guide, then a layer, before the grid (the engine's rule).
    if (Math.abs(delta) > reach) return
    if (!best || Math.abs(delta) < Math.abs(best.delta) - 1e-9 || (Math.abs(Math.abs(delta) - Math.abs(best.delta)) <= 1e-9 && rank(edge) < rank(best.edge))) best = { delta, edge, value }
  }
  for (const value of values) {
    for (const edge of edges) consider(edge.at - value, edge, value)
    if (grid) consider(Math.round(value / grid) * grid - value, null, value)
  }
  return best
}

const rank = (edge: SnapEdge | null) => (edge == null ? 3 : edge.kind === 'guide' ? 0 : edge.kind === 'layer' ? 1 : 2)

/** A point snapped on each axis (a marquee's or crop frame's corner, a handle). */
export function snapPoint(p: { x: number; y: number }, source: SnapSource, reach: number): { x: number; y: number; lines: SnapLine[] } {
  const sx = snapAxis([p.x], source.x, source.grid, reach)
  const sy = snapAxis([p.y], source.y, source.grid, reach)
  const x = p.x + (sx?.delta ?? 0)
  const y = p.y + (sy?.delta ?? 0)
  const lines: SnapLine[] = []
  if (sx?.edge) lines.push({ axis: 'x', at: sx.edge.at, from: Math.min(sx.edge.from, y), to: Math.max(sx.edge.to, y), kind: sx.edge.kind })
  if (sy?.edge) lines.push({ axis: 'y', at: sy.edge.at, from: Math.min(sy.edge.from, x), to: Math.max(sy.edge.to, x), kind: sy.edge.kind })
  return { x, y, lines }
}

/**
 * A box moved by (`dx`, `dy`), snapped: its edges or centre to the nearest edge on each axis (the
 * Move tool's drag, Free Transform's box), with the lines to show between the box and what it met.
 */
export function snapMove(box: Box, dx: number, dy: number, source: SnapSource, reach: number): { dx: number; dy: number; lines: SnapLine[] } {
  const sx = snapAxis([box.x0 + dx, (box.x0 + box.x1) / 2 + dx, box.x1 + dx], source.x, source.grid, reach)
  const sy = snapAxis([box.y0 + dy, (box.y0 + box.y1) / 2 + dy, box.y1 + dy], source.y, source.grid, reach)
  const nx = dx + (sx?.delta ?? 0)
  const ny = dy + (sy?.delta ?? 0)
  const lines: SnapLine[] = []
  if (sx?.edge) lines.push({ axis: 'x', at: sx.edge.at, from: Math.min(sx.edge.from, box.y0 + ny), to: Math.max(sx.edge.to, box.y1 + ny), kind: sx.edge.kind })
  if (sy?.edge) lines.push({ axis: 'y', at: sy.edge.at, from: Math.min(sy.edge.from, box.x0 + nx), to: Math.max(sy.edge.to, box.x1 + nx), kind: sy.edge.kind })
  return { dx: nx, dy: ny, lines }
}

/** The guide within `reach` of `p` (the nearer one when two are), or null. */
export function guideAt(guides: Guides, p: { x: number; y: number }, reach: number): { orientation: 'horizontal' | 'vertical'; index: number } | null {
  let best: { orientation: 'horizontal' | 'vertical'; index: number; distance: number } | null = null
  guides.vertical.forEach((at, index) => {
    const distance = Math.abs(at - p.x)
    if (distance <= reach && (!best || distance < best.distance)) best = { orientation: 'vertical', index, distance }
  })
  guides.horizontal.forEach((at, index) => {
    const distance = Math.abs(at - p.y)
    if (distance <= reach && (!best || distance < best.distance)) best = { orientation: 'horizontal', index, distance }
  })
  const found = best as { orientation: 'horizontal' | 'vertical'; index: number } | null
  return found ? { orientation: found.orientation, index: found.index } : null
}

/** A guide's position as it is dropped: a whole pixel, or, with Shift, the nearest of the ruler's ticks. */
export function guidePosition(at: number, zoom: number, shift: boolean): number {
  if (shift) {
    const { minor } = rulerStep(zoom)
    return Math.round(at / minor) * minor
  }
  return Math.round(at)
}

/** The ruler's numbered step (document pixels: 1, 2 or 5 times a power of ten, at least 50 screen pixels apart) and its small ticks' step. */
export function rulerStep(zoom: number): { major: number; minor: number } {
  const z = zoom > 0 ? zoom : 1
  let major = 1
  for (let power = 1; power < 1e7; power *= 10) {
    const step = [1, 2, 5].map((k) => k * power).find((candidate) => candidate * z >= 50)
    if (step) {
      major = step
      break
    }
  }
  // Small ticks between the numbered ones: at least 8 screen pixels apart, never under a pixel.
  const parts = [10, 5, 2, 1].find((count) => major % count === 0 && (major / count) * z >= 8) ?? 1
  return { major, minor: major / parts }
}

/** The ruler's ticks along a side `length` screen pixels long that starts at document coordinate `from`: numbered ticks with their label, and small ones, in screen pixels from the side's start. */
export function rulerTicks(from: number, zoom: number, length: number): { major: { at: number; label: string }[]; minor: number[] } {
  const z = zoom > 0 ? zoom : 1
  const { major, minor } = rulerStep(z)
  const to = from + length / z
  const out = { major: [] as { at: number; label: string }[], minor: [] as number[] }
  for (let value = Math.floor(from / minor) * minor; value <= to; value += minor) {
    const at = (value - from) * z
    if (at < 0) continue
    const onMajor = Math.abs(value / major - Math.round(value / major)) < 1e-6
    if (onMajor) out.major.push({ at, label: String(Math.round(value)) })
    else out.minor.push(at)
    if (out.major.length + out.minor.length > 2000) break
  }
  return out
}

/** Snapping as the tools reach it (`env.snap`): off when View › Snap is, or while Ctrl is held. */
export type Snapper = {
  /** A document point snapped at `zoom` (a corner, a handle), with the lines to show. */
  point: (p: { x: number; y: number }, zoom: number) => { x: number; y: number; lines: SnapLine[] }
  /** A box moved by (`dx`, `dy`) snapped at `zoom`, leaving layer `moving` out of the targets; with the lines to show. */
  move: (box: Box, dx: number, dy: number, zoom: number, moving?: number | null) => { dx: number; dy: number; lines: SnapLine[] }
  /** The gesture ended: its snap lines go. */
  done: () => void
}

/** Smart guides' colour (Photoshop's magenta). */
export const SMART_GUIDE = '#ff2bd6'

/** The lines a snap shows, in magenta, across the canvas at any zoom. */
export function drawSnapLines(context: CanvasRenderingContext2D, toScreen: (x: number, y: number) => { x: number; y: number }, lines: readonly SnapLine[]) {
  if (!lines.length) return
  context.save()
  context.setLineDash([])
  context.lineWidth = 1
  context.strokeStyle = SMART_GUIDE
  context.beginPath()
  for (const line of lines) {
    const a = line.axis === 'x' ? toScreen(line.at, line.from) : toScreen(line.from, line.at)
    const b = line.axis === 'x' ? toScreen(line.at, line.to) : toScreen(line.to, line.at)
    if (line.axis === 'x') {
      const x = Math.round(a.x) + 0.5
      context.moveTo(x, a.y)
      context.lineTo(x, b.y)
    } else {
      const y = Math.round(a.y) + 0.5
      context.moveTo(a.x, y)
      context.lineTo(b.x, y)
    }
  }
  context.stroke()
  context.restore()
}
