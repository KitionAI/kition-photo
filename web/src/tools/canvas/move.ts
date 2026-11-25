import type { KitPointer, KitTool } from '@kition/sdk/editor'
import { NO_REPAINT, accent, type KitGesture, type PhotoToolEnv } from './common'
import { constrainEight, dragTransform, handleAt, handleCursor, quadAngle, quadCentre, rectQuad, type Handle, type Point, type Quad } from './geometry'

/** How far (screen pixels) an Alt-drag runs before it copies the layer (a plain Alt-click copies nothing). */
const COPY_AFTER_PX = 2

/**
 * Auto-Select (or Cmd-click while it is off) picks the layer under the press first, as upstream:
 * the answer is the layer it picked, or null when the active layer stayed.
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

/** The Show Transform Controls handle under a press on the active layer's box, or null. */
function boxHandle(env: PhotoToolEnv, pointer: KitPointer, toScreen: (x: number, y: number) => Point): Handle | null {
  const layer = env.layer
  if (!env.options.showTransform || !layer?.bounds || layer.locked) return null
  const { x0, y0, x1, y1 } = layer.bounds
  const screen = rectQuad(x0, y0, x1, y1).map((p) => toScreen(p.x, p.y)) as Quad
  const handle = handleAt(screen, null, { x: pointer.canvasX, y: pointer.canvasY }, false)
  return handle && handle !== 'inside' ? handle : null
}

/**
 * Move (plan 2026-11-24, stage 2): drags the active layer, its pixels following the pointer (a
 * snapshot of the layer drawn over the canvas until the release moves it, in one history step);
 * Shift holds the drag to eight directions; Alt copies the layer first (Duplicate Layer, then the
 * move); with a selection it drags the selected pixels (`select.float`, Alt: a copy of them). A
 * layer whose position is locked says so and moves nothing. Auto-Select picks the layer under the
 * press first; with Show Transform Controls, a press on a handle of the box opens Free Transform.
 */
export function moveTool(env: PhotoToolEnv): KitTool {
  return {
    id: 'Move',
    cursor: 'move',
    begin: (start, context) => {
      const { overlay, viewport } = context
      const handle = boxHandle(env, start, (x, y) => viewport.toScreen(x, y))
      if (handle && env.transform.start()) return transformGesture(env, handle, start)
      const floating = env.selection != null && env.layer?.kind === 'Pixel'
      env.moving(true)
      // With a selection the selected pixels move: Auto-Select does not pick another layer.
      const picked = floating ? Promise.resolve(null) : autoSelect(env, start)
      // The layer the drag moves, once known: the picked one is known when the editor shows it active.
      let pickedLayer: number | null | undefined
      void picked.then((layer) => {
        pickedLayer = layer
        judge()
      })
      let refused = false
      let judged = false
      let image: { bitmap: ImageBitmap; opacity: number } | null = null
      let bounds = env.layer?.bounds ?? null
      const judge = () => {
        if (judged || pickedLayer === undefined) return
        const active = env.layer
        if (!active || active.id == null || (pickedLayer !== null && active.id !== pickedLayer)) return
        judged = true
        bounds = active.bounds
        // A locked layer stays (the Background's selected pixels may still float, leaving the background colour).
        if (floating ? active.locked && !active.background : active.locked) {
          refused = true
          overlay.clear()
          tell()
          return
        }
        const layer = active.id
        void env.layerImage(layer).then((bitmap) => {
          if (bitmap && !refused) image = { bitmap, opacity: active.opacity }
          else bitmap?.close()
          if (image && moved) draw()
        })
      }
      let delta = { x: 0, y: 0 }
      let moved = false
      // A drag of a locked layer says why nothing moves (a plain click says nothing).
      let told = false
      const tell = () => {
        if (!refused || !moved || told) return
        told = true
        env.say('move.locked')
      }
      const offsetOf = (pointer: KitPointer) => {
        const dx = pointer.x - start.x
        const dy = pointer.y - start.y
        return pointer.shift ? constrainEight(dx, dy) : { x: Math.round(dx), y: Math.round(dy) }
      }
      const draw = () => {
        if (refused) return
        const box = bounds
        const clip = floating ? env.selection : null
        overlay.draw((canvas, toScreen) => {
          const size = env.documentSize
          if (image) {
            const a = toScreen(delta.x, delta.y)
            const b = toScreen(size.width + delta.x, size.height + delta.y)
            canvas.save()
            if (clip) {
              const c0 = toScreen(clip[0] + delta.x, clip[1] + delta.y)
              const c1 = toScreen(clip[0] + clip[2] + delta.x, clip[1] + clip[3] + delta.y)
              canvas.beginPath()
              canvas.rect(c0.x, c0.y, c1.x - c0.x, c1.y - c0.y)
              canvas.clip()
            }
            canvas.globalAlpha = image.opacity
            canvas.imageSmoothingEnabled = viewport.view.zoom < 1
            canvas.drawImage(image.bitmap, a.x, a.y, b.x - a.x, b.y - a.y)
            canvas.restore()
          }
          const outline = clip ? { x0: clip[0], y0: clip[1], x1: clip[0] + clip[2], y1: clip[1] + clip[3] } : box
          if (!outline) return
          const a = toScreen(outline.x0 + delta.x, outline.y0 + delta.y)
          const b = toScreen(outline.x1 + delta.x, outline.y1 + delta.y)
          canvas.setLineDash([])
          canvas.lineWidth = 1
          canvas.strokeStyle = accent()
          canvas.strokeRect(Math.round(a.x) + 0.5, Math.round(a.y) + 0.5, Math.round(b.x - a.x), Math.round(b.y - a.y))
        })
      }
      return {
        move: (pointer) => {
          judge()
          moved ||= Math.hypot(pointer.clientX - start.clientX, pointer.clientY - start.clientY) >= COPY_AFTER_PX
          if (!moved) return
          tell()
          delta = offsetOf(pointer)
          draw()
        },
        cancel: () => {
          env.moving(false)
          refused = true
          image?.bitmap.close()
          image = null
        },
        end: async (pointer) => {
          // A release may come with no move before it (sparse input): its own position counts.
          moved ||= Math.hypot(pointer.clientX - start.clientX, pointer.clientY - start.clientY) >= COPY_AFTER_PX
          if (moved) delta = offsetOf(pointer)
          env.moving(false)
          await picked
          judge()
          refused ||= !judged && Boolean(env.layer?.locked && !floating && pickedLayer === null)
          const done = image
          image = null
          done?.bitmap.close()
          tell()
          if (refused || !moved) return void overlay.clear()
          const { x: dx, y: dy } = delta
          if (floating) {
            if (!(await env.run('select.float', { dx, dy, copy: start.alt }))) overlay.clear()
            return
          }
          if (start.alt) {
            if (!(await env.run('layer.duplicate', {}))) return void overlay.clear()
          }
          if (!dx && !dy) return void overlay.clear()
          // The engine also moves other selected/linked layers and their effects.
          if (!(await env.run('layer.translate', { dx, dy }))) overlay.clear()
        },
      }
    },
    hover: (pointer, { viewport }) => {
      const handle = boxHandle(env, pointer, (x, y) => viewport.toScreen(x, y))
      env.setCursor(handle ? handleCursor(handle, 0) : null)
    },
    leave: () => env.setCursor(null),
  }
}

/**
 * One drag on Free Transform's box: a handle scales (Shift proportional, Alt about the reference
 * point; Cmd moves a corner alone or slides an edge), inside moves, outside rotates (Shift: 15°),
 * the reference point moves itself. The layer follows live (the editor previews `edit.transform`).
 */
export function transformGesture(env: PhotoToolEnv, handle: Handle, start: KitPointer): KitGesture | void {
  const box = env.transform.box
  if (!box) return
  const from = { x: start.x, y: start.y }
  const startQuad = box.quad
  const startReference = box.reference
  // A reference point left at the centre stays at the centre of the box.
  const centred = Math.hypot(startReference.x - quadCentre(startQuad).x, startReference.y - quadCentre(startQuad).y) < 1e-6
  const modeKeys = box.mode !== 'free'
  let quad = startQuad
  let reference = startReference
  const place = (pointer: KitPointer) => {
    if (handle === 'centre') {
      reference = { x: pointer.x, y: pointer.y }
      return
    }
    quad = dragTransform(startQuad, startReference, handle, from, pointer, { shift: pointer.shift, alt: pointer.alt, meta: pointer.meta || modeKeys })
    if (centred) reference = quadCentre(quad)
    else if (handle === 'inside') reference = { x: startReference.x + (quad[0].x - startQuad[0].x), y: startReference.y + (quad[0].y - startQuad[0].y) }
  }
  return {
    move: (pointer) => {
      place(pointer)
      env.transform.update(quad, reference, false)
    },
    end: (pointer) => {
      place(pointer)
      env.transform.update(quad, reference, true)
    },
    cancel: () => env.transform.update(startQuad, startReference, true),
  }
}

/**
 * The canvas while Free Transform's box is open: drags on it (see `transformGesture`), the
 * handles' cursors, and a double click inside commits (as Enter does).
 */
export function transformTool(env: PhotoToolEnv, commit: () => void): KitTool {
  const handleOf = (pointer: KitPointer, toScreen: (x: number, y: number) => Point) => {
    const box = env.transform.box
    if (!box) return null
    const screen = box.quad.map((p) => toScreen(p.x, p.y)) as Quad
    return handleAt(screen, toScreen(box.reference.x, box.reference.y), { x: pointer.canvasX, y: pointer.canvasY })
  }
  return {
    id: 'FreeTransform',
    cursor: 'default',
    begin: (pointer, { viewport }) => {
      const handle = handleOf(pointer, (x, y) => viewport.toScreen(x, y))
      return handle ? transformGesture(env, handle, pointer) : undefined
    },
    hover: (pointer, { viewport }) => {
      const handle = handleOf(pointer, (x, y) => viewport.toScreen(x, y))
      const box = env.transform.box
      env.setCursor(handle && box ? handleCursor(handle, quadAngle(box.quad)) : null)
    },
    leave: () => env.setCursor(null),
    doubleClick: (pointer, { viewport }) => {
      if (handleOf(pointer, (x, y) => viewport.toScreen(x, y)) === 'inside') commit()
    },
  }
}
