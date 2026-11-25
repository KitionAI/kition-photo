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
  const { session, t } = useKitEditor()
  const sessionNow = useRef(session)
  sessionNow.current = session
  const preview = useMemo(
    () =>
      createPreview({
        run: async (values) => {
          const result = await sessionNow.current.run(command, values)
          if (result === null) throw new Error(t('dialog.refused'))
        },
        // The preview's step leaves the history, Redo included (History's delete), so a cancelled
        // preview cannot come back by Redo; where the host denies that, it is undone instead.
        undo: async () => {
          if ((await sessionNow.current.run('history.deleteState', {})) === null) await sessionNow.current.run('edit.undo', {})
        },
        revision: () => sessionNow.current.doc?.info.revision ?? 0,
      }),
    // One preview for the dialog's life; `t` only words an error.
    // eslint-disable-next-line react-hooks/exhaustive-deps
    [command],
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
