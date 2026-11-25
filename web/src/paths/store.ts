// Imports carry `.ts` so `node --test` loads this module as it is.
import { createLive, type Live } from '../panels/live.ts'
import { hitAnchor, hitSegment, type AnchorRef, type PathData } from './bezier.ts'

/**
 * The paths the path tools edit (plan 2026-11-24, stage 7): the work path and the active shape
 * layer's path, as the engine last answered them, which anchors are selected, and whether a drag
 * is under way (its preview is drawn by the tool, so the canvas leaves the path out meanwhile).
 * The editor reloads it when the document changes; the canvas draws it (`drawPathEditing`).
 */
export type PathKind = 'work' | 'shape'
/** A path to edit: the work path, or shape layer `layer`'s. */
export type PathTarget = { kind: PathKind; layer: number | null; path: PathData }
export type PathSelection = { kind: PathKind; layer: number | null; anchors: readonly AnchorRef[] }
export type PathView = {
  work: PathData | null
  shape: { layer: number; path: PathData } | null
  selected: PathSelection | null
  /** A gesture draws the path itself now; the canvas leaves this one out. */
  editing: PathKind | null
}

export const NO_PATHS: PathView = { work: null, shape: null, selected: null, editing: null }

export type PathStore = Live<PathView>

export function createPathStore(): PathStore {
  return createLive<PathView>(NO_PATHS)
}

/** The paths a press can reach, in the order it tries them: the active shape layer's, then the work path. */
export function pathTargets(view: PathView): PathTarget[] {
  const out: PathTarget[] = []
  if (view.shape) out.push({ kind: 'shape', layer: view.shape.layer, path: view.shape.path })
  if (view.work) out.push({ kind: 'work', layer: null, path: view.work })
  return out
}

/** The engine's name for a target in the `path.*` commands: `work`, or `layer` with the shape layer's id. */
export function targetParams(target: { kind: PathKind; layer: number | null }): Record<string, unknown> {
  return target.kind === 'work' ? { name: 'work' } : { name: 'layer', layer: target.layer }
}

/** The command that puts a whole edited path back (an anchor added or deleted): `path.set` for the work path, `shape.edit` for a shape layer. */
export function replaceCommand(target: { kind: PathKind; layer: number | null }, path: PathData): [string, Record<string, unknown>] {
  return target.kind === 'work' ? ['path.set', { name: 'work', path }] : ['shape.edit', { layer: target.layer, path }]
}

/** Whether the selection is on `target`. */
export function selectsTarget(selected: PathSelection | null, target: { kind: PathKind; layer: number | null }) {
  return selected != null && selected.kind === target.kind && (target.kind === 'work' || selected.layer === target.layer)
}

/** The selected anchors of `target` (none when the selection is on another path). */
export function selectedOn(view: PathView, target: { kind: PathKind; layer: number | null }): readonly AnchorRef[] {
  return selectsTarget(view.selected, target) ? view.selected!.anchors : []
}

/** What lies under `p` on the reachable paths: an anchor, else a segment, with its target; or null. */
export function partAt(view: PathView, p: [number, number], reach: number): { target: PathTarget; anchor: AnchorRef | null; segment: { subpath: number; knot: number; t: number } | null } | null {
  for (const target of pathTargets(view)) {
    const anchor = hitAnchor(target.path, p, reach)
    if (anchor) return { target, anchor, segment: null }
  }
  for (const target of pathTargets(view)) {
    const segment = hitSegment(target.path, p, reach)
    if (segment) return { target, anchor: null, segment }
  }
  return null
}

/** The selection with anchors that no longer exist dropped (after an undo, say), or null when none is left. */
export function prunedSelection(view: PathView): PathSelection | null {
  const selected = view.selected
  if (!selected) return null
  const path = selected.kind === 'work' ? view.work : view.shape && view.shape.layer === selected.layer ? view.shape.path : null
  if (!path) return null
  const anchors = selected.anchors.filter(([s, k]) => path.subpaths[s]?.knots[k] != null)
  return { ...selected, anchors }
}
