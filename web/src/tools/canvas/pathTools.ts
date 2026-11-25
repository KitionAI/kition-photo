import type { KitPointer, KitTool } from '@kition/sdk/editor'
import { shapeStyle } from '../options'
import { anchorsIn, convertPoint, deleteAnchor, deleteAnchors, hitAnchor, hitHandle, hitSegment, hitSubpath, moveAnchors, moveHandle, movePath, splitSegment, subpathAnchors, type AnchorRef, type PathData, type Pt } from '../../paths/bezier'
import { drawPathPoints, outlinePath, pointsFor } from '../../paths/draw'
import { partAt, pathTargets, prunedSelection, replaceCommand, selectedOn, targetParams, type PathKind, type PathTarget } from '../../paths/store'
import { HIT_RADIUS, NO_REPAINT, clickTool, dashed, type KitGesture, type KitOverlay, type PhotoToolEnv } from './common'

type Context = Parameters<KitTool['begin']>[1]
type ToScreen = (x: number, y: number) => { x: number; y: number }

/** How far (screen pixels) a press runs before it is a drag, not a click. */
const DRAG_AFTER_PX = 2

/** Clears a gesture's preview (a bare overlay, as tests drive a tool with, may have no `clear`). */
const clear = (overlay: KitOverlay) => (overlay as Partial<KitOverlay>).clear?.()

/**
 * The paths a press can reach: the editor's (the active shape layer's, then the work path), or,
 * where there is no editor's store (a tool driven on its own), the work path as the engine answers it.
 */
async function targetsOf(env: PhotoToolEnv): Promise<PathTarget[]> {
  if (env.paths) return pathTargets(env.paths.get())
  const work = await env.query<{ path: PathData }>('path.info', { name: 'work' })
  return work?.path ? [{ kind: 'work', layer: null, path: work.path }] : []
}

/** Selects `anchors` of `target` (none: clears the selection), and redraws the canvas's marks. */
function select(env: PhotoToolEnv, target: { kind: PathKind; layer: number | null } | null, anchors: readonly AnchorRef[]) {
  if (!env.paths) return
  const view = env.paths.get()
  env.paths.set({ ...view, selected: target ? { kind: target.kind, layer: target.layer, anchors: [...anchors] } : null })
  env.redraw()
}

/** While a gesture draws `kind`'s path itself, the canvas leaves it out (null: draws it again). */
function editing(env: PhotoToolEnv, kind: PathKind | null) {
  if (!env.paths) return
  env.paths.set({ ...env.paths.get(), editing: kind })
  env.redraw()
}

/** Puts `path` into the editor's store at once (the engine's answer follows), so the canvas shows no step back. */
function settle(env: PhotoToolEnv, target: PathTarget, path: PathData) {
  if (!env.paths) return
  const view = env.paths.get()
  const next = target.kind === 'work' ? { ...view, work: path } : { ...view, shape: { layer: target.layer!, path } }
  env.paths.set({ ...next, editing: null, selected: prunedSelection(next) })
  env.redraw()
}

/** Runs a path command and ends the gesture's preview: false when the engine refused it. */
async function commit(env: PhotoToolEnv, overlay: KitOverlay, target: PathTarget, after: PathData, command: string, params: Record<string, unknown>) {
  const ran = await env.run(command, params, target.kind === 'work' ? NO_REPAINT : undefined)
  if (ran) settle(env, target, after)
  else editing(env, null)
  clear(overlay)
  return ran
}

/** Draws a path being dragged (the tool's preview) with the tool's points. */
function preview(overlay: KitOverlay, path: PathData, anchors: readonly AnchorRef[], tool: string) {
  overlay.draw((context, toScreen) => drawPathPoints(context, toScreen, path, anchors, pointsFor(tool)))
}

/** A dashed rectangle between two document points (Direct Selection's marquee). */
function drawMarquee(overlay: KitOverlay, a: Pt, b: Pt) {
  overlay.draw((context, toScreen) => {
    const p = toScreen(a[0], a[1])
    const q = toScreen(b[0], b[1])
    context.beginPath()
    context.rect(Math.min(p.x, q.x) + 0.5, Math.min(p.y, q.y) + 0.5, Math.abs(q.x - p.x), Math.abs(q.y - p.y))
    dashed(context)
  })
}

const offset = (start: KitPointer, pointer: KitPointer) => ({ dx: Math.round(pointer.x - start.x), dy: Math.round(pointer.y - start.y) })
const dragged = (start: KitPointer, pointer: KitPointer, zoom: number) => Math.hypot(pointer.x - start.x, pointer.y - start.y) * zoom >= DRAG_AFTER_PX

/**
 * Direct Selection (plan 2026-11-24, stage 7): a press on a selected anchor's direction point
 * drags it (a smooth point keeps its direction lines in line; Alt breaks them); on an anchor
 * selects it (Shift adds or takes it away) and a drag moves the selected anchors; on a segment
 * selects its two anchors, a drag bends it; elsewhere a drag selects the anchors inside its box.
 * Works on the work path and the active shape layer's path; each drag is one history step.
 */
export function directGesture(env: PhotoToolEnv, start: KitPointer, { overlay, viewport }: Context, tool = 'DirectSelection'): KitGesture {
  const zoom = viewport.view.zoom || 1
  const reach = HIT_RADIUS / zoom
  const p: Pt = [start.x, start.y]
  type Hit =
    | { kind: 'handle'; target: PathTarget; subpath: number; knot: number; handle: 'in' | 'out' }
    | { kind: 'anchor'; target: PathTarget; anchor: AnchorRef; anchors: AnchorRef[] }
    | { kind: 'segment'; target: PathTarget; subpath: number; knot: number; t: number }
    | { kind: 'marquee'; targets: PathTarget[] }
  const decide = (targets: PathTarget[]): Hit => {
    const view = env.paths?.get()
    for (const target of targets) {
      const handle = hitHandle(target.path, view ? selectedOn(view, target) : [], p, reach)
      if (handle) return { kind: 'handle', target, ...handle }
    }
    for (const target of targets) {
      const anchor = hitAnchor(target.path, p, reach)
      if (!anchor) continue
      const chosen = view ? selectedOn(view, target) : []
      const has = chosen.some(([s, k]) => s === anchor[0] && k === anchor[1])
      // A press on a selected anchor drags them all; Shift adds one (or takes it away on a click); else it alone.
      const anchors = has ? [...chosen] : start.shift ? [...chosen, anchor] : [anchor]
      return { kind: 'anchor', target, anchor, anchors }
    }
    for (const target of targets) {
      const segment = hitSegment(target.path, p, reach)
      if (segment) return { kind: 'segment', target, ...segment }
    }
    return { kind: 'marquee', targets }
  }
  let hit: Hit | null = null
  const ready = targetsOf(env).then((targets) => (hit = decide(targets)))
  let moved = false
  const edited = (pointer: KitPointer): { path: PathData; anchors: readonly AnchorRef[] } | null => {
    if (!hit || hit.kind === 'marquee') return null
    const { dx, dy } = offset(start, pointer)
    if (hit.kind === 'handle') return { path: moveHandle(hit.target.path, hit.subpath, hit.knot, hit.handle, [pointer.x, pointer.y], pointer.alt), anchors: [[hit.subpath, hit.knot]] }
    if (hit.kind === 'anchor') return { path: moveAnchors(hit.target.path, hit.anchors, dx, dy), anchors: hit.anchors }
    // A bent segment: previewed by moving the grabbed point's neighbours' direction points (the engine reshapes it the same way).
    const sub = hit.target.path.subpaths[hit.subpath]
    const next = (hit.knot + 1) % sub.knots.length
    const weightA = 3 * (1 - hit.t) * (1 - hit.t) * hit.t
    const weightB = 3 * (1 - hit.t) * hit.t * hit.t
    const scale = weightA + weightB > 1e-6 ? 1 / (weightA + weightB) : 0
    let path = hit.target.path
    path = moveHandle(path, hit.subpath, hit.knot, 'out', [sub.knots[hit.knot].out[0] + dx * scale, sub.knots[hit.knot].out[1] + dy * scale], true)
    path = moveHandle(path, hit.subpath, next, 'in', [sub.knots[next].in[0] + dx * scale, sub.knots[next].in[1] + dy * scale], true)
    return { path, anchors: [[hit.subpath, hit.knot], [hit.subpath, next]] }
  }
  const show = (pointer: KitPointer) => {
    moved ||= dragged(start, pointer, zoom)
    if (!moved || !hit) return
    if (hit.kind === 'marquee') return drawMarquee(overlay, p, [pointer.x, pointer.y])
    const next = edited(pointer)
    if (!next) return
    editing(env, hit.target.kind)
    preview(overlay, next.path, next.anchors, tool)
  }
  return {
    move: show,
    end: async (pointer) => {
      moved ||= dragged(start, pointer, zoom)
      const decided = await ready
      if (!moved) {
        clear(overlay)
        // A click: selects what it hit.
        if (decided.kind === 'anchor') {
          const has = env.paths ? selectedOn(env.paths.get(), decided.target).some(([s, k]) => s === decided.anchor[0] && k === decided.anchor[1]) : false
          select(env, decided.target, start.shift && has ? selectedOn(env.paths!.get(), decided.target).filter(([s, k]) => !(s === decided.anchor[0] && k === decided.anchor[1])) : decided.anchors)
        } else if (decided.kind === 'segment') {
          const sub = decided.target.path.subpaths[decided.subpath]
          select(env, decided.target, [[decided.subpath, decided.knot], [decided.subpath, (decided.knot + 1) % sub.knots.length]])
        } else if (decided.kind === 'marquee' && !start.shift) select(env, null, [])
        return
      }
      const { dx, dy } = offset(start, pointer)
      if (decided.kind === 'marquee') {
        clear(overlay)
        const box = [start.x, start.y, pointer.x, pointer.y] as const
        for (const target of decided.targets) {
          const inside = anchorsIn(target.path, box)
          if (!inside.length) continue
          const before = env.paths && start.shift ? selectedOn(env.paths.get(), target) : []
          select(env, target, [...before, ...inside.filter(([s, k]) => !before.some(([a, b]) => a === s && b === k))])
          return
        }
        if (!start.shift) select(env, null, [])
        return
      }
      const next = edited(pointer)
      if (!next) return
      const name = targetParams(decided.target)
      if (decided.kind === 'handle') {
        select(env, decided.target, next.anchors)
        await commit(env, overlay, decided.target, next.path, 'path.moveHandle', { ...name, subpath: decided.subpath, knot: decided.knot, handle: decided.handle, to: [pointer.x, pointer.y], independent: pointer.alt })
      } else if (decided.kind === 'anchor') {
        if (!dx && !dy) return void editing(env, null)
        select(env, decided.target, decided.anchors)
        await commit(env, overlay, decided.target, next.path, 'path.moveAnchors', { ...name, anchors: decided.anchors.map(([s, k]) => [s, k]), move: [dx, dy] })
      } else {
        if (!dx && !dy) return void editing(env, null)
        await commit(env, overlay, decided.target, next.path, 'path.bendSegment', { ...name, subpath: decided.subpath, knot: decided.knot, t: decided.t, move: [dx, dy] })
      }
    },
    cancel: () => {
      clear(overlay)
      editing(env, null)
    },
  }
}

export function directSelectionTool(env: PhotoToolEnv): KitTool {
  return {
    id: 'DirectSelection',
    cursor: 'default',
    begin: (start, context) => directGesture(env, start, context),
  }
}

/**
 * Path Selection: a press on a path (on it, or inside its bounds) selects that component and a
 * drag moves it, the outline following the pointer; with no path there, a press on another shape
 * layer selects that layer and drags its path. One history step per drag.
 */
export function pathSelectionTool(env: PhotoToolEnv): KitTool {
  return {
    id: 'PathSelection',
    cursor: 'default',
    begin: (start, { overlay, viewport }) => {
      const zoom = viewport.view.zoom || 1
      const reach = HIT_RADIUS / zoom
      const p: Pt = [start.x, start.y]
      const ready = (async (): Promise<{ target: PathTarget; subpath: number } | null> => {
        const targets = await targetsOf(env)
        for (const target of targets) {
          const subpath = hitSubpath(target.path, p, reach)
          if (subpath != null) return { target, subpath }
        }
        // Another shape layer under the press: it becomes the active layer, and its path the one to move.
        if (!env.layerKind) {
          // Without the editor's store, the old way: the active shape layer, else the work path, whatever was pressed.
          const target = targets[0]
          return target ? { target, subpath: -1 } : null
        }
        const under = await env.query<{ layer: number | null }>('layer.pickAt', { x: Math.floor(start.x), y: Math.floor(start.y), select: false })
        if (under?.layer == null || env.layerKind(under.layer) !== 'Shape') return null
        if (!(await env.run('layer.select', { layer: under.layer, mode: 'replace' }, NO_REPAINT))) return null
        const info = await env.query<{ path: PathData }>('shape.info', { layer: under.layer })
        return info?.path ? { target: { kind: 'shape', layer: under.layer, path: info.path }, subpath: -1 } : null
      })()
      let found: { target: PathTarget; subpath: number } | null | undefined
      void ready.then((value) => {
        found = value
        if (!value) return select(env, null, [])
        // The component (or, without one known, the whole path) is selected.
        select(env, value.target, value.subpath >= 0 && value.target.path.subpaths.length > 1 ? subpathAnchors(value.target.path, value.subpath) : value.target.path.subpaths.flatMap((_, s) => subpathAnchors(value.target.path, s)))
      })
      let moved = false
      const movedPath = (pointer: KitPointer) => {
        if (!found) return null
        const { dx, dy } = offset(start, pointer)
        const whole = found.subpath < 0 || found.target.path.subpaths.length === 1
        return { dx, dy, whole, path: whole ? movePath(found.target.path, dx, dy) : moveAnchors(found.target.path, subpathAnchors(found.target.path, found.subpath), dx, dy) }
      }
      return {
        move: (pointer) => {
          moved ||= dragged(start, pointer, zoom)
          const next = moved ? movedPath(pointer) : null
          if (!next || !found) return
          editing(env, found.target.kind)
          overlay.draw((context, toScreen) => outlinePath(context, toScreen, next.path))
        },
        end: async (pointer) => {
          moved ||= dragged(start, pointer, zoom)
          await ready
          const next = moved ? movedPath(pointer) : null
          if (!next || !found || (!next.dx && !next.dy)) {
            clear(overlay)
            return void editing(env, null)
          }
          const { target } = found
          if (!next.whole) await commit(env, overlay, target, next.path, 'path.moveAnchors', { ...targetParams(target), anchors: subpathAnchors(target.path, found.subpath).map(([s, k]) => [s, k]), move: [next.dx, next.dy] })
          else if (target.kind === 'shape') await commit(env, overlay, target, next.path, 'shape.edit', { layer: target.layer, move: [next.dx, next.dy] })
          else await commit(env, overlay, target, next.path, 'path.transform', { name: 'work', translateX: next.dx, translateY: next.dy })
        },
        cancel: () => {
          clear(overlay)
          editing(env, null)
        },
      }
    },
  }
}

/** A Pen knot while it is drawn: anchor, in and out direction points, and 1 when Alt made it a corner. */
type PenKnot = [number, number, number, number, number, number] | [number, number, number, number, number, number, number]

const knotOf = ([x, y, ix, iy, ox, oy, corner]: PenKnot) => ({ anchor: [x, y] as Pt, in: [ix, iy] as Pt, out: [ox, oy] as Pt, smooth: !corner && (ix !== x || iy !== y || ox !== x || oy !== y) })

/** A small cursor of a pen nib with a mark: + adds an anchor, − deletes one, ^ converts one, o closes the path. */
function penCursor(mark: '' | '+' | '-' | '^' | 'o') {
  const marks: Record<string, string> = {
    '+': '<path d="M15 3v6M12 6h6" stroke="#000" stroke-width="1.6"/>',
    '-': '<path d="M12 6h6" stroke="#000" stroke-width="1.6"/>',
    '^': '<path d="M12 9l3-5 3 5" fill="none" stroke="#000" stroke-width="1.4"/>',
    o: '<circle cx="15" cy="6" r="2.6" fill="none" stroke="#000" stroke-width="1.4"/>',
  }
  const svg = `<svg xmlns="http://www.w3.org/2000/svg" width="22" height="22"><path d="M2 2l3 10 4 4 7-7-4-4z" fill="#fff" stroke="#000" stroke-width="1.2"/><circle cx="6.5" cy="6.5" r="1.2"/>${marks[mark] ?? ''}</svg>`
  return `url("data:image/svg+xml,${encodeURIComponent(svg)}") 2 2, crosshair`
}

/**
 * The Pen (plan 2026-11-24, stage 7): a click adds a corner anchor, a drag pulls its direction
 * lines (Alt: only the outgoing one), Shift keeps 45°; a click on the first anchor closes the path;
 * Enter or Escape finishes it. With no path being drawn, a click on the path's segment adds an
 * anchor there and one on an anchor deletes it (Photoshop's Auto Add/Delete), Alt on an anchor
 * converts it (a click makes a corner, a drag pulls smooth direction lines), and Cmd lends Direct
 * Selection. In Path mode the path goes into the work path (as a new component when there is one);
 * in Shape mode it becomes a shape layer.
 */
export function penTool(env: PhotoToolEnv): KitTool {
  const drawing = clickTool<PenKnot>('Pen', env, {
    min: 2,
    shiftSnaps: true,
    point: (pointer) => [pointer.x, pointer.y, pointer.x, pointer.y, pointer.x, pointer.y],
    drag: (knot, pointer) => {
      knot[4] = pointer.x
      knot[5] = pointer.y
      if (pointer.alt) {
        // Alt pulls the outgoing direction line alone: a corner.
        knot[6] = 1
        return
      }
      knot[2] = 2 * knot[0] - pointer.x
      knot[3] = 2 * knot[1] - pointer.y
    },
    draw: (context, toScreen, knots, hover) => {
      const path: PathData = { subpaths: [{ closed: false, knots: knots.map(knotOf) }] }
      // The rubber band: the next segment as it would curve to the pointer.
      if (hover && knots.length) {
        const last = knots[knots.length - 1]
        const a = toScreen(last[0], last[1])
        const c = toScreen(last[4], last[5])
        const b = toScreen(hover.x, hover.y)
        context.beginPath()
        context.moveTo(a.x, a.y)
        context.bezierCurveTo(c.x, c.y, b.x, b.y, b.x, b.y)
        context.setLineDash([4, 3])
        context.lineWidth = 1
        context.strokeStyle = 'rgba(0, 0, 0, 0.55)'
        context.stroke()
        context.setLineDash([])
      }
      const last = knots.length - 1
      drawPathPoints(context, toScreen, path, last >= 0 ? [[0, last]] : [], { anchors: 'all', handles: 'selected' })
    },
    finish: (knots, _first, closed) => {
      const path: PathData = { subpaths: [{ closed, knots: knots.map(knotOf) }] }
      if (env.options.penMode === 'shape') {
        const style = shapeStyle(env.options, env.foreground, env.background)
        const open = { fill: null, stroke: { width: Math.max(1, env.options.strokeWidth), color: env.foreground } }
        return env.run('shape.add', { kind: 'path', path, ...(closed ? style : open) })
      }
      // A work path there already takes the new path as another component of it (Photoshop's way).
      const work = env.paths?.get().work
      return env.run('path.set', work && work.subpaths.length ? { name: 'work', path, op: 'combine' } : { name: 'work', path }, NO_REPAINT)
    },
  })
  const busy = () => env.pending.current?.tool === 'Pen'
  /** What a press with no path being drawn does on a path: delete, add or convert an anchor. */
  const onPath = (pointer: KitPointer, zoom: number) => {
    if (!env.paths || busy()) return null
    return partAt(env.paths.get(), [pointer.x, pointer.y], HIT_RADIUS / zoom)
  }
  return {
    ...drawing,
    begin: (pointer, context) => {
      if (!busy() && pointer.meta) return directGesture(env, { ...pointer, meta: false }, context, 'Pen')
      const zoom = context.viewport.view.zoom || 1
      const part = onPath(pointer, zoom)
      if (part?.anchor && pointer.alt) return convertGesture(env, part.target, part.anchor, pointer, context, 'Pen')
      if (part?.anchor) {
        const after = deleteAnchor(part.target.path, part.anchor[0], part.anchor[1])
        void commit(env, context.overlay, part.target, after, ...replaceCommand(part.target, after))
        return
      }
      if (part?.segment && !pointer.alt) {
        const { subpath, knot, t } = part.segment
        const after = splitSegment(part.target.path, subpath, knot, t)
        select(env, part.target, [[subpath, knot + 1]])
        void commit(env, context.overlay, part.target, after, ...replaceCommand(part.target, after))
        return
      }
      return drawing.begin(pointer, context)
    },
    hover: (pointer, context) => {
      drawing.hover?.(pointer, context)
      const zoom = context.viewport.view.zoom || 1
      if (busy()) {
        const first = (env.pending.current as unknown as { points?: PenKnot[] }).points?.[0]
        return env.setCursor(first && Math.hypot(first[0] - pointer.x, first[1] - pointer.y) * zoom < HIT_RADIUS ? penCursor('o') : penCursor(''))
      }
      const part = onPath(pointer, zoom)
      env.setCursor(pointer.meta ? 'default' : part?.anchor ? penCursor(pointer.alt ? '^' : '-') : part?.segment && !pointer.alt ? penCursor('+') : penCursor(''))
    },
    leave: () => env.setCursor(null),
  }
}

/**
 * A press on an anchor that converts it (the Convert Point tool, Alt with the Pen): a click makes it
 * a corner with retracted direction lines, a drag pulls smooth direction lines out of it.
 */
function convertGesture(env: PhotoToolEnv, target: PathTarget, anchor: AnchorRef, start: KitPointer, { overlay, viewport }: Context, tool: string): KitGesture {
  const zoom = viewport.view.zoom || 1
  let moved = false
  const converted = (pointer: KitPointer) => convertPoint(target.path, anchor[0], anchor[1], moved ? [pointer.x, pointer.y] : null)
  select(env, target, [anchor])
  return {
    move: (pointer) => {
      moved ||= dragged(start, pointer, zoom)
      if (!moved) return
      editing(env, target.kind)
      preview(overlay, converted(pointer), [anchor], tool)
    },
    end: async (pointer) => {
      moved ||= dragged(start, pointer, zoom)
      const after = converted(pointer)
      await commit(env, overlay, target, after, 'path.convertPoint', { ...targetParams(target), subpath: anchor[0], knot: anchor[1], ...(moved ? { out: [pointer.x, pointer.y] } : {}) })
    },
    cancel: () => {
      clear(overlay)
      editing(env, null)
    },
  }
}

/**
 * Convert Point (plan 2026-11-24, stage 7): a drag on a direction point moves it alone (the point
 * becomes a corner), a drag from an anchor pulls smooth direction lines, a click on an anchor makes
 * it a corner. Every direction line of the path shows. Cmd lends Direct Selection.
 */
export function convertPointTool(env: PhotoToolEnv): KitTool {
  return {
    id: 'ConvertPoint',
    cursor: penCursor('^'),
    begin: (pointer, context) => {
      if (pointer.meta) return directGesture(env, { ...pointer, meta: false }, context, 'ConvertPoint')
      const zoom = context.viewport.view.zoom || 1
      const reach = HIT_RADIUS / zoom
      const p: Pt = [pointer.x, pointer.y]
      const targets = env.paths ? pathTargets(env.paths.get()) : []
      for (const target of targets) {
        const all = target.path.subpaths.flatMap((_, s) => subpathAnchors(target.path, s))
        const handle = hitHandle(target.path, all, p, reach)
        if (!handle) continue
        const { overlay } = context
        const dragTo = (to: KitPointer) => moveHandle(target.path, handle.subpath, handle.knot, handle.handle, [to.x, to.y], true)
        select(env, target, [[handle.subpath, handle.knot]])
        return {
          move: (to) => {
            editing(env, target.kind)
            preview(overlay, dragTo(to), [[handle.subpath, handle.knot]], 'ConvertPoint')
          },
          end: async (to) => {
            if (!dragged(pointer, to, zoom)) {
              clear(overlay)
              return void editing(env, null)
            }
            await commit(env, overlay, target, dragTo(to), 'path.moveHandle', { ...targetParams(target), subpath: handle.subpath, knot: handle.knot, handle: handle.handle, to: [to.x, to.y], independent: true })
          },
          cancel: () => {
            clear(overlay)
            editing(env, null)
          },
        }
      }
      const part = env.paths ? partAt(env.paths.get(), p, reach) : null
      if (part?.anchor) return convertGesture(env, part.target, part.anchor, pointer, context, 'ConvertPoint')
    },
  }
}

/** Arrow keys on selected anchors (Direct Selection, Pen, Convert Point) or a selected path (Path Selection): one `path.moveAnchors` a press, bundled into one step while pressed in a row. */
export async function nudgeAnchors(env: Pick<PhotoToolEnv, 'paths' | 'run' | 'redraw'>, dx: number, dy: number, coalesce: string): Promise<boolean> {
  const view = env.paths?.get()
  const selected = view?.selected
  if (!view || !selected?.anchors.length) return false
  const target = pathTargets(view).find((item) => item.kind === selected.kind && (item.kind === 'work' || item.layer === selected.layer))
  if (!target) return false
  const after = moveAnchors(target.path, selected.anchors, dx, dy)
  const ran = await env.run('path.moveAnchors', { ...targetParams(target), anchors: selected.anchors.map(([s, k]) => [s, k]), move: [dx, dy], coalesce }, target.kind === 'work' ? NO_REPAINT : undefined)
  if (ran && env.paths) {
    const now = env.paths.get()
    env.paths.set(target.kind === 'work' ? { ...now, work: after } : { ...now, shape: { layer: target.layer!, path: after } })
    env.redraw()
  }
  return true
}

/** Delete or Backspace on selected anchors: the path without them, in one step. */
export async function deleteSelectedAnchors(env: Pick<PhotoToolEnv, 'paths' | 'run' | 'redraw'>): Promise<boolean> {
  const view = env.paths?.get()
  const selected = view?.selected
  if (!view || !selected?.anchors.length) return false
  const target = pathTargets(view).find((item) => item.kind === selected.kind && (item.kind === 'work' || item.layer === selected.layer))
  if (!target) return false
  const after = deleteAnchors(target.path, selected.anchors)
  const [command, params] = after.subpaths.length || target.kind === 'shape' ? replaceCommand(target, after) : ['path.delete', { name: 'work' }]
  if (await env.run(command, params, target.kind === 'work' ? NO_REPAINT : undefined)) {
    env.paths!.set({ ...env.paths!.get(), selected: null })
    env.redraw()
  }
  return true
}

/** The canvas menu's Add / Delete Anchor Point at document point `p`: the anchor there goes, or one is added on the segment there. */
export async function editPointAt(env: Pick<PhotoToolEnv, 'paths' | 'run' | 'redraw'>, kind: 'addAnchor' | 'deleteAnchor', p: Pt, zoom: number): Promise<boolean> {
  const view = env.paths?.get()
  if (!view) return false
  const part = partAt(view, p, HIT_RADIUS / zoom)
  if (!part) return false
  let after: PathData | null = null
  if (kind === 'deleteAnchor' && part.anchor) after = deleteAnchor(part.target.path, part.anchor[0], part.anchor[1])
  if (kind === 'addAnchor' && part.segment) after = splitSegment(part.target.path, part.segment.subpath, part.segment.knot, part.segment.t)
  if (!after) return false
  return env.run(...replaceCommand(part.target, after), part.target.kind === 'work' ? NO_REPAINT : undefined)
}

export type { ToScreen }
