import { dragRect, type KitPointer, type KitTool, type RasterRect } from '@kition/sdk/editor'
import type { ToolOptions } from '../options'
import { CORNERS, EDGES, HANDLE_AT, cropQuad, quadPoint, snap45, type CropBox, type Point, type Quad } from './geometry'
import type { LivePreview } from './live'
import type { LiveStroke } from './stroke'

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
/** The canvas's view a tool draws through. */
export type KitViewport = Parameters<KitTool['begin']>[1]['viewport']
/** One drag a tool's press starts. */
export type KitGesture = Exclude<ReturnType<KitTool['begin']>, void>

/** A tool that takes several clicks (Pen, Polygonal and Magnetic Lasso): Enter finishes it, Escape drops it, Backspace takes its last point back. */
export type PendingClicks = { tool: string; finish: () => void; cancel: () => void; back?: () => void }

/** Free Transform's box (plan 2026-11-24, stage 2): the source rectangle [x0, y0, x1, y1], where its corners go, and the reference point. */
export type TransformBox = { layer: number; rect: [number, number, number, number]; quad: Quad; reference: Point; mode: 'free' | 'skew' | 'distort' }

/** Free Transform as the tools reach it: the open box (null when none), a drag's new box, and how to open one. */
export type TransformControl = {
  readonly box: TransformBox | null
  /** Opens the box on the active layer (or its selected part); false, saying why, when it cannot. */
  start: () => boolean
  /** The box during a drag: drawn and previewed at once; `settled` on release (the options bar follows). */
  update: (quad: Quad, reference: Point, settled: boolean) => void
}

/** The Crop tool's frame (`initial`: the whole canvas, not yet touched, which a drag inside replaces). */
export type CropFrame = CropBox & { initial: boolean }

/** The Crop tool's frame as the tool reaches it. */
export type CropControl = {
  readonly frame: CropFrame | null
  /** A drag's new frame: drawn at once; `settled` on release. */
  update: (frame: CropFrame | null, settled: boolean) => void
  commit: () => void
}

/** What Photo's tools need from the editor. */
export type PhotoToolEnv = {
  /** The active tool's options-bar options. */
  options: ToolOptions
  foreground: string
  background: string
  /** The active layer's id, kind (`Pixel`, `Shape`, `Type`…), and bounds, for Move and Path Selection; `locked` when its position may not move (the Background, a position or full lock). */
  layer: { id: number | undefined; kind: string; bounds: RasterRect | null; locked: boolean; background: boolean; opacity: number } | null
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
  /** The Crop tool proposes a new frame (a drawn area, unturned). */
  proposeCrop: (area: Area) => void
  /** The Crop tool's frame; the editor commits it (Enter, a double click) or drops it (Escape). */
  crop: CropControl
  /** Free Transform's box. */
  transform: TransformControl
  /** A command's result shown while a gesture is under way, one history step for the whole gesture. */
  preview: (command: string) => LivePreview
  /** A cursor for now (a handle's arrows), or null for the tool's own. */
  setCursor: (cursor: string | null) => void
  /** The selection's outline as the canvas traced it (segments in document pixels), or null. */
  selectionOutline: Int32Array | null
  /** Hides the selection's marching ants while a gesture draws them moved. */
  hideSelection: (hidden: boolean) => void
  /** Tells the person something (Photo's string `key`), as a toast. */
  say: (key: string) => void
  /** Redraws the canvas's marks (the transform box, the crop frame) after a change to them. */
  redraw: () => void
  /** The layer's own pixels on the canvas (the Layers panel's thumbnail, up to 1024 pixels on its longer side), for the Move tool's drag. */
  layerImage: (layer: number) => Promise<ImageBitmap | null>
  /** The Type tools ask for text at a document point. */
  beginText: (pointer: KitPointer, orientation: 'horizontal' | 'vertical') => void
  /** The Clone Stamp's and Healing Brush's source, set with Alt-click; null until then. */
  cloneSource: { x: number; y: number } | null
  setCloneSource: (source: { x: number; y: number } | null) => void
  /** The several-click tool in progress, which the editor's Enter and Escape reach. */
  pending: { current: PendingClicks | null }
  /** The Move tool says a drag started or ended (the transform box hides meanwhile). */
  moving: (moving: boolean) => void
  /**
   * A stroke the engine paints while it is drawn (plan 2026-11-24, stage 3), begun with the
   * command's params and the points so far; `failed` is called when it shows no preview after all,
   * so the tool draws its own line. Absent where there is no engine: strokes paint on release.
   */
  liveStroke?: (command: string, params: Record<string, unknown>, viewport: KitViewport, failed: () => void) => LiveStroke | null
  /**
   * Leaves `layer` out of the canvas until the document next changes (the Move tool's drag shows
   * the layer only at the pointer); null puts it back. `area` is drawn again (the whole view when null).
   */
  omitLayer?: (layer: number | null, viewport: KitViewport, area: RasterRect | null) => void
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
  options: { square?: boolean; area?: (start: KitPointer, pointer: KitPointer) => Area; outline?: (context: CanvasRenderingContext2D, a: { x: number; y: number }, b: { x: number; y: number }) => void; fill?: () => string | null; readout?: boolean } = {},
): KitTool {
  return {
    id,
    cursor,
    begin: (pressed, { overlay }) => {
      // Space held mid-drag moves the box: the press point travels with it.
      let start = pressed
      const areaAt = (pointer: KitPointer) => options.area ? options.area(start, pointer) : dragRect(start, pointer, pointer.shift && Boolean(options.square))
      let last = pressed
      const show = (pointer: KitPointer) => {
        last = pointer
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
          if (options.readout) drawReadout(context, `${area.width} × ${area.height} px`, { x: Math.max(a.x, b.x), y: Math.max(a.y, b.y) })
        })
      }
      const gesture: KitGesture & { spaceMove: (dx: number, dy: number) => void } = {
        move: show,
        spaceMove: (dx, dy) => {
          start = { ...start, x: start.x + dx, y: start.y + dy }
          last = { ...last, x: last.x + dx, y: last.y + dy }
        },
        // A fixed-size selection also works on a click; release may have no preceding move.
        end: (pointer) => {
          if (done(areaAt(pointer), start, pointer) === false) overlay.clear()
        },
      }
      return gesture
    },
  }
}

/** A small label by the pointer (a marquee's size), on a dark chip. */
export function drawReadout(context: CanvasRenderingContext2D, text: string, at: { x: number; y: number }) {
  context.save()
  context.setLineDash([])
  context.font = '11px system-ui, sans-serif'
  const width = context.measureText(text).width + 10
  const x = at.x + 10
  const y = at.y + 10
  context.fillStyle = 'rgba(20, 20, 24, 0.82)'
  context.fillRect(x, y, width, 18)
  context.fillStyle = '#ffffff'
  context.textBaseline = 'middle'
  context.fillText(text, x + 5, y + 9)
  context.restore()
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
    /** Shift holds a new point (and the rubber band) to 45° from the last one (the lassos). */
    shiftSnaps?: boolean
  },
): KitTool {
  /** The pointer, held to 45° from the last point when Shift asks it. */
  const snapped = (pointer: KitPointer, points: readonly P[]): KitPointer => {
    const last = points[points.length - 1]
    if (!options.shiftSnaps || !pointer.shift || !last) return pointer
    const at = snap45({ x: last[0], y: last[1] }, pointer)
    return { ...pointer, x: at.x, y: at.y }
  }
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
          back: () => {
            created.points.pop()
            if (created.points.length) redraw(created, null)
            else created.cancel()
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
      state.points.push(options.point(snapped(pointer, state.points)))
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
      redraw(state, snapped(pointer, state.points))
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

/** The handles' side, in screen pixels. */
const HANDLE_SIDE = 7

/** Eight square handles on a box's corners and edges (white, outlined). */
function drawHandles(context: CanvasRenderingContext2D, screen: Quad) {
  context.setLineDash([])
  context.lineWidth = 1
  for (const handle of [...CORNERS, ...EDGES]) {
    const [s, t] = HANDLE_AT[handle]
    const p = quadPoint(screen, s, t)
    const x = Math.round(p.x - HANDLE_SIDE / 2) + 0.5
    const y = Math.round(p.y - HANDLE_SIDE / 2) + 0.5
    context.fillStyle = '#ffffff'
    context.fillRect(x, y, HANDLE_SIDE, HANDLE_SIDE)
    context.strokeStyle = '#1b1b1f'
    context.strokeRect(x, y, HANDLE_SIDE, HANDLE_SIDE)
  }
}

function traceQuad(context: CanvasRenderingContext2D, screen: Quad) {
  context.beginPath()
  context.moveTo(screen[0].x, screen[0].y)
  for (let index = 1; index < 4; index += 1) context.lineTo(screen[index].x, screen[index].y)
  context.closePath()
}

/**
 * The Crop tool's frame: the rest of the image dimmed, the kept area outlined with eight handles,
 * turned with the frame, with the rule-of-thirds lines when `thirds`.
 */
export function drawCropProposal(frame: CropBox, context: CanvasRenderingContext2D, toScreen: ToScreen, documentSize: { width: number; height: number }, thirds: boolean) {
  const screen = cropQuad(frame).map((p) => toScreen(p.x, p.y)) as Quad
  const d0 = toScreen(0, 0)
  const d1 = toScreen(documentSize.width, documentSize.height)
  const margin = 4000
  context.fillStyle = 'rgba(15, 15, 30, 0.45)'
  context.beginPath()
  context.rect(Math.min(d0.x, screen[0].x) - margin, Math.min(d0.y, screen[0].y) - margin, Math.abs(d1.x - d0.x) + margin * 3, Math.abs(d1.y - d0.y) + margin * 3)
  context.moveTo(screen[0].x, screen[0].y)
  for (let index = 3; index >= 1; index -= 1) context.lineTo(screen[index].x, screen[index].y)
  context.closePath()
  context.fill('evenodd')
  traceQuad(context, screen)
  context.strokeStyle = '#ffffff'
  context.lineWidth = 1
  context.setLineDash([])
  context.stroke()
  if (thirds) {
    context.globalAlpha = 0.6
    context.beginPath()
    for (const k of [1 / 3, 2 / 3]) {
      const a = quadPoint(screen, k, 0), b = quadPoint(screen, k, 1)
      const c = quadPoint(screen, 0, k), d = quadPoint(screen, 1, k)
      context.moveTo(a.x, a.y)
      context.lineTo(b.x, b.y)
      context.moveTo(c.x, c.y)
      context.lineTo(d.x, d.y)
    }
    context.stroke()
    context.globalAlpha = 1
  }
  drawHandles(context, screen)
}

/**
 * A layer's box on the canvas in the accent colour: Free Transform's and Show Transform Controls'
 * with eight handles (and the reference point when given), or, with neither, a thin dotted mark of
 * where the selected layer is (the Move tool's).
 */
export function drawTransformBox(quad: Quad, context: CanvasRenderingContext2D, toScreen: ToScreen, handles: boolean, reference: Point | null = null) {
  const screen = quad.map((p) => toScreen(p.x, p.y)) as Quad
  const round = screen.map((p) => ({ x: Math.round(p.x) + 0.5, y: Math.round(p.y) + 0.5 })) as Quad
  traceQuad(context, round)
  context.lineWidth = 1
  if (handles) {
    context.setLineDash([])
    context.strokeStyle = accent()
    context.stroke()
    drawHandles(context, screen)
  } else {
    context.strokeStyle = 'rgba(255, 255, 255, 0.7)'
    context.setLineDash([])
    context.stroke()
    context.strokeStyle = accent()
    context.setLineDash([2, 3])
    context.stroke()
    context.setLineDash([])
  }
  if (reference) {
    const p = toScreen(reference.x, reference.y)
    context.beginPath()
    context.arc(p.x, p.y, 4.5, 0, Math.PI * 2)
    context.moveTo(p.x - 7, p.y)
    context.lineTo(p.x + 7, p.y)
    context.moveTo(p.x, p.y - 7)
    context.lineTo(p.x, p.y + 7)
    context.strokeStyle = '#ffffff'
    context.lineWidth = 3
    context.stroke()
    context.strokeStyle = '#1b1b1f'
    context.lineWidth = 1
    context.stroke()
  }
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
