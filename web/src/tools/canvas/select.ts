import type { KitPointer, KitTool } from '@kition/sdk/editor'
import type { PhotoToolId } from '../catalog'
import { marqueeEnd, pixelArea, selectionModeFor, toolParams } from '../options'
import { NO_REPAINT, ellipseOutline, rectDrag, type KitGesture, type PhotoToolEnv } from './common'
import { centredBox, insideOutline } from './geometry'

/** How far (screen pixels) a press inside the selection runs before it moves the outline (less is a click). */
const OUTLINE_MOVE_AFTER_PX = 2

/** Draws selection segments `[ax, ay, bx, by, …]` moved by (`dx`, `dy`) as marching ants. */
function drawMovedAnts(segments: ArrayLike<number>, dx: number, dy: number, context: CanvasRenderingContext2D, toScreen: (x: number, y: number) => { x: number; y: number }) {
  context.beginPath()
  for (let index = 0; index + 3 < segments.length; index += 4) {
    const a = toScreen(segments[index] + dx, segments[index + 1] + dy)
    const b = toScreen(segments[index + 2] + dx, segments[index + 3] + dy)
    context.moveTo(Math.round(a.x) + 0.5, Math.round(a.y) + 0.5)
    context.lineTo(Math.round(b.x) + 0.5, Math.round(b.y) + 0.5)
  }
  context.lineWidth = 1
  context.strokeStyle = '#ffffff'
  context.stroke()
  context.strokeStyle = '#111111'
  context.setLineDash([4, 4])
  context.stroke()
}

/**
 * A selection tool's press inside the selection, in Replace mode with no keys, drags the selection's
 * outline (not its pixels) and drops it there in one step (`select.transformSelection`); a click
 * there is the tool's own (a marquee's click deselects). Elsewhere the tool works as before.
 */
export function withOutlineMove(id: PhotoToolId, tool: KitTool, env: PhotoToolEnv): KitTool {
  return {
    ...tool,
    begin: (start, context) => {
      const outline = env.selection ? env.selectionOutline : null
      const replace = !start.shift && !start.alt && !start.meta && selectionModeFor(id, env.options.selectionMode, start) === 'replace'
      if (!outline || !replace || !insideOutline(outline, start.x, start.y)) return tool.begin(start, context)
      let moved = false
      let delta = { x: 0, y: 0 }
      const offset = (pointer: KitPointer) => ({ x: Math.round(pointer.x - start.x), y: Math.round(pointer.y - start.y) })
      const show = (pointer: KitPointer) => {
        moved ||= Math.hypot(pointer.clientX - start.clientX, pointer.clientY - start.clientY) >= OUTLINE_MOVE_AFTER_PX
        if (!moved) return
        delta = offset(pointer)
        env.hideSelection(true)
        context.overlay.draw((canvas, toScreen) => drawMovedAnts(outline, delta.x, delta.y, canvas, toScreen))
      }
      const gesture: KitGesture = {
        move: show,
        end: (pointer, now) => {
          show(pointer)
          if (!moved) {
            // A click: the tool's own (a marquee's deselects).
            const own = tool.begin(start, now)
            own?.end?.(pointer, now)
            return
          }
          void (delta.x || delta.y ? env.run('select.transformSelection', { dx: delta.x, dy: delta.y }, NO_REPAINT) : Promise.resolve(false)).then((ran) => {
            env.hideSelection(false)
            if (!ran) now.overlay.clear()
          })
        },
        cancel: () => env.hideSelection(false),
      }
      return gesture
    },
  }
}

/**
 * The Rectangular and Elliptical Marquee: a drag selects its box, shaped by the options bar's
 * style (a fixed ratio or size; Shift squares a normal one). Empty add/subtract gestures preserve
 * the selection; empty replace/intersect gestures clear it.
 */
export function marqueeTool(id: 'RectMarquee' | 'EllipseMarquee', ellipse: boolean, env: PhotoToolEnv): KitTool {
  const done = (area: { x: number; y: number; width: number; height: number }, start: KitPointer) => {
    const mode = selectionModeFor(id, env.options.selectionMode, start)
    if (area.width < 1 || area.height < 1) {
      if (mode === 'replace' || mode === 'intersect') void env.run('select.deselect', {}, NO_REPAINT)
      else return false
    } else void env.run('select.rect', { ...area, ellipse, mode, ...toolParams(id, env.options) }, NO_REPAINT)
  }
  const tool: KitTool = {
    id,
    cursor: 'crosshair',
    begin: (pressed, context) => {
      // Shift and Alt at the press choose the mode (add, subtract); pressed again (or first) after
      // it, they square the box and draw it from its centre, as Photoshop's.
      let shiftFree = !pressed.shift
      let altFree = !pressed.alt
      const area = (start: KitPointer, pointer: KitPointer) => {
        if (!pointer.shift) shiftFree = true
        if (!pointer.alt) altFree = true
        const square = pointer.shift && shiftFree
        const centred = pointer.alt && altFree
        const o = env.options
        if (o.marqueeStyle === 'fixedSize') {
          const width = Math.max(1, Math.round(o.marqueeWidth))
          const height = Math.max(1, Math.round(o.marqueeHeight))
          if (centred) return { x: Math.round(start.x - width / 2), y: Math.round(start.y - height / 2), width, height }
          // Snap the anchor once: rounding both corners outwards adds a pixel at fractional zoom.
          return {
            x: Math.floor(start.x) - (pointer.x < start.x ? width : 0),
            y: Math.floor(start.y) - (pointer.y < start.y ? height : 0),
            width, height,
          }
        }
        if (centred && o.marqueeStyle === 'normal') return centredBox(start, pointer, square)
        const end = marqueeEnd(o.marqueeStyle, o.marqueeWidth, o.marqueeHeight, square, start, pointer)
        if (o.marqueeStyle === 'fixedRatio' || square) {
          // Quantize constrained extents, not both corners: fractional anchors must not
          // turn a square into a rectangle. Nonempty subpixel drags still select a pixel.
          const dx = end.x - start.x, dy = end.y - start.y
          const width = dx === 0 ? 0 : Math.max(1, Math.round(Math.abs(dx)))
          const height = dy === 0 ? 0 : Math.max(1, Math.round(Math.abs(dy)))
          if (centred) return { x: Math.round(start.x - width), y: Math.round(start.y - height), width: width * 2, height: height * 2 }
          return {
            x: Math.floor(start.x) - (dx < 0 ? width : 0),
            y: Math.floor(start.y) - (dy < 0 ? height : 0),
            width, height,
          }
        }
        const box = pixelArea(start, end)
        // Rounding fractional coordinates must not turn an empty gesture into a pixel.
        if (end.x === start.x) box.width = 0
        if (end.y === start.y) box.height = 0
        return box
      }
      return rectDrag(id, 'crosshair', done, { area, outline: ellipse ? ellipseOutline : undefined, readout: true }).begin(pressed, context)
    },
  }
  return withOutlineMove(id, tool, env)
}

/** Object Selection: the object inside the dragged box. */
export function objectSelectionTool(env: PhotoToolEnv): KitTool {
  const tool = rectDrag('ObjectSelection', 'crosshair', (area, start) => {
    if (area.width < 2 || area.height < 2) return false
    void env.run('select.object', { rect: [area.x, area.y, area.width, area.height], mode: selectionModeFor('ObjectSelection', 'replace', start), ...toolParams('ObjectSelection', env.options) }, NO_REPAINT)
  })
  return withOutlineMove('ObjectSelection', tool, env)
}

export function magicWandTool(env: PhotoToolEnv): KitTool {
  const tool: KitTool = {
    id: 'MagicWand',
    cursor: 'crosshair',
    begin: (pointer) => void env.run('select.magicWand', { x: Math.floor(pointer.x), y: Math.floor(pointer.y), mode: selectionModeFor('MagicWand', env.options.selectionMode, pointer), ...toolParams('MagicWand', env.options) }, NO_REPAINT),
  }
  return withOutlineMove('MagicWand', tool, env)
}
