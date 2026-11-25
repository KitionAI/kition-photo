import type { KitPointer, KitTool } from '@kition/sdk/editor'
import type { PhotoToolId } from '../catalog'
import { selectionModeFor, toolParams } from '../options'
import { NO_REPAINT, accentTint, clipToDocument, lineDrag, strokeArea, tracePolyline, type KitGesture, type PhotoToolEnv } from './common'
import { lockAxis } from './geometry'
import { picksColour } from '../altPicks'
import type { LivePreview } from './live'

/** Brush-like tools with no colour of their own show their reach in the accent at this opacity while painting. */
const REACH_ALPHA = 0.35
const ERASE_TINT = '#f3f3f6'

/** Tools that paint the foreground colour, and preview in it. */
const COLOURED = new Set(['Brush', 'Pencil', 'MixerBrush'])
/** Tools that sample from a source set with Alt-click. */
const SOURCED = new Set(['CloneStamp', 'Healing'])

/** What a tool adds to its stroke besides the brush options, or null when the press is not a stroke (Alt-click sets a source). */
function strokeExtras(id: string, env: PhotoToolEnv, start: KitPointer): Record<string, unknown> | null {
  if (COLOURED.has(id)) return { color: env.foreground }
  // The Eraser takes no colour: on a locked layer it paints the background colour, the engine's.
  if (id === 'Eraser') return { erase: true }
  if (!SOURCED.has(id)) return {}
  if (start.alt) {
    env.setCloneSource({ x: start.x, y: start.y })
    return null
  }
  if (!env.cloneSource) return null
  // The source keeps its offset from the brush's first point.
  return { offset: [Math.round(env.cloneSource.x - start.x), Math.round(env.cloneSource.y - start.y)] }
}

/**
 * Where the last brush stroke of the document ended, as [x, y, pressure] (Photoshop's Shift-click
 * draws a straight stroke from there): shared by every brush tool and layer; the editor forgets it
 * when another document opens (`forgetStrokeEnd`).
 */
let strokeEnd: number[] | null = null

export function forgetStrokeEnd() {
  strokeEnd = null
}


/**
 * The one factory every brush-like tool shares: it collects the stroke's points and runs
 * `command` with the tool's options (`toolParams`) and its own extras. Shift keeps a drag to the
 * axis it set out along (after 3 screen pixels) and a Shift-press starts with a straight line from
 * the last stroke's end; Alt picks the foreground colour on the tools that have no Alt of their own.
 */
export function brushTool(id: PhotoToolId, command: string, env: PhotoToolEnv): KitTool {
  return {
    id,
    cursor: 'crosshair',
    begin: (start, context) => {
      if (picksColour(id, start)) return eyedropperGesture(env, start)
      const { overlay, viewport } = context
      const size = env.options.size
      const extras = strokeExtras(id, env, start)
      if (!extras) return
      const from = start.shift && strokeEnd ? strokeEnd : null
      const points: number[][] = from ? [[from[0], from[1], from[2]], [start.x, start.y, start.pressure]] : [[start.x, start.y, start.pressure]]
      let last = from ? { x: from[0], y: from[1] } : { x: start.x, y: start.y }
      // Shift's lock: from where the drag (or Shift, pressed mid-drag) began.
      let origin: { x: number; y: number } | null = null
      let locked: 'h' | 'v' | null = null
      const segment = (to: { x: number; y: number }) =>
        overlay.add((canvas, toScreen) => {
          const a = toScreen(last.x, last.y)
          const b = toScreen(to.x, to.y)
          clipToDocument(canvas, toScreen, env.documentSize)
          canvas.lineCap = id === 'Pencil' ? 'square' : 'round'
          canvas.lineJoin = 'round'
          canvas.lineWidth = Math.max(1, size * viewport.view.zoom)
          canvas.strokeStyle = COLOURED.has(id) ? env.foreground : id === 'Eraser' ? ERASE_TINT : accentTint(REACH_ALPHA)
          canvas.beginPath()
          canvas.moveTo(a.x, a.y)
          canvas.lineTo(b.x, b.y)
          canvas.stroke()
        })
      segment({ x: start.x, y: start.y })
      last = { x: start.x, y: start.y }
      const place = (pointer: KitPointer, zoom: number) => {
        if (!pointer.shift) {
          origin = null
          locked = null
          return { x: pointer.x, y: pointer.y }
        }
        origin ??= { ...last }
        const held = lockAxis(origin, pointer, zoom, locked)
        locked = held.locked
        return held.point
      }
      return {
        move: (pointer, now) => {
          const at = place(pointer, now.viewport.view.zoom)
          points.push([at.x, at.y, pointer.pressure])
          segment(at)
          last = at
        },
        end: (pointer, now) => {
          // Release may be newer than the last move; a click must still produce only one dab.
          const at = place(pointer, now.viewport.view.zoom)
          if (at.x !== last.x || at.y !== last.y) points.push([at.x, at.y, pointer.pressure])
          strokeEnd = points[points.length - 1]
          // Effects on this layer, its group or a clipped layer can extend beyond the brush tip.
          const damage = env.hasLayerEffects ? null : strokeArea(points, size)
          void env.run(command, { points, ...toolParams(id, env.options), ...extras }, damage)
        },
      }
    },
  }
}

/** The temporary Eyedropper: the press and the drag after it set the foreground colour, sampled as the Eyedropper's options say. */
export function eyedropperGesture(env: PhotoToolEnv, start: KitPointer): KitGesture {
  let asked = 0
  const sample = (pointer: KitPointer) => {
    const mine = ++asked
    void env.query<number[]>('document.sampleColor', { x: Math.floor(pointer.x), y: Math.floor(pointer.y), ...toolParams('Eyedropper', env.options) }).then((rgba) => {
      if (rgba && mine === asked) env.pick([0, 1, 2].map((channel) => Math.round(rgba[channel] * 255)) as [number, number, number], false)
    })
  }
  sample(start)
  return { move: sample, end: sample }
}

/** A tool whose Alt-press picks the foreground colour (Paint Bucket, Gradient) and otherwise does its own. */
export function withAltPick(id: PhotoToolId, tool: KitTool, env: PhotoToolEnv): KitTool {
  return { ...tool, begin: (pointer, context) => (picksColour(id, pointer) ? eyedropperGesture(env, pointer) : tool.begin(pointer, context)) }
}

/** How often (ms) Quick Selection grows the selection during a drag. */
const QUICK_EVERY_MS = 120

/**
 * Quick Selection: brushes a selection on (Alt brushes it off). The selection grows while the
 * drag runs (the editor previews `select.quick` with the points so far, one history step for the
 * stroke); a click runs it once.
 */
export function quickSelectionTool(env: PhotoToolEnv): KitTool {
  return {
    id: 'QuickSelection',
    cursor: 'crosshair',
    begin: (start, { overlay, viewport }) => {
      const points: number[][] = [[Math.round(start.x), Math.round(start.y)]]
      const mode = selectionModeFor('QuickSelection', 'add', start)
      const params = () => ({ points: points.map((point) => [...point]), ...toolParams('QuickSelection', env.options), mode })
      let live: LivePreview | null = null
      let shownAt = 0
      const draw = () =>
        overlay.draw((context, toScreen) => {
          clipToDocument(context, toScreen, env.documentSize)
          tracePolyline(context, toScreen, points)
          context.lineCap = 'round'
          context.lineJoin = 'round'
          context.lineWidth = Math.max(1, env.options.size * viewport.view.zoom)
          context.strokeStyle = accentTint(REACH_ALPHA)
          context.stroke()
        })
      draw()
      return {
        move: (pointer) => {
          points.push([Math.round(pointer.x), Math.round(pointer.y)])
          draw()
          const now = Date.now()
          if (now - shownAt < QUICK_EVERY_MS) return
          shownAt = now
          live ??= env.preview('select.quick')
          live.push(params())
        },
        cancel: () => void live?.cancel(),
        end: (pointer) => {
          const x = Math.round(pointer.x), y = Math.round(pointer.y)
          const last = points[points.length - 1]
          if (x !== last[0] || y !== last[1]) points.push([x, y])
          if (live) void live.finish(params()).then(() => overlay.clear())
          else void env.run('select.quick', params(), NO_REPAINT)
        },
      }
    },
  }
}

/** A click that runs `command` at the pixel under the pointer (Paint Bucket, Magic Eraser). */
export function clickFill(id: string, command: string, params: () => Record<string, unknown>, env: PhotoToolEnv): KitTool {
  return {
    id,
    cursor: 'crosshair',
    begin: (pointer) => void env.run(command, { x: Math.floor(pointer.x), y: Math.floor(pointer.y), ...params() }),
  }
}

/**
 * The Gradient: a drag makes a Gradient Fill layer of the current gradient preset (upstream's live
 * "Gradient"), or with "Classic gradient" paints foreground to background into the pixels.
 */
export function gradientTool(env: PhotoToolEnv): KitTool {
  return lineDrag('Gradient', 'crosshair', (start, end) => {
    if (Math.hypot(end.x - start.x, end.y - start.y) < 1) return
    const ends = { from: [start.x, start.y], to: [end.x, end.y], ...toolParams('Gradient', env.options) }
    if (env.options.gradientClassic) void env.run('paint.gradient', { ...ends, colors: [env.foreground, env.background] })
    else void env.run('gradient.fill.create', ends)
  })
}
