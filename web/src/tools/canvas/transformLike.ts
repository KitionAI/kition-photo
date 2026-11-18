import { dragRect, type KitPointer, type KitTool } from '@kition/kit-editor'
import { cropRatioSize, marqueeEnd, pixelArea, toolParams } from '../options'
import { NO_REPAINT, dashed, lineDrag, rectDrag, type PhotoToolEnv } from './common'
import { lassoGesture } from './lasso'

/**
 * Auto-Select (or Cmd-click while it is off) picks the layer under the press first, as upstream:
 * the answer is the layer it picked, or null when the active layer stayed (its bounds still hold
 * for the preview).
 */
async function autoSelect(env: PhotoToolEnv, pointer: KitPointer) {
  const o = env.options
  if (o.autoSelect === pointer.meta) return null
  const at = { x: Math.floor(pointer.x), y: Math.floor(pointer.y), target: o.autoSelectTarget }
  const under = await env.query<{ layer: number | null }>('layer.pickAt', { ...at, select: false })
  if (under?.layer == null || under.layer === env.layer?.id) return null
  await env.run('layer.pickAt', { ...at, mode: pointer.shift ? 'add' : 'replace' }, NO_REPAINT)
  return under.layer
}

/**
 * Move: drags the active layer, previewing its bounds. Auto-Select picks the one under the press
 * first and the same drag moves it (the tool reads the active layer when it acts, so the change
 * does not rebuild it mid-drag); the preview takes the picked layer's bounds once they arrive.
 */
export function moveTool(env: PhotoToolEnv): KitTool {
  return {
    id: 'Move',
    cursor: 'move',
    begin: (start, { overlay }) => {
      env.moving(true)
      const picked = autoSelect(env, start)
      let bounds = env.layer?.bounds ?? null
      let pickedLayer: number | null = null
      void picked.then((layer) => {
        if (layer == null) return
        pickedLayer = layer
        bounds = null
      })
      let delta = { x: 0, y: 0 }
      return {
        move: (pointer) => {
          delta = { x: Math.round(pointer.x - start.x), y: Math.round(pointer.y - start.y) }
          const active = env.layer
          if (!bounds && pickedLayer != null && active?.id === pickedLayer) bounds = active.bounds
          const box = bounds
          if (!box) return
          overlay.draw((context, toScreen) => {
            const a = toScreen(box.x0 + delta.x, box.y0 + delta.y)
            const b = toScreen(box.x1 + delta.x, box.y1 + delta.y)
            context.beginPath()
            context.rect(a.x + 0.5, a.y + 0.5, b.x - a.x, b.y - a.y)
            dashed(context)
          })
        },
        cancel: () => env.moving(false),
        end: async (pointer) => {
          delta = { x: Math.round(pointer.x - start.x), y: Math.round(pointer.y - start.y) }
          env.moving(false)
          await picked
          if (!delta.x && !delta.y) return
          // The engine also moves other selected/linked layers and their effects; active bounds
          // only describe the preview, not the complete region that needs repainting on commit.
          void env.run('layer.translate', { dx: delta.x, dy: delta.y })
        },
      }
    },
  }
}

/** Crop: proposes the dragged area, held to the options bar's ratio; the editor crops on Enter or the options bar's button. */
export function cropTool(env: PhotoToolEnv): KitTool {
  return rectDrag(
    'Crop',
    'crosshair',
    (area) => {
      if (area.width < 1 || area.height < 1) return false
      env.proposeCrop(area)
    },
    {
      area: (start, pointer) => {
        const ratio = cropRatioSize(env.options.cropRatio, env.documentSize.width, env.documentSize.height)
        if (!ratio) return dragRect(start, pointer, pointer.shift)
        // From a whole pixel, both sides rounded from the same scale: a 1 : 1 crop stays square
        // at any zoom (rounding the two corners outward on their own could differ by a pixel).
        const from = { x: Math.round(start.x), y: Math.round(start.y) }
        const end = marqueeEnd('fixedRatio', ratio[0], ratio[1], false, from, pointer)
        return pixelArea(from, { x: from.x + Math.round(end.x - from.x), y: from.y + Math.round(end.y - from.y) })
      },
    },
  )
}

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
