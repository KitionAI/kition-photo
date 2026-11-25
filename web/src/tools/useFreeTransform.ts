import { useCallback, useRef, useState } from 'react'
import { createPreview } from '../dialogs/preview'
import type { PhotoInfo } from '../photo'
import type { TransformBox } from './canvas'
import { quadCentre, quadFromFields, rectQuad, sameQuad, transformFields, type Point, type Quad } from './canvas/geometry'
import { livePreview, type LivePreview } from './canvas/live'

type Run = (command: string, params: Record<string, unknown>) => Promise<unknown>

/** The engine's `edit.transform` for a box: its source rectangle and where the corners go. */
export const transformParams = (box: TransformBox) => ({
  layer: box.layer,
  rect: box.rect,
  quad: box.quad.map((p) => [p.x, p.y]),
  interpolation: 'bicubic',
})

/** The person's view of a preview: run, take back (the history state deleted, else undone), the revision. */
export function previewEngine(command: string, run: Run, revision: () => number) {
  return createPreview({
    run: async (params) => {
      if ((await run(command, params)) === null) throw new Error(`${command} refused`)
    },
    undo: async () => {
      if ((await run('history.deleteState', {})) === null) await run('edit.undo', {})
    },
    revision,
  })
}

/**
 * Free Transform (plan 2026-11-24, stage 2): Cmd+T or Edit › Free Transform opens a box on the
 * active layer (its selected part when there is a selection) with eight handles, a rotation zone
 * outside and a reference point. Every drag previews `edit.transform` on the canvas, one history
 * step for the whole box; Enter (or a double click inside, or choosing another tool) commits it,
 * Escape takes it back. An untouched box commits nothing.
 */
export function useFreeTransform({ info, run, revision, say, redraw }: { info: () => PhotoInfo | null | undefined; run: Run; revision: () => number; say: (key: string) => void; redraw: () => void }) {
  const [box, setBox] = useState<TransformBox | null>(null)
  const boxNow = useRef<TransformBox | null>(null)
  const live = useRef<LivePreview | null>(null)
  // The revision the box's own last run left: an edit by anyone else after it (a menu command, an
  // agent) means the box's step is no longer the last, so closing leaves the history as it is.
  const ours = useRef<number | null>(null)
  const tracked = useCallback<Run>(
    async (command, params) => {
      const result = await run(command, params)
      ours.current = revision()
      return result
    },
    [run, revision],
  )
  const set = useCallback(
    (next: TransformBox | null, settled: boolean) => {
      boxNow.current = next
      redraw()
      if (settled) setBox(next)
    },
    [redraw],
  )
  const start = useCallback(
    (mode: TransformBox['mode'] = 'free') => {
      if (boxNow.current) return true
      const now = info()
      const active = now?.layers.find((layer) => layer.id === now.activeLayer)
      if (!now || !active || active.id == null) return false
      if (active.background || active.locks.position || active.locks.all) {
        say('transform.locked')
        return false
      }
      const bounds = now.activeBounds
      const selected = now.selectionBounds
      let rect: [number, number, number, number] | null = bounds ? [bounds.x0, bounds.y0, bounds.x1, bounds.y1] : null
      if (rect && selected) rect = [Math.max(rect[0], selected[0]), Math.max(rect[1], selected[1]), Math.min(rect[2], selected[0] + selected[2]), Math.min(rect[3], selected[1] + selected[3])]
      if (!rect || rect[2] <= rect[0] || rect[3] <= rect[1]) {
        say('transform.empty')
        return false
      }
      const quad = rectQuad(...rect)
      ours.current = null
      live.current = livePreview(previewEngine('edit.transform', tracked, revision), () => say('transform.failed'))
      set({ layer: active.id, rect, quad, reference: quadCentre(quad), mode }, true)
      return true
    },
    [info, revision, tracked, say, set],
  )
  const update = useCallback(
    (quad: Quad, reference: Point, settled: boolean) => {
      const open = boxNow.current
      if (!open) return
      const next = { ...open, quad, reference }
      set(next, settled)
      live.current?.push(transformParams(next))
    },
    [set],
  )
  /** Ends the box: kept (one step, none when untouched) or taken back. */
  const close = useCallback(
    async (keep: boolean) => {
      const open = boxNow.current
      const preview = live.current
      live.current = null
      set(null, true)
      if (!open || !preview) return
      if (ours.current !== null && revision() !== ours.current) return
      if (keep && !sameQuad(open.quad, rectQuad(...open.rect))) await preview.finish(transformParams(open))
      else await preview.cancel()
    },
    [revision, set],
  )
  /** The options bar's numbers for the open box, and their setter. */
  const fields = box ? transformFields(box.quad, box.rect, box.reference) : null
  const setFields = useCallback(
    (patch: Partial<NonNullable<typeof fields>>) => {
      const open = boxNow.current
      if (!open) return
      const now = { ...transformFields(open.quad, open.rect, open.reference), ...patch }
      const quad = quadFromFields(open.rect, now)
      update(quad, { x: now.x, y: now.y }, true)
    },
    [update],
  )
  /** Drops the box without touching the history (the document was replaced). */
  const forget = useCallback(() => {
    live.current = null
    set(null, true)
  }, [set])
  return { box, boxNow, start, update, commit: () => close(true), cancel: () => close(false), fields, setFields, forget }
}
