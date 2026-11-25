import type { KitPointer, KitTool } from '@kition/sdk/editor'
import type { PhotoToolId } from '../catalog'
import { fasteningSpacing, selectionModeFor, toolParams } from '../options'
import { NO_REPAINT, clickTool, dashed, tracePolyline, type KitGesture, type KitOverlay, type PhotoToolEnv } from './common'
import { withOutlineMove } from './select'

/**
 * A freehand selection outline from `start`: released, it selects the outline in the mode the
 * press's keys give (`feather` overrides the bar's). An incomplete outline preserves the selection
 * in add/subtract mode and clears it in replace/intersect mode.
 */
export function lassoGesture(id: PhotoToolId, env: PhotoToolEnv, start: KitPointer, overlay: KitOverlay, feather?: number): KitGesture {
  const points: number[][] = [[Math.round(start.x), Math.round(start.y)]]
  return {
    move: (pointer) => {
      points.push([Math.round(pointer.x), Math.round(pointer.y)])
      overlay.draw((context, toScreen) => {
        tracePolyline(context, toScreen, points)
        dashed(context)
      })
    },
    end: (pointer) => {
      const x = Math.round(pointer.x), y = Math.round(pointer.y)
      const last = points[points.length - 1]
      if (x !== last[0] || y !== last[1]) points.push([x, y])
      const mode = selectionModeFor(id, env.options.selectionMode, start)
      if (points.length < 3) {
        if (mode === 'replace' || mode === 'intersect') void env.run('select.deselect', {}, NO_REPAINT)
        else overlay.clear()
      } else void env.run('select.lasso', { points, mode, ...toolParams('Lasso', env.options), ...(feather === undefined ? {} : { feather }) }, NO_REPAINT)
    },
  }
}

/** The Lasso: a freehand outline combined according to the toolbar and press modifiers. */
export function lassoTool(env: PhotoToolEnv): KitTool {
  const tool: KitTool = {
    id: 'Lasso',
    cursor: 'crosshair',
    begin: (start, { overlay }) => lassoGesture('Lasso', env, start, overlay),
  }
  return withOutlineMove('Lasso', tool, env)
}

/**
 * The Polygonal Lasso (straight sides) and the Magnetic Lasso (its fastening points, clicked or,
 * as the pointer runs on, placed by Frequency, are joined along the image's edges by the engine):
 * click points, then click the first one, double-click, or press Enter. The modifiers of the first
 * click, else the options bar's mode, choose how it combines with the selection.
 */
export function clickLassoTool(id: 'PolygonLasso' | 'MagneticLasso', env: PhotoToolEnv): KitTool {
  return clickTool(id, env, {
    min: 3,
    point: (pointer) => [Math.round(pointer.x), Math.round(pointer.y)],
    // The outline so far, the rubber band to the pointer, and (fainter) the side that closes it.
    draw: (context, toScreen, points, hover) => {
      const open = hover ? [...points, [hover.x, hover.y]] : points
      tracePolyline(context, toScreen, open)
      dashed(context)
      if (open.length > 2) {
        tracePolyline(context, toScreen, [open[open.length - 1], open[0]])
        context.globalAlpha = 0.45
        dashed(context)
        context.globalAlpha = 1
      }
    },
    finish: (points, first) => env.run(id === 'PolygonLasso' ? 'select.lasso' : 'select.magneticLasso', { points, mode: selectionModeFor(id, env.options.selectionMode, first), ...toolParams(id, env.options) }, NO_REPAINT),
    closeOnDoubleClick: true,
    shiftSnaps: id === 'PolygonLasso',
    fastenAfter: id === 'MagneticLasso' ? () => fasteningSpacing(env.options.frequency) : undefined,
  })
}
