import type { KitPointer, KitTool } from '@kition/sdk/editor'
import type { PhotoToolId } from '../catalog'
import { selectionModeFor, toolParams } from '../options'
import { NO_REPAINT, accentTint, clipToDocument, lineDrag, strokeArea, tracePolyline, type PhotoToolEnv } from './common'

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
 * The one factory every brush-like tool shares: it collects the stroke's points and runs
 * `command` with the tool's options (`toolParams`) and its own extras.
 */
export function brushTool(id: PhotoToolId, command: string, env: PhotoToolEnv): KitTool {
  return {
    id,
    cursor: 'crosshair',
    begin: (start, { overlay, viewport }) => {
      const size = env.options.size
      const extras = strokeExtras(id, env, start)
      if (!extras) return
      const points: number[][] = [[start.x, start.y, start.pressure]]
      let last = start
      const segment = (to: KitPointer) =>
        overlay.add((context, toScreen) => {
          const from = toScreen(last.x, last.y)
          const next = toScreen(to.x, to.y)
          clipToDocument(context, toScreen, env.documentSize)
          context.lineCap = id === 'Pencil' ? 'square' : 'round'
          context.lineJoin = 'round'
          context.lineWidth = Math.max(1, size * viewport.view.zoom)
          context.strokeStyle = COLOURED.has(id) ? env.foreground : id === 'Eraser' ? ERASE_TINT : accentTint(REACH_ALPHA)
          context.beginPath()
          context.moveTo(from.x, from.y)
          context.lineTo(next.x, next.y)
          context.stroke()
        })
      segment(start)
      return {
        move: (pointer) => {
          points.push([pointer.x, pointer.y, pointer.pressure])
          segment(pointer)
          last = pointer
        },
        end: (pointer) => {
          // Release may be newer than the last move; a click must still produce only one dab.
          if (pointer.x !== last.x || pointer.y !== last.y) points.push([pointer.x, pointer.y, pointer.pressure])
          // Effects on this layer, its group or a clipped layer can extend beyond the brush tip.
          const damage = env.hasLayerEffects ? null : strokeArea(points, size)
          void env.run(command, { points, ...toolParams(id, env.options), ...extras }, damage)
        },
      }
    },
  }
}

/** Quick Selection: brushes a selection on (Alt brushes it off). */
export function quickSelectionTool(env: PhotoToolEnv): KitTool {
  return {
    id: 'QuickSelection',
    cursor: 'crosshair',
    begin: (start, { overlay, viewport }) => {
      const points: number[][] = [[Math.round(start.x), Math.round(start.y)]]
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
        },
        end: (pointer) => {
          const x = Math.round(pointer.x), y = Math.round(pointer.y)
          const last = points[points.length - 1]
          if (x !== last[0] || y !== last[1]) points.push([x, y])
          void env.run('select.quick', { points, ...toolParams('QuickSelection', env.options), mode: selectionModeFor('QuickSelection', 'add', start) }, NO_REPAINT)
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
