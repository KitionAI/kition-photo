import type { KitTool } from '@kition/sdk/editor'
import { shapeStyle } from '../options'
import { HIT_RADIUS, NO_REPAINT, SOFT_EDGE, clickTool, dashed, ellipseOutline, lineDrag, rectArea, rectDrag, tracePath, tracePolyline, type PathData, type PathKnot, type PhotoToolEnv } from './common'

/** What each shape tool asks `shape.create` for, beyond its box, fill, and stroke (upstream's `shape_geometry`). */
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
      else void env.run('shape.create', { ...kind(id, env), rect, ...appearance }, damage)
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
    void env.run('shape.create', { kind: 'line', from: [start.x, start.y], to: [start.x + dx, start.y + dy], weight: Math.max(1, env.options.weight), ...shapeStyle(env.options, env.foreground, env.background) })
  })
}

/** A Pen knot: anchor, then its in and out handles. */
type PenKnot = [number, number, number, number, number, number]

const knotOf = ([x, y, ix, iy, ox, oy]: PenKnot): PathKnot => ({ anchor: [x, y], in: [ix, iy], out: [ox, oy], smooth: ix !== x || iy !== y || ox !== x || oy !== y })

/**
 * The Pen: each click adds an anchor, a drag after it pulls smooth handles; a click on the first
 * anchor closes the path, Enter leaves it open; a double click is two clicks, as upstream. In Path
 * mode the path becomes the work path; in Shape mode a shape layer with the shape tools' Fill and
 * Stroke when closed, else stroked with the foreground colour at least 1 px wide (`pen_commit`).
 */
export function penTool(env: PhotoToolEnv): KitTool {
  return clickTool<PenKnot>('Pen', env, {
    min: 2,
    point: (pointer) => [pointer.x, pointer.y, pointer.x, pointer.y, pointer.x, pointer.y],
    drag: (knot, pointer) => {
      knot[4] = pointer.x
      knot[5] = pointer.y
      knot[2] = 2 * knot[0] - pointer.x
      knot[3] = 2 * knot[1] - pointer.y
    },
    draw: (context, toScreen, knots, hover) => {
      tracePath(context, toScreen, { subpaths: [{ closed: false, knots: knots.map(knotOf) }] })
      if (hover && knots.length) {
        const last = toScreen(knots[knots.length - 1][0], knots[knots.length - 1][1])
        const to = toScreen(hover.x, hover.y)
        context.moveTo(last.x, last.y)
        context.lineTo(to.x, to.y)
      }
      dashed(context)
      for (const [x, y] of knots) {
        const p = toScreen(x, y)
        context.fillStyle = '#ffffff'
        context.fillRect(p.x - 3, p.y - 3, 6, 6)
        context.strokeStyle = '#111111'
        context.setLineDash([])
        context.strokeRect(p.x - 3, p.y - 3, 6, 6)
      }
    },
    finish: (knots, _first, closed) => {
      const path: PathData = { subpaths: [{ closed, knots: knots.map(knotOf) }] }
      if (env.options.penMode === 'shape') {
        const style = shapeStyle(env.options, env.foreground, env.background)
        const open = { fill: null, stroke: { width: Math.max(1, env.options.strokeWidth), color: env.foreground } }
        return env.run('shape.create', { kind: 'path', path, ...(closed ? style : open) })
      }
      return env.run('path.set', { name: 'work', path }, NO_REPAINT)
    },
  })
}

/** Path Selection: drags the active shape layer's path, else the work path. */
export function pathSelectionTool(env: PhotoToolEnv): KitTool {
  return lineDrag('PathSelection', 'default', (start, end) => {
    const dx = Math.round(end.x - start.x)
    const dy = Math.round(end.y - start.y)
    if (!dx && !dy) return
    const layer = env.layer
    if (layer?.kind === 'Shape') void env.run('shape.edit', { layer: layer.id, move: [dx, dy] })
    else
      // Upstream's `target_path`: without a work path there is nothing to move, and the drag does nothing.
      void env.query<{ path: PathData }>('path.info', { name: 'work' }).then((work) => {
        if (work?.path) void env.run('path.transform', { name: 'work', translateX: dx, translateY: dy }, NO_REPAINT)
      })
  })
}

/** Direct Selection: drags the work path's anchor under the press, with its handles. */
export function directSelectionTool(env: PhotoToolEnv): KitTool {
  return {
    id: 'DirectSelection',
    cursor: 'default',
    begin: (start, { overlay, viewport }) => {
      const reach = HIT_RADIUS / viewport.view.zoom
      // The path is read as the drag starts; the hit is decided once it has answered.
      const work = env.query<{ path: PathData }>('path.info', { name: 'work' })
      return {
        move: (end) => {
          overlay.draw((context, toScreen) => {
            tracePolyline(context, toScreen, [
              [start.x, start.y],
              [end.x, end.y],
            ])
            dashed(context)
          })
        },
        end: async (end) => {
          const dx = Math.round(end.x - start.x)
          const dy = Math.round(end.y - start.y)
          const path = (await work)?.path
          if (!path || (!dx && !dy)) return
          const hits = path.subpaths.flatMap((subpath, s) => subpath.knots.map((knot, k) => ({ at: [s, k], distance: Math.hypot(knot.anchor[0] - start.x, knot.anchor[1] - start.y) })))
          const hit = hits.filter((item) => item.distance <= reach).sort((a, b) => a.distance - b.distance)[0]
          if (hit) void env.run('path.moveAnchors', { name: 'work', anchors: [hit.at], move: [dx, dy] }, NO_REPAINT)
        },
      }
    },
  }
}
