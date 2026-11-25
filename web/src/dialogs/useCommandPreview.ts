import { useEffect, useMemo, useRef, useState } from 'react'
import { useKitEditor, type PreviewDialogPreview } from '@kition/sdk/editor'
import { createPreview, PREVIEW_DEBOUNCE_MS } from './preview'

/** `value`, once it has stayed the same (as JSON) for `ms`. */
export function useDebounced<T>(value: T, ms: number): T {
  const [settled, setSettled] = useState(value)
  const key = JSON.stringify(value)
  const latest = useRef(value)
  latest.current = value
  useEffect(() => {
    const timer = window.setTimeout(() => setSettled(latest.current), ms)
    return () => window.clearTimeout(timer)
  }, [key, ms])
  return settled
}

/**
 * A dialog's command, run through the editor's session (the mask target's, so a dialog edits what
 * the panels and tools edit): `preview` for `PreviewDialog` when the dialog previews (run, and on a
 * change take the preview's step away and run again, 150 ms after the last change), `ok` that
 * leaves one history step with `params`, and `cancel` that takes the preview's step away. A
 * refused or failed run throws, so the dialog shows why and stays open.
 */
export function useCommandPreview(command: string, params: Record<string, unknown>, previewing: boolean) {
  return useStepsPreview(command, (values) => [{ command, params: values }], params, previewing)
}

/** One command of a preview's run, and its parameters. */
export type PreviewStep = { command: string; params: Record<string, unknown> }

/**
 * `useCommandPreview` for a dialog whose run is several commands (Layer Style: the effects, then
 * the blending options): `steps` turns the dialog's values into them, and they run one after
 * another with one `coalesce` key, so the preview and OK are each one history step.
 */
export function useStepsPreview(id: string, steps: (params: Record<string, unknown>) => readonly PreviewStep[], params: Record<string, unknown>, previewing: boolean) {
  const { session, t } = useKitEditor()
  const sessionNow = useRef(session)
  sessionNow.current = session
  const stepsNow = useRef(steps)
  stepsNow.current = steps
  const preview = useMemo(
    () => {
      let runs = 0
      return createPreview({
        run: async (values) => {
          const list = stepsNow.current(values)
          // Several commands are one step: the same key, new for each run (a run never merges with the one before).
          runs += 1
          const coalesce = list.length > 1 ? `photo-dialog-${id}-${Date.now()}-${runs}` : undefined
          for (const step of list) {
            const result = await sessionNow.current.run(step.command, coalesce ? { ...step.params, coalesce } : step.params)
            if (result === null) throw new Error(t('dialog.refused'))
          }
        },
        // The preview's step leaves the history, Redo included (History's delete), so a cancelled
        // preview cannot come back by Redo; where the host denies that, it is undone instead.
        undo: async () => {
          if ((await sessionNow.current.run('history.deleteState', {})) === null) await sessionNow.current.run('edit.undo', {})
        },
        revision: () => sessionNow.current.doc?.info.revision ?? 0,
      })
    },
    // One preview for the dialog's life; `t` only words an error.
    // eslint-disable-next-line react-hooks/exhaustive-deps
    [id],
  )
  const settled = useDebounced(params, PREVIEW_DEBOUNCE_MS)
  const settledNow = useRef(settled)
  settledNow.current = settled
  const paramsNow = useRef(params)
  paramsNow.current = params
  const dialogPreview: PreviewDialogPreview | undefined = previewing
    ? {
        begin: () => preview.show(settledNow.current),
        update: () => preview.show(settledNow.current),
        end: () => preview.end(),
        draft: settled,
      }
    : undefined
  return {
    preview: dialogPreview,
    ok: () => preview.commit(paramsNow.current),
    cancel: () => void preview.drop(),
  }
}
