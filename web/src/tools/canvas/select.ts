import type { KitTool } from '@kition/sdk/editor'
import { marqueeEnd, pixelArea, selectionModeFor, toolParams } from '../options'
import { NO_REPAINT, ellipseOutline, rectDrag, type PhotoToolEnv } from './common'

/**
 * The Rectangular and Elliptical Marquee: a drag selects its box, shaped by the options bar's
 * style (a fixed ratio or size; Shift squares a normal one). Empty add/subtract gestures preserve
 * the selection; empty replace/intersect gestures clear it.
 */
export function marqueeTool(id: 'RectMarquee' | 'EllipseMarquee', ellipse: boolean, env: PhotoToolEnv): KitTool {
  return rectDrag(
    id,
    'crosshair',
    (area, start) => {
      const mode = selectionModeFor(id, env.options.selectionMode, start)
      if (area.width < 1 || area.height < 1) {
        if (mode === 'replace' || mode === 'intersect') void env.run('select.deselect', {}, NO_REPAINT)
        else return false
      } else void env.run('select.rect', { ...area, ellipse, mode, ...toolParams(id, env.options) }, NO_REPAINT)
    },
    {
      area: (start, pointer) => {
        const o = env.options
        if (o.marqueeStyle === 'fixedSize') {
          const width = Math.max(1, Math.round(o.marqueeWidth))
          const height = Math.max(1, Math.round(o.marqueeHeight))
          // Snap the anchor once: rounding both corners outwards adds a pixel at fractional zoom.
          return {
            x: Math.floor(start.x) - (pointer.x < start.x ? width : 0),
            y: Math.floor(start.y) - (pointer.y < start.y ? height : 0),
            width, height,
          }
        }
        const end = marqueeEnd(o.marqueeStyle, o.marqueeWidth, o.marqueeHeight, pointer.shift, start, pointer)
        if (o.marqueeStyle === 'fixedRatio' || pointer.shift) {
          // Quantize constrained extents, not both corners: fractional anchors must not
          // turn a square into a rectangle. Nonempty subpixel drags still select a pixel.
          const dx = end.x - start.x, dy = end.y - start.y
          const width = dx === 0 ? 0 : Math.max(1, Math.round(Math.abs(dx)))
          const height = dy === 0 ? 0 : Math.max(1, Math.round(Math.abs(dy)))
          return {
            x: Math.floor(start.x) - (dx < 0 ? width : 0),
            y: Math.floor(start.y) - (dy < 0 ? height : 0),
            width, height,
          }
        }
        const area = pixelArea(start, end)
        // Rounding fractional coordinates must not turn an empty gesture into a pixel.
        if (end.x === start.x) area.width = 0
        if (end.y === start.y) area.height = 0
        return area
      },
      outline: ellipse ? ellipseOutline : undefined,
    },
  )
}

/** Object Selection: the object inside the dragged box. */
export function objectSelectionTool(env: PhotoToolEnv): KitTool {
  return rectDrag('ObjectSelection', 'crosshair', (area, start) => {
    if (area.width < 2 || area.height < 2) return false
    void env.run('select.object', { rect: [area.x, area.y, area.width, area.height], mode: selectionModeFor('ObjectSelection', 'replace', start), ...toolParams('ObjectSelection', env.options) }, NO_REPAINT)
  })
}

export function magicWandTool(env: PhotoToolEnv): KitTool {
  return {
    id: 'MagicWand',
    cursor: 'crosshair',
    begin: (pointer) => void env.run('select.magicWand', { x: Math.floor(pointer.x), y: Math.floor(pointer.y), mode: selectionModeFor('MagicWand', env.options.selectionMode, pointer), ...toolParams('MagicWand', env.options) }, NO_REPAINT),
  }
}
