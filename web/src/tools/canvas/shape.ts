import type { KitTool } from '@kition/sdk/editor'
import { shapeStyle } from '../options'
import { SOFT_EDGE, ellipseOutline, lineDrag, rectArea, rectDrag, type PhotoToolEnv } from './common'

/** What each shape tool asks `shape.add` for, beyond its box, fill, and stroke (upstream's `shape_geometry`). */
function kind(id: string, env: PhotoToolEnv): Record<string, unknown> {
  const o = env.options
  if (id === 'Rectangle') return o.radius > 0 ? { kind: 'roundedRect', radii: [o.radius, o.radius, o.radius, o.radius] } : { kind: 'rect' }
  if (id === 'EllipseShape') return { kind: 'ellipse' }
  if (id === 'Triangle') return { kind: 'polygon', sides: 3 }
  return { kind: 'polygon', sides: Math.max(3, o.sides) }
}

/**
 * Rectangle, Ellipse, Triangle, Polygon, and Custom Shape: a drag makes a shape layer, filled with
 * the foreground colour and stroked with the background colour as the options bar says (Shift squares it).
 */
export function shapeTool(id: string, env: PhotoToolEnv): KitTool {
  const style = () => shapeStyle(env.options, env.foreground, env.background)
  return rectDrag(
    id,
    'crosshair',
    (area, start) => {
      if (area.width < 1 || area.height < 1) return false
      const rect = [area.x, area.y, area.width, area.height]
      const appearance = style()
      // The engine owns stroke bounds, including miter joins that extend beyond half the width.
      const damage = appearance.stroke ? null : rectArea(area, SOFT_EDGE)
      if (id === 'CustomShape') void env.run('shape.presets.place', { preset: env.options.customShape, rect, keepAspect: start.shift, ...appearance }, damage)
      else void env.run('shape.add', { ...kind(id, env), rect, ...appearance }, damage)
    },
    { square: true, outline: id === 'EllipseShape' ? ellipseOutline : undefined, fill: () => (env.options.shapeFill ? env.foreground : null) },
  )
}

/** The Line tool: a drag makes a line shape layer (Shift snaps it to 45°). */
export function lineTool(env: PhotoToolEnv): KitTool {
  return lineDrag('Line', 'crosshair', (start, end) => {
    let dx = end.x - start.x
    let dy = end.y - start.y
    if (end.shift) {
      const angle = Math.round(Math.atan2(dy, dx) / (Math.PI / 4)) * (Math.PI / 4)
      const length = Math.hypot(dx, dy)
      dx = length * Math.cos(angle)
      dy = length * Math.sin(angle)
    }
    if (Math.hypot(dx, dy) < 1) return
    void env.run('shape.add', { kind: 'line', from: [start.x, start.y], to: [start.x + dx, start.y + dy], weight: Math.max(1, env.options.weight), ...shapeStyle(env.options, env.foreground, env.background) })
  })
}

// The Pen, Path Selection and Direct Selection edit paths (plan 2026-11-24, stage 7): `pathTools.ts`.
export { directSelectionTool, pathSelectionTool, penTool } from './pathTools'
