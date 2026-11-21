import { dragRect, type KitPointer, type KitTool, type RasterRect } from '@kition/sdk/editor'
import type { ToolOptions } from '../options'

/** An image area that needs no repaint: the change touched no pixels (selections, paths, notes). */
export const NO_REPAINT: RasterRect = { x0: 0, y0: 0, x1: 0, y1: 0 }

export const SOFT_EDGE = 2
const ANT_DASH = [4, 4]
/** How near (screen pixels) a press must land to hit a point: the first anchor that closes a path, an anchor to drag. */
export const HIT_RADIUS = 6

export type Area = { x: number; y: number; width: number; height: number }
type ToScreen = (x: number, y: number) => { x: number; y: number }
/** The canvas's preview layer a tool draws on. */
export type KitOverlay = Parameters<KitTool['begin']>[1]['overlay']
/** One drag a tool's press starts. */
export type KitGesture = Exclude<ReturnType<KitTool['begin']>, void>

/** A tool that takes several clicks (Pen, Polygonal and Magnetic Lasso): Enter finishes it, Escape drops it. */
export type PendingClicks = { tool: string; finish: () => void; cancel: () => void }

/** What Photo's tools need from the editor. */
export type PhotoToolEnv = {
  /** The active tool's options-bar options. */
  options: ToolOptions
  foreground: string
  background: string
  /** The active layer's id, kind (`Pixel`, `Shape`, `Type`…), and bounds, for Move and Path Selection. */
  layer: { id: number | undefined; kind: string; bounds: RasterRect | null } | null
  /** Enabled layer styles can expand painted pixels, including effects on groups or clipped layers. */
  hasLayerEffects: boolean
  /** Runs an engine command; false when it failed (the editor has said why). */
  run: (command: string, params: Record<string, unknown>, damage?: RasterRect | null) => Promise<boolean>
  /** A read-only engine command, answered as JSON (null when it fails). */
  query: <R>(command: string, params?: Record<string, unknown>) => Promise<R | null>
  /** The selection's bounds as [x, y, width, height], or null without one: Patch and Smart Move drag it when the press is inside. */
  selection: readonly [number, number, number, number] | null
  /** The document's size, for Crop's Original Ratio. */
  documentSize: { width: number; height: number }
  /** The Eyedropper picked a colour: the foreground, or with Alt the background. */
  pick: (rgb: [number, number, number], background: boolean) => void
  /** The Crop tool proposes an area; the editor confirms it (Enter) or drops it (Escape). */
  proposeCrop: (area: Area) => void
  /** The Type tools ask for text at a document point. */
  beginText: (pointer: KitPointer, orientation: 'horizontal' | 'vertical') => void
  /** The Clone Stamp's and Healing Brush's source, set with Alt-click; null until then. */
  cloneSource: { x: number; y: number } | null
  setCloneSource: (source: { x: number; y: number } | null) => void
  /** The several-click tool in progress, which the editor's Enter and Escape reach. */
  pending: { current: PendingClicks | null }
  /** The Move tool says a drag started or ended (the transform box hides meanwhile). */
  moving: (moving: boolean) => void
}

export function rectArea(area: Area, margin = 0): RasterRect {
  return { x0: area.x - margin, y0: area.y - margin, x1: area.x + area.width + margin, y1: area.y + area.height + margin }
}

export function strokeArea(points: readonly number[][], size: number): RasterRect {
  if (!points.length) return NO_REPAINT
  const reach = size / 2 + SOFT_EDGE
  let x0 = Infinity, y0 = Infinity, x1 = -Infinity, y1 = -Infinity
  // Coalesced input can contain more points than a function call accepts as arguments.
  for (const [x, y] of points) {
    x0 = Math.min(x0, x)
    y0 = Math.min(y0, y)
    x1 = Math.max(x1, x)
    y1 = Math.max(y1, y)
  }
  return {
    x0: Math.floor(x0 - reach),
    y0: Math.floor(y0 - reach),
    x1: Math.ceil(x1 + reach),
    y1: Math.ceil(y1 + reach),
  }
}

/** Clips what follows to the document, so a paint preview never shows past the canvas edge. */
export function clipToDocument(context: CanvasRenderingContext2D, toScreen: ToScreen, documentSize: { width: number; height: number }) {
  const d0 = toScreen(0, 0)
  const d1 = toScreen(documentSize.width, documentSize.height)
  context.beginPath()
  context.rect(d0.x, d0.y, d1.x - d0.x, d1.y - d0.y)
  context.clip()
}

/** Strokes the current path as marching ants (white under black dashes). */
export function dashed(context: CanvasRenderingContext2D) {
  context.lineWidth = 1
  context.strokeStyle = '#ffffff'
  context.setLineDash([])
  context.stroke()
  context.strokeStyle = '#111111'
  context.setLineDash(ANT_DASH)
  context.stroke()
}

/** A polyline through document points, optionally closed. */
export function tracePolyline(context: CanvasRenderingContext2D, toScreen: ToScreen, points: readonly (readonly number[])[], closed = false) {
  context.beginPath()
  points.forEach(([x, y], index) => {
    const p = toScreen(x, y)
    if (index === 0) context.moveTo(p.x, p.y)
    else context.lineTo(p.x, p.y)
  })
  if (closed) context.closePath()
}

/** Previews a straight drag from `start`: the moved distance, for tools that act on release. */
export function lineDrag(id: string, cursor: string, done: (start: KitPointer, end: KitPointer) => void): KitTool {
  return {
    id,
    cursor,
    begin: (start, { overlay }) => {
      let end = start
      return {
        move: (pointer) => {
          end = pointer
          overlay.draw((context, toScreen) => {
            tracePolyline(context, toScreen, [
              [start.x, start.y],
              [end.x, end.y],
            ])
            dashed(context)
          })
        },
        end: (pointer) => done(start, pointer),
      }
    },
  }
}

/**
 * A drag that previews a rectangle (or `outline`) and hands the final area to `done`. Shift squares
 * it when `square`; `area` shapes it instead (a fixed ratio or size). Returning false from `done`
 * clears the preview when no document command or persistent preview takes over.
 */
export function rectDrag(
  id: string,
  cursor: string,
  done: (area: Area, start: KitPointer, end: KitPointer) => void | false,
  options: { square?: boolean; area?: (start: KitPointer, pointer: KitPointer) => Area; outline?: (context: CanvasRenderingContext2D, a: { x: number; y: number }, b: { x: number; y: number }) => void; fill?: () => string | null } = {},
): KitTool {
  return {
    id,
    cursor,
    begin: (start, { overlay }) => {
      const areaAt = (pointer: KitPointer) => options.area ? options.area(start, pointer) : dragRect(start, pointer, pointer.shift && Boolean(options.square))
      return {
        move: (pointer) => {
          const area = areaAt(pointer)
          overlay.draw((context, toScreen) => {
            const a = toScreen(area.x, area.y)
            const b = toScreen(area.x + area.width, area.y + area.height)
            context.beginPath()
            if (options.outline) options.outline(context, a, b)
            else context.rect(a.x + 0.5, a.y + 0.5, b.x - a.x, b.y - a.y)
            const fill = options.fill?.()
            if (fill) {
              context.globalAlpha = 0.6
              context.fillStyle = fill
              context.fill()
              context.globalAlpha = 1
            }
            dashed(context)
          })
        },
        // A fixed-size selection also works on a click; release may have no preceding move.
        end: (pointer) => {
          if (done(areaAt(pointer), start, pointer) === false) overlay.clear()
        },
      }
    },
  }
}

/** An ellipse inscribed in the screen box `a`–`b`. */
export function ellipseOutline(context: CanvasRenderingContext2D, a: { x: number; y: number }, b: { x: number; y: number }) {
  context.ellipse((a.x + b.x) / 2, (a.y + b.y) / 2, Math.abs(b.x - a.x) / 2, Math.abs(b.y - a.y) / 2, 0, 0, Math.PI * 2)
}

/**
 * A tool built from clicks (Polygonal and Magnetic Lasso, Pen): each press adds a point; a press
 * on the first point (once there are `min`) closes it, Enter finishes it open, and Escape drops it.
 * With `closeOnDoubleClick` (the lassos, as upstream; not the Pen) a double click closes it too.
 * The points live in `env.pending`, so they outlive the tool object the editor rebuilds when an
 * option changes. `draw` previews the points and the rubber band to the pointer.
 */
export function clickTool<P extends number[]>(
  id: string,
  env: PhotoToolEnv,
  options: {
    min: number
    point: (pointer: KitPointer) => P
    draw: (context: CanvasRenderingContext2D, toScreen: ToScreen, points: readonly P[], hover: KitPointer | null) => void
    /** Runs the tool's command: false when it failed, which drops the preview. */
    finish: (points: P[], first: KitPointer, closed: boolean) => Promise<boolean>
    closeOnDoubleClick?: boolean
    /** A drag after a press (the Pen pulls handles): changes the last point. */
    drag?: (last: P, pointer: KitPointer) => void
    /** How far (screen pixels) the pointer runs from the last point before it fastens one by itself (the Magnetic Lasso's Frequency). */
    fastenAfter?: () => number
  },
): KitTool {
  type State = PendingClicks & { points: P[]; first: KitPointer; overlay: KitOverlay }
  const current = () => (env.pending.current?.tool === id ? (env.pending.current as State) : null)
  const redraw = (state: State, hover: KitPointer | null) => state.overlay.draw((context, toScreen) => options.draw(context, toScreen, state.points, hover))
  const finish = (closed: boolean) => {
    const state = current()
    if (!state) return
    env.pending.current = null
    // A double click reaches `begin` twice first: drop the repeated points.
    const points = state.points.filter((point, index) => index === 0 || point[0] !== state.points[index - 1][0] || point[1] !== state.points[index - 1][1])
    if (points.length < options.min) state.overlay.clear()
    else
      void options.finish(points, state.first, closed).then((ran) => {
        // A command that ran repaints, which clears the preview; one that failed leaves it to us,
        // unless a new run of clicks has started drawing its own.
        if (!ran && !env.pending.current) state.overlay.clear()
      })
  }
  return {
    id,
    cursor: 'crosshair',
    begin: (pointer, { overlay, viewport }) => {
      let state = current()
      if (!state) {
        const created: State = {
          tool: id,
          points: [],
          first: pointer,
          overlay,
          finish: () => finish(false),
          cancel: () => {
            env.pending.current = null
            overlay.clear()
          },
        }
        env.pending.current = created
        state = created
      }
      const first = state.points[0]
      const near = first && Math.hypot(first[0] - pointer.x, first[1] - pointer.y) * viewport.view.zoom < HIT_RADIUS
      if (near && state.points.length >= options.min) {
        finish(true)
        return
      }
      state.points.push(options.point(pointer))
      redraw(state, null)
      const drag = options.drag
      if (!drag) return
      const last = state.points[state.points.length - 1]
      const held = state
      const move = (moved: KitPointer) => {
        drag(last, moved)
        redraw(held, null)
      }
      return { move, end: move }
    },
    hover: (pointer, { viewport }) => {
      const state = current()
      if (!state) return
      const last = state.points[state.points.length - 1]
      if (options.fastenAfter && last && Math.hypot(last[0] - pointer.x, last[1] - pointer.y) * viewport.view.zoom >= options.fastenAfter()) state.points.push(options.point(pointer))
      redraw(state, pointer)
    },
    doubleClick: options.closeOnDoubleClick ? () => finish(true) : undefined,
  }
}

/** Marching ants for selection segments `[ax, ay, bx, by, …]` in document pixels. */
export function drawSelectionAnts(segments: Int32Array, context: CanvasRenderingContext2D, toScreen: ToScreen) {
  context.beginPath()
  for (let index = 0; index + 3 < segments.length; index += 4) {
    const a = toScreen(segments[index], segments[index + 1])
    const b = toScreen(segments[index + 2], segments[index + 3])
    context.moveTo(Math.round(a.x) + 0.5, Math.round(a.y) + 0.5)
    context.lineTo(Math.round(b.x) + 0.5, Math.round(b.y) + 0.5)
  }
  context.lineWidth = 1
  context.strokeStyle = '#ffffff'
  context.stroke()
  context.strokeStyle = '#111111'
  context.setLineDash(ANT_DASH)
  context.lineDashOffset = -((Date.now() / 80) % 8)
  context.stroke()
}

/** Kition Photo's brand colour (`ui.brand.light` in `kit.json`), when the host set no `--kit-primary`. */
const BRAND = '#087fa1'

/** The theme's accent colour (the kit's brand), for marks drawn on the canvas. */
export function accent() {
  return getComputedStyle(document.documentElement).getPropertyValue('--kit-primary').trim() || BRAND
}

/** The accent colour at `alpha` opacity (the brand's when the accent is not a `#rrggbb` colour). */
export function accentTint(alpha: number) {
  const hex = /^#([0-9a-f]{6})$/i.exec(accent())?.[1] ?? BRAND.slice(1)
  const value = Number.parseInt(hex, 16)
  return `rgba(${(value >> 16) & 255}, ${(value >> 8) & 255}, ${value & 255}, ${alpha})`
}

/** The proposed crop: the rest of the image dimmed, the kept area outlined, with the rule-of-thirds lines when `thirds`. */
export function drawCropProposal(area: Area, context: CanvasRenderingContext2D, toScreen: ToScreen, documentSize: { width: number; height: number }, thirds: boolean) {
  const a = toScreen(area.x, area.y)
  const b = toScreen(area.x + area.width, area.y + area.height)
  const d0 = toScreen(0, 0)
  const d1 = toScreen(documentSize.width, documentSize.height)
  context.fillStyle = 'rgba(15, 15, 30, 0.45)'
  context.beginPath()
  context.rect(d0.x, d0.y, d1.x - d0.x, d1.y - d0.y)
  context.rect(a.x, a.y, b.x - a.x, b.y - a.y)
  context.fill('evenodd')
  context.strokeStyle = '#ffffff'
  context.lineWidth = 1
  context.strokeRect(a.x + 0.5, a.y + 0.5, b.x - a.x, b.y - a.y)
  if (!thirds) return
  context.globalAlpha = 0.6
  context.beginPath()
  for (const k of [1 / 3, 2 / 3]) {
    const x = Math.round(a.x + (b.x - a.x) * k) + 0.5
    const y = Math.round(a.y + (b.y - a.y) * k) + 0.5
    context.moveTo(x, a.y)
    context.lineTo(x, b.y)
    context.moveTo(a.x, y)
    context.lineTo(b.x, y)
  }
  context.stroke()
  context.globalAlpha = 1
}

/**
 * The Move tool's transform box (Show Transform Controls): the layer's bounds in the accent colour.
 * Upstream's handles start Free Transform, a later wave's canvas transform, so none are drawn yet.
 */
export function drawTransformBox(bounds: RasterRect, context: CanvasRenderingContext2D, toScreen: ToScreen) {
  const a = toScreen(bounds.x0, bounds.y0)
  const b = toScreen(bounds.x1, bounds.y1)
  context.setLineDash([])
  context.lineWidth = 1
  context.strokeStyle = accent()
  context.strokeRect(Math.round(a.x) + 0.5, Math.round(a.y) + 0.5, Math.round(b.x - a.x), Math.round(b.y - a.y))
}

/** A path's knots as the engine's `path.info` gives them. */
export type PathKnot = { anchor: [number, number]; in: [number, number]; out: [number, number]; smooth?: boolean }
export type PathData = { subpaths: { closed: boolean; knots: PathKnot[] }[] }

/** Strokes a path's Bézier segments (the work path, the Pen's path in progress). */
export function tracePath(context: CanvasRenderingContext2D, toScreen: ToScreen, path: PathData) {
  context.beginPath()
  for (const subpath of path.subpaths) {
    const { knots } = subpath
    if (!knots.length) continue
    const start = toScreen(...knots[0].anchor)
    context.moveTo(start.x, start.y)
    const segments = subpath.closed ? knots.length : knots.length - 1
    for (let index = 0; index < segments; index += 1) {
      const a = knots[index]
      const b = knots[(index + 1) % knots.length]
      const c0 = toScreen(...a.out)
      const c1 = toScreen(...b.in)
      const end = toScreen(...b.anchor)
      context.bezierCurveTo(c0.x, c0.y, c1.x, c1.y, end.x, end.y)
    }
    if (subpath.closed) context.closePath()
  }
}

/** The work path's outline over the image, in the accent tint over a dark halo (as upstream). */
export function drawPathOutline(path: PathData, context: CanvasRenderingContext2D, toScreen: ToScreen) {
  tracePath(context, toScreen, path)
  context.setLineDash([])
  context.lineWidth = 3
  context.strokeStyle = 'rgba(0, 0, 0, 0.45)'
  context.stroke()
  context.lineWidth = 1
  context.strokeStyle = accent()
  context.stroke()
}
