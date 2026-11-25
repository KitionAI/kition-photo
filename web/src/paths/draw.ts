import { accent, tracePath } from '../tools/canvas/common'
import { curved, type AnchorRef, type PathData } from './bezier'
import { pathTargets, selectedOn, type PathView } from './store'

type ToScreen = (x: number, y: number) => { x: number; y: number }

/** Anchors' side and direction points' radius, in screen pixels: the same at any zoom. */
const ANCHOR_SIDE = 7
const HANDLE_RADIUS = 3.5

/** How a path's points show: every anchor (the Pen, Direct Selection, Convert Point) or only the selected ones, and whose direction lines. */
export type PointsShown = { anchors: 'all' | 'selected' | 'none'; handles: 'selected' | 'all' }

/** The tools that show a path's anchors, and how. */
export function pointsFor(tool: string): PointsShown {
  if (tool === 'ConvertPoint') return { anchors: 'all', handles: 'all' }
  if (tool === 'DirectSelection' || tool === 'Pen') return { anchors: 'all', handles: 'selected' }
  if (tool === 'PathSelection') return { anchors: 'selected', handles: 'selected' }
  return { anchors: 'none', handles: 'selected' }
}

/** The path's outline: a dark halo under a line in the accent colour (as upstream). */
export function outlinePath(context: CanvasRenderingContext2D, toScreen: ToScreen, path: PathData) {
  tracePath(context, toScreen, path)
  context.setLineDash([])
  context.lineWidth = 3
  context.strokeStyle = 'rgba(0, 0, 0, 0.45)'
  context.stroke()
  context.lineWidth = 1
  context.strokeStyle = accent()
  context.stroke()
}

/**
 * A path with its points: the outline, the anchors as small squares (filled when selected, hollow
 * otherwise) and the direction lines of the anchors `shown` asks for, each ending in a dot.
 */
export function drawPathPoints(context: CanvasRenderingContext2D, toScreen: ToScreen, path: PathData, selected: readonly AnchorRef[], shown: PointsShown) {
  outlinePath(context, toScreen, path)
  if (shown.anchors === 'none') return
  const isSelected = (s: number, k: number) => selected.some(([a, b]) => a === s && b === k)
  const colour = accent()
  context.setLineDash([])
  context.lineWidth = 1
  path.subpaths.forEach((subpath, s) =>
    subpath.knots.forEach((knot, k) => {
      const chosen = isSelected(s, k)
      if (!(shown.handles === 'all' || chosen) || !curved(knot)) return
      const a = toScreen(...knot.anchor)
      for (const handle of [knot.in, knot.out]) {
        if (handle[0] === knot.anchor[0] && handle[1] === knot.anchor[1]) continue
        const h = toScreen(...handle)
        context.beginPath()
        context.moveTo(a.x, a.y)
        context.lineTo(h.x, h.y)
        context.strokeStyle = colour
        context.stroke()
        context.beginPath()
        context.arc(h.x, h.y, HANDLE_RADIUS, 0, Math.PI * 2)
        context.fillStyle = colour
        context.fill()
      }
    }),
  )
  path.subpaths.forEach((subpath, s) =>
    subpath.knots.forEach((knot, k) => {
      const chosen = isSelected(s, k)
      if (shown.anchors === 'selected' && !chosen) return
      const p = toScreen(...knot.anchor)
      const x = Math.round(p.x - ANCHOR_SIDE / 2) + 0.5
      const y = Math.round(p.y - ANCHOR_SIDE / 2) + 0.5
      context.fillStyle = chosen ? colour : '#ffffff'
      context.fillRect(x, y, ANCHOR_SIDE - 1, ANCHOR_SIDE - 1)
      context.strokeStyle = colour
      context.strokeRect(x, y, ANCHOR_SIDE - 1, ANCHOR_SIDE - 1)
    }),
  )
}

/** The paths the path tools edit, as the canvas shows them while one of them is chosen (a dragged one is the tool's to draw). */
export function drawPathEditing(context: CanvasRenderingContext2D, toScreen: ToScreen, view: PathView, tool: string) {
  const shown = pointsFor(tool)
  for (const target of pathTargets(view)) {
    if (view.editing === target.kind) continue
    drawPathPoints(context, toScreen, target.path, selectedOn(view, target), shown)
  }
}
