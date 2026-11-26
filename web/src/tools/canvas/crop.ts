import { dragRect, type KitPointer, type KitTool } from '@kition/sdk/editor'
import { cropRatioSize, marqueeEnd, pixelArea } from '../options'
import { rectDrag, type CropFrame, type KitGesture, type PhotoToolEnv } from './common'
import { cropQuad, dragCrop, handleAt, handleCursor, type Handle, type Point, type Quad } from './geometry'

type KitToolContext = Parameters<KitTool['begin']>[1]

/** The frame's handle under the pointer: one of eight, inside, or outside (turn). */
function handleOf(frame: CropFrame, pointer: KitPointer, toScreen: (x: number, y: number) => Point): Handle | null {
  const screen = cropQuad(frame).map((p) => toScreen(p.x, p.y)) as Quad
  return handleAt(screen, null, { x: pointer.canvasX, y: pointer.canvasY })
}

/** The options bar's ratio for the frame (a preset or typed W : H), or null when free. */
function heldRatio(env: PhotoToolEnv): [number, number] | null {
  return cropRatioSize(env.options.cropRatio, env.documentSize.width, env.documentSize.height)
}

/**
 * A drag that draws a new frame (proposed to the editor on release), held to the bar's ratio
 * (Shift squares a free one). The frame there was gives way meanwhile, and comes back when the
 * drag draws nothing.
 */
/**
 * The drawn frame's dashed preview stays on the overlay after the release (no document change
 * clears it); the next hover or frame drag clears it, before the frame is turned or moved away from it.
 */
let previewLeft = false

/** A point snapped (plan 2026-11-24, stage 7) to guides, the grid, layers and the document, as View › Snap To asks. */
function snapped(env: PhotoToolEnv, pointer: KitPointer, zoom: () => number): KitPointer {
  if (!env.snap) return pointer
  const at = env.snap.point({ x: pointer.x, y: pointer.y }, zoom())
  return { ...pointer, x: at.x, y: at.y }
}

function drawFrame(env: PhotoToolEnv, press: KitPointer, context: KitToolContext): KitGesture | void {
  // Read only when snapping (a tool driven on its own may have no viewport).
  const zoom = () => context.viewport?.view.zoom ?? 1
  const start = snapped(env, press, zoom)
  const before = env.crop?.frame ?? null
  let proposed = false
  const drag = rectDrag(
    'Crop',
    'crosshair',
    (area) => {
      if (area.width < 1 || area.height < 1) return false
      proposed = true
      previewLeft = true
      env.proposeCrop(area)
    },
    {
      area: (from, raw) => {
        const pointer = snapped(env, raw, zoom)
        const ratio = heldRatio(env)
        if (!ratio) return dragRect(from, pointer, pointer.shift)
        // From a whole pixel, both sides rounded from the same scale: a 1 : 1 crop stays square
        // at any zoom (rounding the two corners outward on their own could differ by a pixel).
        const at = { x: Math.round(from.x), y: Math.round(from.y) }
        const end = marqueeEnd('fixedRatio', ratio[0], ratio[1], false, at, pointer)
        return pixelArea(at, { x: at.x + Math.round(end.x - at.x), y: at.y + Math.round(end.y - at.y) })
      },
    },
  ).begin(start, context)
  if (!drag) return
  if (before) env.crop.update(null, false)
  const restore = () => {
    if (before && !proposed) env.crop.update(before, true)
  }
  return {
    ...drag,
    end: (pointer, now) => {
      drag.end?.(pointer, now)
      env.snap?.done()
      restore()
    },
    cancel: (now) => {
      drag.cancel?.(now)
      env.snap?.done()
      restore()
    },
  }
}

/**
 * Crop (plan 2026-11-24, stage 2): choosing the tool puts a frame round the whole canvas; its
 * eight handles size it (held to the bar's ratio, Shift to its own proportions), a drag inside
 * moves it, a drag outside turns it (Shift: 15° steps), and a drag inside the untouched first frame,
 * or anywhere when there is none, draws a new one. Enter or a double click inside crops; Escape
 * drops the frame.
 */
export function cropTool(env: PhotoToolEnv): KitTool {
  return {
    id: 'Crop',
    cursor: 'crosshair',
    begin: (pointer, context) => {
      const { viewport } = context
      const frame = env.crop?.frame ?? null
      if (!frame) return drawFrame(env, pointer, context)
      const handle = handleOf(frame, pointer, (x, y) => viewport.toScreen(x, y))
      if (!handle || (handle === 'inside' && frame.initial)) return drawFrame(env, pointer, context)
      if (previewLeft) {
        previewLeft = false
        context.overlay.clear()
      }
      const from = { x: pointer.x, y: pointer.y }
      const zoom = () => viewport.view.zoom
      // A handle snaps where it is dragged; the moved frame by its edges and centre; a turn does not.
      const snap = (to: KitPointer): KitPointer => {
        if (!env.snap || handle === 'rotate') return to
        if (handle !== 'inside') return snapped(env, to, zoom)
        const moved = env.snap.move({ x0: frame.x, y0: frame.y, x1: frame.x + frame.width, y1: frame.y + frame.height }, to.x - from.x, to.y - from.y, zoom())
        return { ...to, x: from.x + moved.dx, y: from.y + moved.dy }
      }
      const place = (raw: KitPointer): CropFrame => {
        const to = snap(raw)
        return { ...dragCrop(frame, handle, from, to, { shift: to.shift }, handle === 'inside' || handle === 'rotate' ? null : heldRatio(env)), initial: false }
      }
      return {
        move: (to) => env.crop.update(place(to), false),
        end: (to) => {
          env.crop.update(place(to), true)
          env.snap?.done()
        },
        cancel: () => {
          env.snap?.done()
          env.crop.update(frame, true)
        },
      }
    },
    hover: (pointer, { viewport, overlay }) => {
      if (previewLeft) {
        previewLeft = false
        overlay.clear()
      }
      const frame = env.crop.frame
      const handle = frame ? handleOf(frame, pointer, (x, y) => viewport.toScreen(x, y)) : null
      env.setCursor(frame && handle && !(handle === 'inside' && frame.initial) ? handleCursor(handle, (frame.angle * Math.PI) / 180) : null)
    },
    leave: () => env.setCursor(null),
    doubleClick: (pointer, { viewport }) => {
      const frame = env.crop.frame
      if (frame && handleOf(frame, pointer, (x, y) => viewport.toScreen(x, y)) === 'inside') env.crop.commit()
    },
  }
}
