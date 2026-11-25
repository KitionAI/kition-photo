import type { KitTool } from '@kition/sdk/editor'
import { toolParams } from '../options'
import { NO_REPAINT, lineDrag, rectDrag, type PhotoToolEnv } from './common'
import { lassoGesture } from './lasso'

// Move lives in `move.ts` and Crop in `crop.ts` (plan 2026-11-24, stage 2); Crop is also exported here, where it was.
export { cropTool } from './crop'
export { moveTool } from './move'

/** Slice: the dragged area becomes a user slice. */
export function sliceTool(env: PhotoToolEnv): KitTool {
  return rectDrag('Slice', 'crosshair', (area) => {
    if (area.width < 1 || area.height < 1) return false
    void env.run('slice.new', { rect: [area.x, area.y, area.width, area.height] }, NO_REPAINT)
  })
}

type SliceList = { slices: { number: number; origin: string; rect: [number, number, number, number] }[] }

/** Slice Select: drags the slice under the press (the user's slices before the automatic ones). */
export function sliceSelectTool(env: PhotoToolEnv): KitTool {
  return lineDrag('SliceSelect', 'default', async (start, end) => {
    const dx = Math.round(end.x - start.x)
    const dy = Math.round(end.y - start.y)
    if (!dx && !dy) return
    const list = await env.query<SliceList>('slice.list')
    const under = (list?.slices ?? []).filter(({ rect: [x, y, width, height] }) => start.x >= x && start.x < x + width && start.y >= y && start.y < y + height)
    const slice = under.find((item) => item.origin === 'user') ?? under[0]
    if (!slice) return
    const [x, y, width, height] = slice.rect
    void env.run('slice.set', { number: slice.number, rect: [x + dx, y + dy, width, height] }, NO_REPAINT)
  })
}

/**
 * A patch drag's offset, limited so the dragged selection stays on the canvas, which the engine
 * requires (upstream's `patch_offset`).
 */
function patchOffset(env: PhotoToolEnv, dx: number, dy: number): [number, number] {
  const bounds = env.selection
  const { width, height } = env.documentSize
  if (!bounds) return [dx, dy]
  const x0 = Math.max(0, bounds[0])
  const y0 = Math.max(0, bounds[1])
  const x1 = Math.min(width, bounds[0] + bounds[2])
  const y1 = Math.min(height, bounds[1] + bounds[3])
  if (x1 <= x0 || y1 <= y0) return [dx, dy]
  const clamp = (d: number, lo: number, hi: number) => (lo > hi ? 0 : Math.min(hi, Math.max(lo, d)))
  return [clamp(dx, -x0, width - x1), clamp(dy, -y0, height - y1)]
}

/**
 * Patch and Smart Move: a press inside the selection drags it to where its content comes
 * from (Patch) or goes (Smart Move); any other press (none yet, outside it, or with Shift
 * or Alt) draws a freehand selection, as upstream's `patch_drags_selection`. Inside is judged by
 * the selection's bounds, the part of the selection the editor knows.
 */
export function selectionDragTool(id: 'Patch' | 'SmartMove', env: PhotoToolEnv): KitTool {
  const command = id === 'Patch' ? 'paint.patch' : 'paint.smartMove'
  const drag = lineDrag(id, 'crosshair', (start, end) => {
    const offset = patchOffset(env, Math.round(end.x - start.x), Math.round(end.y - start.y))
    if (offset[0] || offset[1]) void env.run(command, { offset, ...toolParams(id, env.options) })
  })
  return {
    ...drag,
    begin: (start, context) => {
      const bounds = env.selection
      const inside = bounds != null && start.x >= bounds[0] && start.x < bounds[0] + bounds[2] && start.y >= bounds[1] && start.y < bounds[1] + bounds[3]
      // The patch tools have no Feather of their own.
      return inside && !start.shift && !start.alt ? drag.begin(start, context) : lassoGesture(id, env, start, context.overlay, 0)
    },
  }
}
